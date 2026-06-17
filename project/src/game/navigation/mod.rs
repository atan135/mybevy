mod widgets;

use bevy::prelude::*;
use std::env;

use crate::framework::ui::{
    core::{UiCurrentOwner, UiOwnerId, UiPanelCommand, UiPanelSystems},
    widgets::{DisabledButton, LoadingButton},
};
use crate::game::ui_ids::{OWNER_LOBBY, OWNER_LOGIN, OWNER_TOUCH_RIPPLE, OWNER_UI_GALLERY};

pub(in crate::game) use widgets::{primary_route_button_key, secondary_route_button_key};

pub(super) struct NavigationPlugin;

impl Plugin for NavigationPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppUiMode>()
            .add_message::<GameRouteCommand>()
            .add_systems(Startup, setup_start_mode);
        app.configure_sets(
            Update,
            GameRouteSystems::Commands.before(UiPanelSystems::Commands),
        )
        .add_systems(
            Update,
            (handle_route_buttons, handle_game_route_commands)
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
    WanfaTouchRipple,
    UiGallery,
}

impl AppUiMode {
    pub(super) const fn ui_owner(self) -> UiOwnerId {
        match self {
            Self::Login => OWNER_LOGIN,
            Self::Lobby => OWNER_LOBBY,
            Self::WanfaTouchRipple => OWNER_TOUCH_RIPPLE,
            Self::UiGallery => OWNER_UI_GALLERY,
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
    buttons: Query<
        (&Interaction, &RouteButton),
        (
            Changed<Interaction>,
            With<Button>,
            Without<DisabledButton>,
            Without<LoadingButton>,
        ),
    >,
) {
    for (interaction, route_button) in &buttons {
        if *interaction == Interaction::Pressed {
            route_commands.write(GameRouteCommand::ChangeMode(route_button.target));
        }
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

    let mode = match value.trim().to_ascii_lowercase().as_str() {
        "wanfa_touch_ripple" | "wanfa-touch-ripple" | "touch" | "touch_ripple" | "touch-ripple" => {
            AppUiMode::WanfaTouchRipple
        }
        "lobby" | "game_list" | "game-list" | "list" => AppUiMode::Lobby,
        "ui_gallery" | "ui-gallery" | "gallery" => AppUiMode::UiGallery,
        "login" => AppUiMode::Login,
        _ => return,
    };
    next_mode.set(mode);
}
