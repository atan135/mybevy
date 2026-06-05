use bevy::prelude::*;
use std::env;

pub(super) struct NavigationPlugin;

impl Plugin for NavigationPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppScreen>()
            .add_systems(Startup, setup_start_screen)
            .add_systems(Update, handle_route_buttons);
    }
}

#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
pub(super) enum AppScreen {
    #[default]
    Login,
    GameList,
    TouchRipple,
}

#[derive(Component)]
pub(super) struct RouteButton {
    pub(super) target: AppScreen,
}

fn setup_start_screen(mut next_screen: ResMut<NextState<AppScreen>>) {
    let Ok(value) = env::var("TOUCH_START_SCREEN") else {
        return;
    };

    let screen = match value.trim().to_ascii_lowercase().as_str() {
        "touch" | "touch_ripple" | "touch-ripple" => AppScreen::TouchRipple,
        "game_list" | "game-list" | "list" => AppScreen::GameList,
        "login" => AppScreen::Login,
        _ => return,
    };
    next_screen.set(screen);
}

fn handle_route_buttons(
    mut next_screen: ResMut<NextState<AppScreen>>,
    buttons: Query<(&Interaction, &RouteButton), (Changed<Interaction>, With<Button>)>,
) {
    for (interaction, route_button) in &buttons {
        if *interaction == Interaction::Pressed {
            next_screen.set(route_button.target);
        }
    }
}
