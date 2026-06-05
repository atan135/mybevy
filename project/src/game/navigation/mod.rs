use bevy::prelude::*;

pub(super) struct NavigationPlugin;

impl Plugin for NavigationPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppScreen>()
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
