mod widgets;

use bevy::prelude::*;
use std::env;

use crate::framework::ui::{
    audit::{UiAuditRouteCommand, UiAuditScreen, UiAuditScreenRegistry},
    core::{UiCurrentOwner, UiOwnerId, UiPanelCommand, UiPanelSystems},
    widgets::{UiButtonEvent, UiButtonEventKind},
};
use crate::game::ui_ids::{
    OWNER_AUDIO_GALLERY, OWNER_AUDIO_MONITOR, OWNER_AUDIO_SETTINGS, OWNER_FANGYUAN_HOME,
    OWNER_LOBBY, OWNER_LOGIN, OWNER_ROBOT_SYNC_SCENE, OWNER_SAMPLE_SCENE, OWNER_TOUCH_RIPPLE,
    OWNER_UI_GALLERY,
};

pub(in crate::game) use widgets::{
    game_panel_root, primary_route_button_key, secondary_route_button_key,
};

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
    Lobby,
    AudioSettings,
    AudioMonitor,
    AudioGallery,
    WanfaTouchRipple,
    UiGallery,
    SampleScene,
    RobotSyncScene,
    FangyuanHome,
}

impl AppUiMode {
    pub(crate) const fn ui_owner(self) -> UiOwnerId {
        match self {
            Self::Login => OWNER_LOGIN,
            Self::Lobby => OWNER_LOBBY,
            Self::AudioSettings => OWNER_AUDIO_SETTINGS,
            Self::AudioMonitor => OWNER_AUDIO_MONITOR,
            Self::AudioGallery => OWNER_AUDIO_GALLERY,
            Self::WanfaTouchRipple => OWNER_TOUCH_RIPPLE,
            Self::UiGallery => OWNER_UI_GALLERY,
            Self::SampleScene => OWNER_SAMPLE_SCENE,
            Self::RobotSyncScene => OWNER_ROBOT_SYNC_SCENE,
            Self::FangyuanHome => OWNER_FANGYUAN_HOME,
        }
    }

    pub(crate) const fn canonical_screen(self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::Lobby => "lobby",
            Self::AudioSettings => "audio_settings",
            Self::AudioMonitor => "audio_monitor",
            Self::AudioGallery => "audio_gallery",
            Self::WanfaTouchRipple => "wanfa_touch_ripple",
            Self::UiGallery => "ui_gallery",
            Self::SampleScene => "sample_scene",
            Self::RobotSyncScene => "robot_sync_scene",
            Self::FangyuanHome => "fangyuan_home",
        }
    }

    pub(crate) const fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::Login => &["login"],
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
    for mode in all_app_ui_modes() {
        registry.register(UiAuditScreen::new(
            mode.canonical_screen(),
            mode.aliases(),
            mode.ui_owner(),
        ));
    }
}

fn all_app_ui_modes() -> [AppUiMode; 10] {
    [
        AppUiMode::Login,
        AppUiMode::Lobby,
        AppUiMode::AudioSettings,
        AppUiMode::AudioMonitor,
        AppUiMode::AudioGallery,
        AppUiMode::WanfaTouchRipple,
        AppUiMode::UiGallery,
        AppUiMode::SampleScene,
        AppUiMode::RobotSyncScene,
        AppUiMode::FangyuanHome,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ui_ids::{
        OWNER_AUDIO_GALLERY, OWNER_AUDIO_SETTINGS, OWNER_FANGYUAN_HOME, OWNER_ROBOT_SYNC_SCENE,
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
    fn robot_sync_scene_mode_uses_dedicated_owner() {
        assert_eq!(AppUiMode::RobotSyncScene.ui_owner(), OWNER_ROBOT_SYNC_SCENE);
    }

    #[test]
    fn fangyuan_home_mode_uses_dedicated_owner() {
        assert_eq!(AppUiMode::FangyuanHome.ui_owner(), OWNER_FANGYUAN_HOME);
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
}
