use bevy::prelude::*;
use std::env;

use crate::game::ui::core::UiOwnerId;
use crate::game::ui_ids::{OWNER_LOBBY, OWNER_LOGIN, OWNER_TOUCH_RIPPLE, OWNER_UI_GALLERY};

pub(super) struct NavigationPlugin;

impl Plugin for NavigationPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppUiMode>()
            .add_systems(Startup, setup_start_mode);
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
