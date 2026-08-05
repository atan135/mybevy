mod widgets;

use bevy::prelude::*;
use std::{env, str::FromStr};

use crate::framework::ui::{
    audit::{
        UiAuditCaptureRecipe, UiAuditCaptureState, UiAuditDynamicContentRecipe,
        UiAuditReadyCondition, UiAuditRecipe, UiAuditReferenceRecipe, UiAuditRouteCommand,
        UiAuditScreen, UiAuditScreenRecipe, UiAuditScreenRegistry, UiAuditTargetViewport,
    },
    core::binding::UiBindingValues,
    core::{UiCurrentOwner, UiOwnerId, UiPanelCommand, UiPanelSystems},
    document::{
        UiActionDescriptor, UiActionDispatch, UiActionId, UiActionParamSchema, UiActionParamType,
        UiActionRegistry, UiBindingPath, UiBindingType, UiDocumentId, UiDocumentRuntimeCommand,
        UiNodeId, UiRegisteredActionKind,
    },
    widgets::{UiButtonEvent, UiButtonEventKind, UiScrollAuditPosition},
};
use crate::game::declarative_screen::{
    DeclarativeScreenHostCommand, DeclarativeScreenRegistry,
    register_declarative_route_audit_entries,
};
use crate::game::ui_ids::{
    ANCHOR_UI_GALLERY_ANIMATIONS, ANCHOR_UI_GALLERY_COMPONENT_CHECKBOXES,
    ANCHOR_UI_GALLERY_COMPONENT_DROPDOWN, ANCHOR_UI_GALLERY_COMPONENT_SEGMENTED,
    ANCHOR_UI_GALLERY_COMPONENT_TOGGLES, ANCHOR_UI_GALLERY_COMPONENT_TOOLTIP,
    ANCHOR_UI_GALLERY_COMPONENTS, ANCHOR_UI_GALLERY_EFFECTS, ANCHOR_UI_GALLERY_ICON_STATES,
    ANCHOR_UI_GALLERY_ICONS, ANCHOR_UI_GALLERY_IMAGE_ATLAS, ANCHOR_UI_GALLERY_IMAGE_MODES,
    ANCHOR_UI_GALLERY_IMAGE_TILING, ANCHOR_UI_GALLERY_STYLE_SCOPES, ANCHOR_UI_GALLERY_TYPOGRAPHY,
    ANCHOR_UI_GALLERY_TYPOGRAPHY_OVERFLOW, ANCHOR_UI_GALLERY_VISUAL_ACCEPTANCE,
    OWNER_AI_LOGIN_REFERENCE, OWNER_AUDIO_GALLERY, OWNER_AUDIO_MONITOR, OWNER_AUDIO_SETTINGS,
    OWNER_CHARACTER_SELECT, OWNER_FANGYUAN_HOME, OWNER_FANGYUAN_PLAYER_PREVIEW, OWNER_LOBBY,
    OWNER_LOGIN, OWNER_MAIN_WORLD, OWNER_ROBOT_SYNC_SCENE, OWNER_SAMPLE_SCENE, OWNER_TOUCH_RIPPLE,
    OWNER_UI_APPROVED_BUSINESS_ACCEPTANCE, OWNER_UI_DOCUMENT_GALLERY, OWNER_UI_GALLERY,
    OWNER_UI_GENERATED_ACCEPTANCE, SCROLL_UI_GALLERY_MAIN,
};
#[cfg(all(debug_assertions, not(target_os = "android")))]
use crate::game::ui_ids::{SCROLL_AUDIO_SETTINGS_MAIN, SCROLL_FANGYUAN_HOME_MAIN};

pub(in crate::game) use widgets::{game_panel_root, secondary_route_button_key};

pub(super) struct NavigationPlugin;

impl Plugin for NavigationPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppUiMode>()
            .init_resource::<DeclarativeScreenRegistry>()
            .add_message::<GameRouteCommand>()
            .add_message::<DeclarativeScreenHostCommand>()
            .add_systems(
                Startup,
                (
                    register_ui_audit_screens,
                    register_game_ui_actions,
                    setup_start_mode,
                ),
            );
        app.configure_sets(
            Update,
            GameRouteSystems::Commands.before(UiPanelSystems::Commands),
        )
        .add_systems(
            Update,
            (
                handle_route_buttons,
                handle_ui_audit_route_commands,
                handle_declarative_ui_actions,
                handle_game_route_commands,
            )
                .chain()
                .in_set(GameRouteSystems::Commands),
        );
    }
}

const DECLARATIVE_CONTINUE_ACTION: &str = "example.continue";
const DECLARATIVE_MINIMAL_DOCUMENT: &str = "example.minimal_page";
const DECLARATIVE_LOBBY_ROUTE: &str = "game.route_lobby";
const DECLARATIVE_CONTINUE_NODE: &str = "page.continue";
pub(in crate::game) const UI_DOCUMENT_GALLERY_DOCUMENT: &str = "gallery.declarative";
const UI_DOCUMENT_GALLERY_ACTION: &str = "gallery.set_status";
const UI_DOCUMENT_GALLERY_CONTROL_ACTION: &str = "gallery.control_changed";
const APPROVED_BUSINESS_ACCEPTANCE_DOCUMENT: &str = "approved.business_acceptance";
const APPROVED_BUSINESS_ACCEPTANCE_ACTION: &str = "approved.acceptance_continue";

fn register_game_ui_actions(mut registry: ResMut<UiActionRegistry>) {
    registry
        .register(
            UiActionDescriptor::new(
                UiActionId::from_str(DECLARATIVE_CONTINUE_ACTION).unwrap(),
                UiDocumentId::from_str(DECLARATIVE_MINIMAL_DOCUMENT).unwrap(),
                OWNER_LOGIN.as_str(),
                UiRegisteredActionKind::Route {
                    target: DECLARATIVE_LOBBY_ROUTE.to_owned(),
                },
            )
            .with_source(UiNodeId::from_str(DECLARATIVE_CONTINUE_NODE).unwrap()),
        )
        .expect("declarative UI action registration must be valid and unique");
    registry
        .register(
            UiActionDescriptor::new(
                UiActionId::from_str(UI_DOCUMENT_GALLERY_ACTION).unwrap(),
                UiDocumentId::from_str(UI_DOCUMENT_GALLERY_DOCUMENT).unwrap(),
                OWNER_UI_DOCUMENT_GALLERY.as_str(),
                UiRegisteredActionKind::UpdateLocalState {
                    binding: UiBindingPath::from_str("gallery.status").unwrap(),
                    value_param: "value".to_owned(),
                },
            )
            .with_source(UiNodeId::from_str("gallery.action").unwrap())
            .with_param(
                "value",
                UiActionParamSchema::required(UiActionParamType::Binding(UiBindingType::String)),
            ),
        )
        .expect("declarative Gallery local action registration must be valid and unique");
    registry
        .register(
            UiActionDescriptor::new(
                UiActionId::from_str(UI_DOCUMENT_GALLERY_CONTROL_ACTION).unwrap(),
                UiDocumentId::from_str(UI_DOCUMENT_GALLERY_DOCUMENT).unwrap(),
                OWNER_UI_DOCUMENT_GALLERY.as_str(),
                UiRegisteredActionKind::BusinessCommand {
                    target: "game.gallery_control_changed".to_owned(),
                },
            )
            .with_sources(
                [
                    "gallery.input",
                    "gallery.checkbox",
                    "gallery.toggle",
                    "gallery.segmented",
                    "gallery.slider",
                    "gallery.stepper",
                    "gallery.tab_overview",
                    "gallery.tab_details",
                    "gallery.select",
                ]
                .into_iter()
                .map(|node| UiNodeId::from_str(node).unwrap()),
            ),
        )
        .expect("declarative Gallery control action registration must be valid and unique");
    registry
        .register(
            UiActionDescriptor::new(
                UiActionId::from_str(APPROVED_BUSINESS_ACCEPTANCE_ACTION).unwrap(),
                UiDocumentId::from_str(APPROVED_BUSINESS_ACCEPTANCE_DOCUMENT).unwrap(),
                OWNER_UI_APPROVED_BUSINESS_ACCEPTANCE.as_str(),
                UiRegisteredActionKind::BusinessCommand {
                    target: "game.approved_business_acceptance".to_owned(),
                },
            )
            .with_source(UiNodeId::from_str("acceptance.continue").unwrap()),
        )
        .expect("approved business acceptance action registration must be valid and unique");
}

fn handle_declarative_ui_actions(
    mut actions: MessageReader<UiActionDispatch>,
    mut route_commands: MessageWriter<GameRouteCommand>,
) {
    for action in actions.read() {
        if let Some(mode) = adapt_registered_ui_action(action) {
            route_commands.write(GameRouteCommand::ChangeMode(mode));
        }
    }
}

fn adapt_registered_ui_action(action: &UiActionDispatch) -> Option<AppUiMode> {
    match &action.kind {
        UiRegisteredActionKind::Route { target }
            if action.action.as_str() == DECLARATIVE_CONTINUE_ACTION
                && action.document_id.as_str() == DECLARATIVE_MINIMAL_DOCUMENT
                && action.owner == OWNER_LOGIN.as_str()
                && action.source_node.as_str() == DECLARATIVE_CONTINUE_NODE
                && target == DECLARATIVE_LOBBY_ROUTE =>
        {
            Some(AppUiMode::Lobby)
        }
        _ => None,
    }
}

#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
pub(super) enum AppUiMode {
    #[default]
    Login,
    CharacterSelect,
    Lobby,
    MainWorld,
    AudioSettings,
    AudioMonitor,
    AudioGallery,
    WanfaTouchRipple,
    UiGallery,
    UiDocumentGallery,
    UiGeneratedAcceptance,
    AiLoginReference,
    SampleScene,
    RobotSyncScene,
    FangyuanHome,
    FangyuanPlayerPreview,
}

impl AppUiMode {
    pub(crate) const fn ui_owner(self) -> UiOwnerId {
        match self {
            Self::Login => OWNER_LOGIN,
            Self::CharacterSelect => OWNER_CHARACTER_SELECT,
            Self::Lobby => OWNER_LOBBY,
            Self::MainWorld => OWNER_MAIN_WORLD,
            Self::AudioSettings => OWNER_AUDIO_SETTINGS,
            Self::AudioMonitor => OWNER_AUDIO_MONITOR,
            Self::AudioGallery => OWNER_AUDIO_GALLERY,
            Self::WanfaTouchRipple => OWNER_TOUCH_RIPPLE,
            Self::UiGallery => OWNER_UI_GALLERY,
            Self::UiDocumentGallery => OWNER_UI_DOCUMENT_GALLERY,
            Self::UiGeneratedAcceptance => OWNER_UI_GENERATED_ACCEPTANCE,
            Self::AiLoginReference => OWNER_AI_LOGIN_REFERENCE,
            Self::SampleScene => OWNER_SAMPLE_SCENE,
            Self::RobotSyncScene => OWNER_ROBOT_SYNC_SCENE,
            Self::FangyuanHome => OWNER_FANGYUAN_HOME,
            Self::FangyuanPlayerPreview => OWNER_FANGYUAN_PLAYER_PREVIEW,
        }
    }

    pub(crate) const fn canonical_screen(self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::CharacterSelect => "character_select",
            Self::Lobby => "lobby",
            Self::MainWorld => "main_world",
            Self::AudioSettings => "audio_settings",
            Self::AudioMonitor => "audio_monitor",
            Self::AudioGallery => "audio_gallery",
            Self::WanfaTouchRipple => "wanfa_touch_ripple",
            Self::UiGallery => "ui_gallery",
            Self::UiDocumentGallery => "ui_document_gallery",
            Self::UiGeneratedAcceptance => "ui_generated_acceptance",
            Self::AiLoginReference => "ai_login_reference",
            Self::SampleScene => "sample_scene",
            Self::RobotSyncScene => "robot_sync_scene",
            Self::FangyuanHome => "fangyuan_home",
            Self::FangyuanPlayerPreview => "fangyuan_player_preview",
        }
    }

    pub(crate) const fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::Login => &["login"],
            Self::CharacterSelect => &[
                "character_select",
                "character-select",
                "characters",
                "select_character",
                "select-character",
            ],
            Self::Lobby => &["lobby", "game_list", "game-list", "list"],
            Self::MainWorld => &["main_world", "main-world"],
            Self::AudioSettings => &["audio_settings", "audio-settings", "audio", "settings"],
            Self::AudioMonitor => &[
                "audio_monitor",
                "audio-monitor",
                "audio_debug",
                "audio-debug",
            ],
            Self::AudioGallery => &["audio_gallery", "audio-gallery"],
            Self::WanfaTouchRipple => &[
                "wanfa_touch_ripple",
                "wanfa-touch-ripple",
                "touch",
                "touch_ripple",
                "touch-ripple",
            ],
            Self::UiGallery => &["ui_gallery", "ui-gallery", "gallery"],
            Self::UiDocumentGallery => &[
                "ui_document_gallery",
                "ui-document-gallery",
                "document_gallery",
                "document-gallery",
                "declarative_gallery",
            ],
            Self::UiGeneratedAcceptance => &[
                "ui_generated_acceptance",
                "ui-generated-acceptance",
                "generated_acceptance",
            ],
            Self::AiLoginReference => &["ai_login_reference", "ai-login-reference", "image_login"],
            Self::SampleScene => &["sample_scene", "sample-scene", "sample"],
            Self::RobotSyncScene => &["robot_sync_scene", "robot-sync-scene", "robot"],
            Self::FangyuanHome => &["fangyuan_home", "fangyuan-home", "fangyuan"],
            Self::FangyuanPlayerPreview => &[
                "fangyuan_player_preview",
                "fangyuan-player-preview",
                "fangyuan_player",
                "fangyuan-player",
                "fangyuan_avatar",
                "fangyuan-avatar",
            ],
        }
    }
}

#[derive(Component)]
pub(super) struct RouteButton {
    pub(super) target: AppUiMode,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(in crate::game) enum GameRouteSystems {
    Commands,
}

#[derive(Clone, Debug, Message)]
pub(in crate::game) enum GameRouteCommand {
    ChangeMode(AppUiMode),
}

fn handle_route_buttons(
    mut route_commands: MessageWriter<GameRouteCommand>,
    route_buttons: Query<&RouteButton>,
    mut button_events: MessageReader<UiButtonEvent>,
) {
    for event in button_events.read() {
        if event.kind != UiButtonEventKind::Click {
            continue;
        }

        let Ok(route_button) = route_buttons.get(event.entity) else {
            continue;
        };
        route_commands.write(GameRouteCommand::ChangeMode(route_button.target));
    }
}

fn handle_ui_audit_route_commands(
    mut audit_route_commands: MessageReader<UiAuditRouteCommand>,
    mut route_commands: MessageWriter<GameRouteCommand>,
    declarative_hosts: Res<DeclarativeScreenRegistry>,
    mut declarative_commands: MessageWriter<DeclarativeScreenHostCommand>,
) {
    for command in audit_route_commands.read() {
        if let Some(mode) = parse_start_screen_mode(&command.screen) {
            if mode.ui_owner() != command.owner {
                warn!(
                    "ui audit route owner mismatch: screen={}, expected={}, actual={}",
                    command.screen,
                    command.owner,
                    mode.ui_owner()
                );
                continue;
            }
            route_commands.write(GameRouteCommand::ChangeMode(mode));
            continue;
        }
        if let Some(host) = declarative_hosts.route(&command.screen) {
            if host.owner != command.owner {
                warn!(
                    "ui audit declarative route owner mismatch: screen={}, expected={}, actual={}",
                    command.screen, command.owner, host.owner
                );
                continue;
            }
            declarative_commands.write(DeclarativeScreenHostCommand::OpenRoute {
                route: host.route.to_owned(),
            });
            continue;
        }
        warn!(
            "ui audit route ignored unknown screen alias: {}",
            command.screen
        );
    }
}

fn handle_game_route_commands(
    mut route_commands: MessageReader<GameRouteCommand>,
    mut next_mode: ResMut<NextState<AppUiMode>>,
    current_mode: Res<State<AppUiMode>>,
    mut current_owner: ResMut<UiCurrentOwner>,
    mut binding_values: ResMut<UiBindingValues>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
    mut runtime_commands: MessageWriter<UiDocumentRuntimeCommand>,
) {
    if current_owner.owner.is_none() {
        current_owner.owner = Some(current_mode.get().ui_owner());
    }

    for command in route_commands.read() {
        match command {
            GameRouteCommand::ChangeMode(mode) => {
                let previous_owner = current_owner.owner.unwrap_or(current_mode.get().ui_owner());
                if mode == current_mode.get() && previous_owner == mode.ui_owner() {
                    continue;
                }
                if *current_mode.get() == AppUiMode::MainWorld {
                    for owner in
                        crate::game::screens::gameplay::host::main_world_ui_cleanup_owners()
                    {
                        binding_values.clear_owner(owner.as_str());
                        panel_commands.write(UiPanelCommand::CloseAllForOwner(*owner));
                        runtime_commands.write(UiDocumentRuntimeCommand::SwitchOwner {
                            previous_owner: owner.as_str().to_owned(),
                        });
                    }
                } else {
                    binding_values.clear_owner(previous_owner.as_str());
                    panel_commands.write(UiPanelCommand::CloseAllForOwner(previous_owner));
                    runtime_commands.write(UiDocumentRuntimeCommand::SwitchOwner {
                        previous_owner: previous_owner.as_str().to_owned(),
                    });
                }
                current_owner.owner = Some(mode.ui_owner());
                next_mode.set(*mode);
            }
        }
    }
}

fn setup_start_mode(
    mut next_mode: ResMut<NextState<AppUiMode>>,
    declarative_hosts: Res<DeclarativeScreenRegistry>,
    mut declarative_commands: MessageWriter<DeclarativeScreenHostCommand>,
) {
    let Ok(value) = env::var("TOUCH_START_SCREEN") else {
        return;
    };

    if let Some(mode) = parse_start_screen_mode(&value) {
        next_mode.set(mode);
        return;
    }
    if let Some(host) = declarative_hosts.route(&value) {
        declarative_commands.write(DeclarativeScreenHostCommand::OpenRoute {
            route: host.route.to_owned(),
        });
    }
}

pub(crate) fn parse_start_screen_mode(value: &str) -> Option<AppUiMode> {
    all_app_ui_modes().into_iter().find(|mode| {
        mode.aliases()
            .iter()
            .any(|alias| alias.eq_ignore_ascii_case(value.trim()))
    })
}

fn register_ui_audit_screens(
    mut registry: ResMut<UiAuditScreenRegistry>,
    declarative_hosts: Res<DeclarativeScreenRegistry>,
) {
    register_ui_audit_screen_entries(&mut registry);
    register_declarative_route_audit_entries(&mut registry, &declarative_hosts);
}

fn register_ui_audit_screen_entries(registry: &mut UiAuditScreenRegistry) {
    for mode in all_app_ui_modes() {
        let screen = UiAuditScreen::new(mode.canonical_screen(), mode.aliases(), mode.ui_owner());
        #[cfg(all(debug_assertions, not(target_os = "android")))]
        if mode == AppUiMode::AudioSettings {
            registry.register_recipe(UiAuditScreenRecipe::new(
                screen.with_recipe(
                    UiAuditRecipe::new(AUDIO_SETTINGS_AUDIT_CAPTURES)
                        .with_reference(DEFAULT_REFERENCE_AUDIT_RECIPE)
                        .with_ready(UiAuditReadyCondition::OwnerDocument),
                ),
            ));
            continue;
        }
        #[cfg(all(debug_assertions, not(target_os = "android")))]
        if mode == AppUiMode::FangyuanHome {
            registry.register_recipe(UiAuditScreenRecipe::new(
                screen.with_recipe(
                    UiAuditRecipe::new(FANGYUAN_HOME_AUDIT_CAPTURES)
                        .with_reference(DEFAULT_REFERENCE_AUDIT_RECIPE)
                        .with_ready(UiAuditReadyCondition::OwnerDocument),
                ),
            ));
            continue;
        }
        if mode == AppUiMode::UiGallery {
            registry.register_recipe(UiAuditScreenRecipe::new(
                screen.with_recipe(
                    UiAuditRecipe::new(UI_GALLERY_AUDIT_CAPTURES)
                        .with_reference(DEFAULT_REFERENCE_AUDIT_RECIPE),
                ),
            ));
        } else if matches!(
            mode,
            AppUiMode::Login
                | AppUiMode::CharacterSelect
                | AppUiMode::Lobby
                | AppUiMode::MainWorld
                | AppUiMode::AudioSettings
                | AppUiMode::WanfaTouchRipple
                | AppUiMode::SampleScene
                | AppUiMode::RobotSyncScene
                | AppUiMode::FangyuanHome
                | AppUiMode::FangyuanPlayerPreview
                | AppUiMode::UiDocumentGallery
                | AppUiMode::UiGeneratedAcceptance
        ) {
            registry.register_recipe(UiAuditScreenRecipe::new(
                screen.with_recipe(
                    UiAuditRecipe::new(DOCUMENT_AUDIT_CAPTURES)
                        .with_reference(DEFAULT_REFERENCE_AUDIT_RECIPE)
                        .with_ready(UiAuditReadyCondition::OwnerDocument),
                ),
            ));
        } else {
            registry.register(screen);
        }
    }
}

const DEFAULT_REFERENCE_AUDIT_RECIPE: UiAuditReferenceRecipe = UiAuditReferenceRecipe::new()
    .with_target_viewport(UiAuditTargetViewport::RuntimeProfile)
    .with_locale("zh_cn")
    .with_theme("default")
    .with_random_seed(None)
    .with_frozen_time_seconds(0.0)
    .with_animation_progress(1.0)
    .with_dynamic_content(UiAuditDynamicContentRecipe::StableFixture(
        "repository_static_data",
    ));

const DOCUMENT_AUDIT_CAPTURES: &[UiAuditCaptureRecipe] = &[UiAuditCaptureRecipe::initial()];

#[cfg(all(debug_assertions, not(target_os = "android")))]
const AUDIO_SETTINGS_AUDIT_CAPTURES: &[UiAuditCaptureRecipe] = &[
    UiAuditCaptureRecipe::initial(),
    UiAuditCaptureRecipe::scroll(
        UiAuditCaptureState::Bottom,
        SCROLL_AUDIO_SETTINGS_MAIN,
        UiScrollAuditPosition::Bottom,
    ),
];

#[cfg(all(debug_assertions, not(target_os = "android")))]
const FANGYUAN_HOME_AUDIT_CAPTURES: &[UiAuditCaptureRecipe] = &[
    UiAuditCaptureRecipe::initial(),
    UiAuditCaptureRecipe::scroll(
        UiAuditCaptureState::Bottom,
        SCROLL_FANGYUAN_HOME_MAIN,
        UiScrollAuditPosition::Bottom,
    ),
];

const UI_GALLERY_AUDIT_CAPTURES: &[UiAuditCaptureRecipe] = &[
    UiAuditCaptureRecipe::scroll(
        UiAuditCaptureState::VisualFoundation,
        SCROLL_UI_GALLERY_MAIN,
        UiScrollAuditPosition::Top,
    ),
    UiAuditCaptureRecipe::scroll_anchor(
        UiAuditCaptureState::VisualAcceptance,
        SCROLL_UI_GALLERY_MAIN,
        ANCHOR_UI_GALLERY_VISUAL_ACCEPTANCE,
    ),
    UiAuditCaptureRecipe::scroll(
        UiAuditCaptureState::ImageFit,
        SCROLL_UI_GALLERY_MAIN,
        UiScrollAuditPosition::Top,
    ),
    UiAuditCaptureRecipe::scroll_anchor(
        UiAuditCaptureState::ImageModes,
        SCROLL_UI_GALLERY_MAIN,
        ANCHOR_UI_GALLERY_IMAGE_MODES,
    ),
    UiAuditCaptureRecipe::scroll_anchor(
        UiAuditCaptureState::ImageTiling,
        SCROLL_UI_GALLERY_MAIN,
        ANCHOR_UI_GALLERY_IMAGE_TILING,
    ),
    UiAuditCaptureRecipe::scroll_anchor(
        UiAuditCaptureState::ImageAtlas,
        SCROLL_UI_GALLERY_MAIN,
        ANCHOR_UI_GALLERY_IMAGE_ATLAS,
    ),
    UiAuditCaptureRecipe::scroll_anchor(
        UiAuditCaptureState::Typography,
        SCROLL_UI_GALLERY_MAIN,
        ANCHOR_UI_GALLERY_TYPOGRAPHY,
    ),
    UiAuditCaptureRecipe::scroll_anchor(
        UiAuditCaptureState::TypographyOverflow,
        SCROLL_UI_GALLERY_MAIN,
        ANCHOR_UI_GALLERY_TYPOGRAPHY_OVERFLOW,
    ),
    UiAuditCaptureRecipe::scroll_anchor(
        UiAuditCaptureState::Icons,
        SCROLL_UI_GALLERY_MAIN,
        ANCHOR_UI_GALLERY_ICONS,
    ),
    UiAuditCaptureRecipe::scroll_anchor(
        UiAuditCaptureState::IconStates,
        SCROLL_UI_GALLERY_MAIN,
        ANCHOR_UI_GALLERY_ICON_STATES,
    ),
    UiAuditCaptureRecipe::scroll_anchor(
        UiAuditCaptureState::StyleScopes,
        SCROLL_UI_GALLERY_MAIN,
        ANCHOR_UI_GALLERY_STYLE_SCOPES,
    ),
    UiAuditCaptureRecipe::scroll_anchor(
        UiAuditCaptureState::Effects,
        SCROLL_UI_GALLERY_MAIN,
        ANCHOR_UI_GALLERY_EFFECTS,
    ),
    UiAuditCaptureRecipe::scroll_anchor(
        UiAuditCaptureState::Animations,
        SCROLL_UI_GALLERY_MAIN,
        ANCHOR_UI_GALLERY_ANIMATIONS,
    ),
    UiAuditCaptureRecipe::scroll_anchor(
        UiAuditCaptureState::Components,
        SCROLL_UI_GALLERY_MAIN,
        ANCHOR_UI_GALLERY_COMPONENTS,
    ),
    UiAuditCaptureRecipe::scroll_anchor(
        UiAuditCaptureState::ComponentCheckboxes,
        SCROLL_UI_GALLERY_MAIN,
        ANCHOR_UI_GALLERY_COMPONENT_CHECKBOXES,
    ),
    UiAuditCaptureRecipe::scroll_anchor(
        UiAuditCaptureState::ComponentToggles,
        SCROLL_UI_GALLERY_MAIN,
        ANCHOR_UI_GALLERY_COMPONENT_TOGGLES,
    ),
    UiAuditCaptureRecipe::scroll_anchor(
        UiAuditCaptureState::ComponentSegmented,
        SCROLL_UI_GALLERY_MAIN,
        ANCHOR_UI_GALLERY_COMPONENT_SEGMENTED,
    ),
    UiAuditCaptureRecipe::scroll_anchor(
        UiAuditCaptureState::ComponentOverlays,
        SCROLL_UI_GALLERY_MAIN,
        ANCHOR_UI_GALLERY_COMPONENT_DROPDOWN,
    ),
    UiAuditCaptureRecipe::scroll_anchor(
        UiAuditCaptureState::ComponentTooltip,
        SCROLL_UI_GALLERY_MAIN,
        ANCHOR_UI_GALLERY_COMPONENT_TOOLTIP,
    ),
    UiAuditCaptureRecipe::scroll(
        UiAuditCaptureState::Top,
        SCROLL_UI_GALLERY_MAIN,
        UiScrollAuditPosition::Top,
    ),
    UiAuditCaptureRecipe::scroll(
        UiAuditCaptureState::Middle,
        SCROLL_UI_GALLERY_MAIN,
        UiScrollAuditPosition::Middle,
    ),
    UiAuditCaptureRecipe::scroll(
        UiAuditCaptureState::Bottom,
        SCROLL_UI_GALLERY_MAIN,
        UiScrollAuditPosition::Bottom,
    ),
];

fn all_app_ui_modes() -> [AppUiMode; 16] {
    [
        AppUiMode::Login,
        AppUiMode::CharacterSelect,
        AppUiMode::Lobby,
        AppUiMode::MainWorld,
        AppUiMode::AudioSettings,
        AppUiMode::AudioMonitor,
        AppUiMode::AudioGallery,
        AppUiMode::WanfaTouchRipple,
        AppUiMode::UiGallery,
        AppUiMode::UiDocumentGallery,
        AppUiMode::UiGeneratedAcceptance,
        AppUiMode::AiLoginReference,
        AppUiMode::SampleScene,
        AppUiMode::RobotSyncScene,
        AppUiMode::FangyuanHome,
        AppUiMode::FangyuanPlayerPreview,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ui_ids::{
        OWNER_AUDIO_GALLERY, OWNER_AUDIO_SETTINGS, OWNER_CHARACTER_SELECT, OWNER_FANGYUAN_HOME,
        OWNER_FANGYUAN_PLAYER_PREVIEW, OWNER_MAIN_WORLD_MAIL_PANEL,
        OWNER_MAIN_WORLD_SETTINGS_PANEL, OWNER_ROBOT_SYNC_SCENE, OWNER_UI_GENERATED_ACCEPTANCE,
    };
    use bevy::{ecs::message::MessageCursor, state::app::StatesPlugin};
    use std::collections::BTreeMap;

    #[test]
    fn same_mode_and_owner_route_is_idempotent() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<AppUiMode>()
            .init_resource::<UiCurrentOwner>()
            .init_resource::<UiBindingValues>()
            .add_message::<GameRouteCommand>()
            .add_message::<UiPanelCommand>()
            .add_message::<UiDocumentRuntimeCommand>()
            .add_systems(Update, handle_game_route_commands);
        app.update();

        app.world_mut()
            .write_message(GameRouteCommand::ChangeMode(AppUiMode::Login));
        app.update();

        assert_eq!(
            app.world().resource::<UiCurrentOwner>().owner,
            Some(OWNER_LOGIN)
        );
        let mut panel_cursor = MessageCursor::<UiPanelCommand>::default();
        assert_eq!(
            panel_cursor
                .read(app.world().resource::<Messages<UiPanelCommand>>())
                .count(),
            0
        );
        let mut runtime_cursor = MessageCursor::<UiDocumentRuntimeCommand>::default();
        assert_eq!(
            runtime_cursor
                .read(app.world().resource::<Messages<UiDocumentRuntimeCommand>>(),)
                .count(),
            0
        );
    }

    #[test]
    fn audio_settings_mode_uses_dedicated_owner() {
        assert_eq!(AppUiMode::AudioSettings.ui_owner(), OWNER_AUDIO_SETTINGS);
    }

    #[test]
    fn main_world_mode_closes_lobby_ui_under_a_dedicated_owner() {
        assert_eq!(AppUiMode::MainWorld.ui_owner(), OWNER_MAIN_WORLD);
        assert_eq!(
            parse_start_screen_mode("main-world"),
            Some(AppUiMode::MainWorld)
        );

        let mut registry = UiAuditScreenRegistry::default();
        register_ui_audit_screen_entries(&mut registry);
        let screen = registry
            .resolve("main-world")
            .expect("main-world route should be registered for audit");
        assert_eq!(screen.owner, OWNER_MAIN_WORLD);
        assert!(screen.recipe.is_some());
    }

    #[test]
    fn leaving_main_world_closes_scene_panels_then_hud_without_a_scene_command() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<AppUiMode>()
            .init_resource::<UiCurrentOwner>()
            .init_resource::<UiBindingValues>()
            .add_message::<GameRouteCommand>()
            .add_message::<UiPanelCommand>()
            .add_message::<UiDocumentRuntimeCommand>()
            .add_systems(Update, handle_game_route_commands);
        app.world_mut()
            .resource_mut::<NextState<AppUiMode>>()
            .set(AppUiMode::MainWorld);
        app.update();
        app.update();

        app.world_mut()
            .write_message(GameRouteCommand::ChangeMode(AppUiMode::Lobby));
        app.update();

        let mut panel_cursor = MessageCursor::<UiPanelCommand>::default();
        let closed_owners = panel_cursor
            .read(app.world().resource::<Messages<UiPanelCommand>>())
            .filter_map(|command| match command {
                UiPanelCommand::CloseAllForOwner(owner) => Some(owner.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            closed_owners,
            [
                OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
                OWNER_MAIN_WORLD_SETTINGS_PANEL.as_str(),
                OWNER_MAIN_WORLD.as_str(),
            ]
        );

        let mut runtime_cursor = MessageCursor::<UiDocumentRuntimeCommand>::default();
        let closed_runtime_owners = runtime_cursor
            .read(app.world().resource::<Messages<UiDocumentRuntimeCommand>>())
            .filter_map(|command| match command {
                UiDocumentRuntimeCommand::SwitchOwner { previous_owner } => {
                    Some(previous_owner.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(closed_runtime_owners, closed_owners);
    }

    #[test]
    fn generated_acceptance_mode_uses_promoted_owner_and_alias() {
        assert_eq!(
            AppUiMode::UiGeneratedAcceptance.ui_owner(),
            OWNER_UI_GENERATED_ACCEPTANCE
        );
        assert_eq!(
            parse_start_screen_mode("ui-generated-acceptance"),
            Some(AppUiMode::UiGeneratedAcceptance)
        );
    }

    #[test]
    fn audio_monitor_mode_uses_dedicated_owner() {
        assert_eq!(AppUiMode::AudioMonitor.ui_owner(), OWNER_AUDIO_MONITOR);
    }

    #[test]
    fn audio_gallery_mode_uses_dedicated_owner() {
        assert_eq!(AppUiMode::AudioGallery.ui_owner(), OWNER_AUDIO_GALLERY);
    }

    #[test]
    fn character_select_mode_uses_dedicated_owner() {
        assert_eq!(
            AppUiMode::CharacterSelect.ui_owner(),
            OWNER_CHARACTER_SELECT
        );
    }

    #[test]
    fn robot_sync_scene_mode_uses_dedicated_owner() {
        assert_eq!(AppUiMode::RobotSyncScene.ui_owner(), OWNER_ROBOT_SYNC_SCENE);
    }

    #[test]
    fn fangyuan_home_mode_uses_dedicated_owner() {
        assert_eq!(AppUiMode::FangyuanHome.ui_owner(), OWNER_FANGYUAN_HOME);
    }

    #[test]
    fn fangyuan_player_preview_mode_uses_dedicated_owner() {
        assert_eq!(
            AppUiMode::FangyuanPlayerPreview.ui_owner(),
            OWNER_FANGYUAN_PLAYER_PREVIEW
        );
    }

    #[test]
    fn start_screen_aliases_include_audio_gallery() {
        assert_eq!(
            parse_start_screen_mode("audio_gallery"),
            Some(AppUiMode::AudioGallery)
        );
        assert_eq!(
            parse_start_screen_mode("audio-gallery"),
            Some(AppUiMode::AudioGallery)
        );
    }

    #[test]
    fn start_screen_aliases_include_character_select() {
        assert_eq!(
            parse_start_screen_mode("character_select"),
            Some(AppUiMode::CharacterSelect)
        );
        assert_eq!(
            parse_start_screen_mode("select-character"),
            Some(AppUiMode::CharacterSelect)
        );
    }

    #[test]
    fn start_screen_aliases_include_fangyuan_home() {
        assert_eq!(
            parse_start_screen_mode("fangyuan_home"),
            Some(AppUiMode::FangyuanHome)
        );
        assert_eq!(
            parse_start_screen_mode("fangyuan-home"),
            Some(AppUiMode::FangyuanHome)
        );
        assert_eq!(
            parse_start_screen_mode("fangyuan"),
            Some(AppUiMode::FangyuanHome)
        );
    }

    #[test]
    fn start_screen_aliases_include_fangyuan_player_preview() {
        assert_eq!(
            parse_start_screen_mode("fangyuan_player_preview"),
            Some(AppUiMode::FangyuanPlayerPreview)
        );
        assert_eq!(
            parse_start_screen_mode("fangyuan-player"),
            Some(AppUiMode::FangyuanPlayerPreview)
        );
    }

    #[test]
    fn start_screen_aliases_include_robot_sync_scene() {
        assert_eq!(
            parse_start_screen_mode("robot_sync_scene"),
            Some(AppUiMode::RobotSyncScene)
        );
        assert_eq!(
            parse_start_screen_mode("robot-sync-scene"),
            Some(AppUiMode::RobotSyncScene)
        );
        assert_eq!(
            parse_start_screen_mode("robot"),
            Some(AppUiMode::RobotSyncScene)
        );
    }

    #[test]
    fn ui_gallery_audit_recipe_registers_scroll_capture_states() {
        let mut registry = UiAuditScreenRegistry::default();
        register_ui_audit_screen_entries(&mut registry);

        let screen = registry
            .resolve("ui-gallery")
            .expect("ui gallery should be registered for audit");
        let recipe = screen.recipe.expect("ui gallery should have audit recipe");

        let states = recipe
            .captures
            .iter()
            .map(|capture| capture.state)
            .collect::<Vec<_>>();
        assert_eq!(
            states,
            vec![
                UiAuditCaptureState::VisualFoundation,
                UiAuditCaptureState::VisualAcceptance,
                UiAuditCaptureState::ImageFit,
                UiAuditCaptureState::ImageModes,
                UiAuditCaptureState::ImageTiling,
                UiAuditCaptureState::ImageAtlas,
                UiAuditCaptureState::Typography,
                UiAuditCaptureState::TypographyOverflow,
                UiAuditCaptureState::Icons,
                UiAuditCaptureState::IconStates,
                UiAuditCaptureState::StyleScopes,
                UiAuditCaptureState::Effects,
                UiAuditCaptureState::Animations,
                UiAuditCaptureState::Components,
                UiAuditCaptureState::ComponentCheckboxes,
                UiAuditCaptureState::ComponentToggles,
                UiAuditCaptureState::ComponentSegmented,
                UiAuditCaptureState::ComponentOverlays,
                UiAuditCaptureState::ComponentTooltip,
                UiAuditCaptureState::Top,
                UiAuditCaptureState::Middle,
                UiAuditCaptureState::Bottom,
            ]
        );

        let target = |state| {
            recipe
                .captures
                .iter()
                .find(|capture| capture.state == state)
                .and_then(|capture| capture.scroll)
                .map(|scroll| scroll.target.as_str())
        };
        assert_eq!(
            target(UiAuditCaptureState::VisualFoundation),
            Some(UiScrollAuditPosition::Top.as_str())
        );
        assert_eq!(
            target(UiAuditCaptureState::VisualAcceptance),
            Some(ANCHOR_UI_GALLERY_VISUAL_ACCEPTANCE.as_str())
        );
        for (state, anchor) in [
            (
                UiAuditCaptureState::ImageModes,
                ANCHOR_UI_GALLERY_IMAGE_MODES,
            ),
            (
                UiAuditCaptureState::ImageTiling,
                ANCHOR_UI_GALLERY_IMAGE_TILING,
            ),
            (
                UiAuditCaptureState::ImageAtlas,
                ANCHOR_UI_GALLERY_IMAGE_ATLAS,
            ),
            (
                UiAuditCaptureState::Typography,
                ANCHOR_UI_GALLERY_TYPOGRAPHY,
            ),
            (
                UiAuditCaptureState::TypographyOverflow,
                ANCHOR_UI_GALLERY_TYPOGRAPHY_OVERFLOW,
            ),
            (UiAuditCaptureState::Icons, ANCHOR_UI_GALLERY_ICONS),
            (
                UiAuditCaptureState::IconStates,
                ANCHOR_UI_GALLERY_ICON_STATES,
            ),
            (
                UiAuditCaptureState::StyleScopes,
                ANCHOR_UI_GALLERY_STYLE_SCOPES,
            ),
            (UiAuditCaptureState::Effects, ANCHOR_UI_GALLERY_EFFECTS),
            (
                UiAuditCaptureState::Components,
                ANCHOR_UI_GALLERY_COMPONENTS,
            ),
            (
                UiAuditCaptureState::ComponentCheckboxes,
                ANCHOR_UI_GALLERY_COMPONENT_CHECKBOXES,
            ),
            (
                UiAuditCaptureState::ComponentToggles,
                ANCHOR_UI_GALLERY_COMPONENT_TOGGLES,
            ),
            (
                UiAuditCaptureState::ComponentSegmented,
                ANCHOR_UI_GALLERY_COMPONENT_SEGMENTED,
            ),
            (
                UiAuditCaptureState::ComponentOverlays,
                ANCHOR_UI_GALLERY_COMPONENT_DROPDOWN,
            ),
            (
                UiAuditCaptureState::ComponentTooltip,
                ANCHOR_UI_GALLERY_COMPONENT_TOOLTIP,
            ),
        ] {
            assert_eq!(target(state), Some(anchor.as_str()));
        }
    }

    #[test]
    fn lobby_audit_recipe_waits_for_owner_document() {
        let mut registry = UiAuditScreenRegistry::default();
        register_ui_audit_screen_entries(&mut registry);

        let screen = registry
            .resolve("lobby")
            .expect("Lobby should be registered for audit");
        let recipe = screen.recipe.expect("Lobby should use a document recipe");

        assert_eq!(recipe.ready, Some(UiAuditReadyCondition::OwnerDocument));
    }

    #[test]
    fn gameplay_hud_audit_recipes_wait_for_owner_documents() {
        let mut registry = UiAuditScreenRegistry::default();
        register_ui_audit_screen_entries(&mut registry);

        for screen_id in [
            "wanfa_touch_ripple",
            "sample_scene",
            "robot_sync_scene",
            "fangyuan_home",
            "fangyuan_player_preview",
        ] {
            let recipe = registry
                .resolve(screen_id)
                .and_then(|screen| screen.recipe)
                .unwrap_or_else(|| panic!("{screen_id} should use a document audit recipe"));
            assert_eq!(recipe.ready, Some(UiAuditReadyCondition::OwnerDocument));
        }
    }

    #[cfg(all(debug_assertions, not(target_os = "android")))]
    #[test]
    fn fangyuan_home_audit_recipe_reaches_scroll_bottom() {
        let mut registry = UiAuditScreenRegistry::default();
        register_ui_audit_screen_entries(&mut registry);

        let recipe = registry
            .resolve("fangyuan_home")
            .and_then(|screen| screen.recipe)
            .expect("Fangyuan Home should use its document audit recipe");
        assert_eq!(
            recipe
                .captures
                .iter()
                .map(|capture| capture.state)
                .collect::<Vec<_>>(),
            vec![UiAuditCaptureState::Initial, UiAuditCaptureState::Bottom]
        );
        assert_eq!(
            recipe.captures[1].scroll.unwrap().target_id,
            SCROLL_FANGYUAN_HOME_MAIN
        );
    }

    #[cfg(all(debug_assertions, not(target_os = "android")))]
    #[test]
    fn audio_settings_audit_recipe_captures_initial_and_bottom() {
        let mut registry = UiAuditScreenRegistry::default();
        register_ui_audit_screen_entries(&mut registry);

        let recipe = registry
            .resolve("audio_settings")
            .and_then(|screen| screen.recipe)
            .expect("Audio Settings should use its document audit recipe");
        assert_eq!(
            recipe
                .captures
                .iter()
                .map(|capture| capture.state)
                .collect::<Vec<_>>(),
            vec![UiAuditCaptureState::Initial, UiAuditCaptureState::Bottom]
        );
        assert_eq!(
            recipe.captures[1].scroll.unwrap().target_id,
            SCROLL_AUDIO_SETTINGS_MAIN
        );
    }

    #[test]
    fn game_registers_declarative_continue_action_and_adapts_it_to_lobby_route() {
        let mut app = App::new();
        app.init_resource::<UiActionRegistry>()
            .add_systems(Startup, register_game_ui_actions);
        app.update();

        let registry = app.world().resource::<UiActionRegistry>();
        let action_id = UiActionId::from_str(DECLARATIVE_CONTINUE_ACTION).unwrap();
        let descriptor = registry
            .descriptor(&action_id)
            .expect("game action should be registered");
        assert_eq!(descriptor.document_id.as_str(), "example.minimal_page");
        assert_eq!(descriptor.owner, OWNER_LOGIN.as_str());

        let business_action = registry
            .descriptor(&UiActionId::from_str(APPROVED_BUSINESS_ACCEPTANCE_ACTION).unwrap())
            .expect("approved business action should be registered");
        assert_eq!(
            business_action.document_id.as_str(),
            APPROVED_BUSINESS_ACCEPTANCE_DOCUMENT
        );
        assert_eq!(
            business_action.owner,
            OWNER_UI_APPROVED_BUSINESS_ACCEPTANCE.as_str()
        );
        assert_eq!(
            business_action.sources,
            [UiNodeId::from_str("acceptance.continue").unwrap()]
                .into_iter()
                .collect()
        );

        let dispatch = UiActionDispatch {
            action: action_id,
            document_id: descriptor.document_id.clone(),
            owner: descriptor.owner.clone(),
            source_node: crate::framework::ui::document::UiNodeId::from_str("page.continue")
                .unwrap(),
            kind: descriptor.kind.clone(),
            params: BTreeMap::new(),
        };
        assert_eq!(
            adapt_registered_ui_action(&dispatch),
            Some(AppUiMode::Lobby)
        );

        let mut spoofed = dispatch.clone();
        spoofed.action = UiActionId::from_str("example.other").unwrap();
        assert_eq!(adapt_registered_ui_action(&spoofed), None);

        let mut spoofed = dispatch.clone();
        spoofed.document_id = UiDocumentId::from_str("other.document").unwrap();
        assert_eq!(adapt_registered_ui_action(&spoofed), None);

        let mut spoofed = dispatch.clone();
        spoofed.owner = OWNER_LOBBY.as_str().to_owned();
        assert_eq!(adapt_registered_ui_action(&spoofed), None);

        let mut spoofed = dispatch.clone();
        spoofed.source_node =
            crate::framework::ui::document::UiNodeId::from_str("page.title").unwrap();
        assert_eq!(adapt_registered_ui_action(&spoofed), None);

        let mut spoofed = dispatch;
        spoofed.kind = UiRegisteredActionKind::Route {
            target: "game.route_login".to_owned(),
        };
        assert_eq!(adapt_registered_ui_action(&spoofed), None);
    }
}
