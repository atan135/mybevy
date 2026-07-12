mod widgets;

use bevy::prelude::*;
use std::env;

use crate::framework::ui::{
    audit::{
        UiAuditCaptureRecipe, UiAuditCaptureState, UiAuditRecipe, UiAuditRouteCommand,
        UiAuditScreen, UiAuditScreenRecipe, UiAuditScreenRegistry,
    },
    core::{UiCurrentOwner, UiOwnerId, UiPanelCommand, UiPanelSystems},
    widgets::{UiButtonEvent, UiButtonEventKind, UiScrollAuditPosition},
};
use crate::game::ui_ids::{
    ANCHOR_UI_GALLERY_ANIMATIONS, ANCHOR_UI_GALLERY_EFFECTS, ANCHOR_UI_GALLERY_ICON_STATES,
    ANCHOR_UI_GALLERY_ICONS, ANCHOR_UI_GALLERY_IMAGE_ATLAS, ANCHOR_UI_GALLERY_IMAGE_MODES,
    ANCHOR_UI_GALLERY_IMAGE_TILING, ANCHOR_UI_GALLERY_STYLE_SCOPES, ANCHOR_UI_GALLERY_TYPOGRAPHY,
    ANCHOR_UI_GALLERY_TYPOGRAPHY_OVERFLOW, OWNER_AUDIO_GALLERY, OWNER_AUDIO_MONITOR,
    OWNER_AUDIO_SETTINGS, OWNER_CHARACTER_SELECT, OWNER_FANGYUAN_HOME,
    OWNER_FANGYUAN_PLAYER_PREVIEW, OWNER_LOBBY, OWNER_LOGIN, OWNER_ROBOT_SYNC_SCENE,
    OWNER_SAMPLE_SCENE, OWNER_TOUCH_RIPPLE, OWNER_UI_GALLERY, SCROLL_UI_GALLERY_MAIN,
};

pub(in crate::game) use widgets::{game_panel_root, secondary_route_button_key};

pub(super) struct NavigationPlugin;

impl Plugin for NavigationPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppUiMode>()
            .add_message::<GameRouteCommand>()
            .add_systems(Startup, (register_ui_audit_screens, setup_start_mode));
        app.configure_sets(
            Update,
            GameRouteSystems::Commands.before(UiPanelSystems::Commands),
        )
        .add_systems(
            Update,
            (
                handle_route_buttons,
                handle_ui_audit_route_commands,
                handle_game_route_commands,
            )
                .chain()
                .in_set(GameRouteSystems::Commands),
        );
    }
}

#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
pub(super) enum AppUiMode {
    #[default]
    Login,
    CharacterSelect,
    Lobby,
    AudioSettings,
    AudioMonitor,
    AudioGallery,
    WanfaTouchRipple,
    UiGallery,
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
            Self::AudioSettings => OWNER_AUDIO_SETTINGS,
            Self::AudioMonitor => OWNER_AUDIO_MONITOR,
            Self::AudioGallery => OWNER_AUDIO_GALLERY,
            Self::WanfaTouchRipple => OWNER_TOUCH_RIPPLE,
            Self::UiGallery => OWNER_UI_GALLERY,
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
            Self::AudioSettings => "audio_settings",
            Self::AudioMonitor => "audio_monitor",
            Self::AudioGallery => "audio_gallery",
            Self::WanfaTouchRipple => "wanfa_touch_ripple",
            Self::UiGallery => "ui_gallery",
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
) {
    for command in audit_route_commands.read() {
        let Some(mode) = parse_start_screen_mode(&command.screen) else {
            warn!(
                "ui audit route ignored unknown screen alias: {}",
                command.screen
            );
            continue;
        };
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
    }
}

fn handle_game_route_commands(
    mut route_commands: MessageReader<GameRouteCommand>,
    mut next_mode: ResMut<NextState<AppUiMode>>,
    current_mode: Res<State<AppUiMode>>,
    mut current_owner: ResMut<UiCurrentOwner>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
) {
    current_owner.owner = Some(current_mode.get().ui_owner());

    for command in route_commands.read() {
        match command {
            GameRouteCommand::ChangeMode(mode) => {
                panel_commands.write(UiPanelCommand::CloseAllForOwner(
                    current_mode.get().ui_owner(),
                ));
                current_owner.owner = Some(mode.ui_owner());
                next_mode.set(*mode);
            }
        }
    }
}

fn setup_start_mode(mut next_mode: ResMut<NextState<AppUiMode>>) {
    let Ok(value) = env::var("TOUCH_START_SCREEN") else {
        return;
    };

    let Some(mode) = parse_start_screen_mode(&value) else {
        return;
    };
    next_mode.set(mode);
}

pub(crate) fn parse_start_screen_mode(value: &str) -> Option<AppUiMode> {
    all_app_ui_modes().into_iter().find(|mode| {
        mode.aliases()
            .iter()
            .any(|alias| alias.eq_ignore_ascii_case(value.trim()))
    })
}

fn register_ui_audit_screens(mut registry: ResMut<UiAuditScreenRegistry>) {
    register_ui_audit_screen_entries(&mut registry);
}

fn register_ui_audit_screen_entries(registry: &mut UiAuditScreenRegistry) {
    for mode in all_app_ui_modes() {
        let screen = UiAuditScreen::new(mode.canonical_screen(), mode.aliases(), mode.ui_owner());
        if mode == AppUiMode::UiGallery {
            registry.register_recipe(UiAuditScreenRecipe::new(
                screen.with_recipe(UiAuditRecipe::new(UI_GALLERY_AUDIT_CAPTURES)),
            ));
        } else {
            registry.register(screen);
        }
    }
}

const UI_GALLERY_AUDIT_CAPTURES: &[UiAuditCaptureRecipe] = &[
    UiAuditCaptureRecipe::scroll(
        UiAuditCaptureState::VisualFoundation,
        SCROLL_UI_GALLERY_MAIN,
        UiScrollAuditPosition::Top,
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

fn all_app_ui_modes() -> [AppUiMode; 12] {
    [
        AppUiMode::Login,
        AppUiMode::CharacterSelect,
        AppUiMode::Lobby,
        AppUiMode::AudioSettings,
        AppUiMode::AudioMonitor,
        AppUiMode::AudioGallery,
        AppUiMode::WanfaTouchRipple,
        AppUiMode::UiGallery,
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
        OWNER_FANGYUAN_PLAYER_PREVIEW, OWNER_ROBOT_SYNC_SCENE, SCROLL_UI_GALLERY_MAIN,
    };

    #[test]
    fn audio_settings_mode_uses_dedicated_owner() {
        assert_eq!(AppUiMode::AudioSettings.ui_owner(), OWNER_AUDIO_SETTINGS);
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

        assert_eq!(recipe.captures.len(), 15);
        assert_eq!(
            recipe.captures[0].state,
            UiAuditCaptureState::VisualFoundation
        );
        assert_eq!(recipe.captures[1].state, UiAuditCaptureState::ImageFit);
        assert_eq!(recipe.captures[2].state, UiAuditCaptureState::ImageModes);
        assert_eq!(recipe.captures[3].state, UiAuditCaptureState::ImageTiling);
        assert_eq!(recipe.captures[4].state, UiAuditCaptureState::ImageAtlas);
        assert_eq!(recipe.captures[5].state, UiAuditCaptureState::Typography);
        assert_eq!(
            recipe.captures[6].state,
            UiAuditCaptureState::TypographyOverflow
        );
        assert_eq!(recipe.captures[7].state, UiAuditCaptureState::Icons);
        assert_eq!(recipe.captures[8].state, UiAuditCaptureState::IconStates);
        assert_eq!(recipe.captures[9].state, UiAuditCaptureState::StyleScopes);
        assert_eq!(recipe.captures[10].state, UiAuditCaptureState::Effects);
        assert_eq!(recipe.captures[11].state, UiAuditCaptureState::Animations);
        assert_eq!(recipe.captures[12].state, UiAuditCaptureState::Top);
        assert_eq!(recipe.captures[13].state, UiAuditCaptureState::Middle);
        assert_eq!(recipe.captures[14].state, UiAuditCaptureState::Bottom);
        assert_eq!(
            recipe.captures[0].scroll.map(|scroll| scroll.target_id),
            Some(SCROLL_UI_GALLERY_MAIN)
        );
        assert_eq!(
            recipe.captures[0]
                .scroll
                .map(|scroll| scroll.target.as_str()),
            Some(UiScrollAuditPosition::Top.as_str())
        );
        assert_eq!(
            recipe.captures[2]
                .scroll
                .map(|scroll| scroll.target.as_str()),
            Some(ANCHOR_UI_GALLERY_IMAGE_MODES.as_str())
        );
        assert_eq!(
            recipe.captures[3]
                .scroll
                .map(|scroll| scroll.target.as_str()),
            Some(ANCHOR_UI_GALLERY_IMAGE_TILING.as_str())
        );
        assert_eq!(
            recipe.captures[5]
                .scroll
                .map(|scroll| scroll.target.as_str()),
            Some(ANCHOR_UI_GALLERY_TYPOGRAPHY.as_str())
        );
        assert_eq!(
            recipe.captures[6]
                .scroll
                .map(|scroll| scroll.target.as_str()),
            Some(ANCHOR_UI_GALLERY_TYPOGRAPHY_OVERFLOW.as_str())
        );
        assert_eq!(
            recipe.captures[7]
                .scroll
                .map(|scroll| scroll.target.as_str()),
            Some(ANCHOR_UI_GALLERY_ICONS.as_str())
        );
        assert_eq!(
            recipe.captures[8]
                .scroll
                .map(|scroll| scroll.target.as_str()),
            Some(ANCHOR_UI_GALLERY_ICON_STATES.as_str())
        );
        assert_eq!(
            recipe.captures[9]
                .scroll
                .map(|scroll| scroll.target.as_str()),
            Some(ANCHOR_UI_GALLERY_STYLE_SCOPES.as_str())
        );
        assert_eq!(
            recipe.captures[10]
                .scroll
                .map(|scroll| scroll.target.as_str()),
            Some(ANCHOR_UI_GALLERY_EFFECTS.as_str())
        );
        assert_eq!(
            recipe.captures[4]
                .scroll
                .map(|scroll| scroll.target.as_str()),
            Some(ANCHOR_UI_GALLERY_IMAGE_ATLAS.as_str())
        );
    }
}
