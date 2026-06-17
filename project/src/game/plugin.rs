use bevy::prelude::*;

use super::{features::touch_ripple::TouchRipplePlugin, screens::ScreensPlugin};

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((ScreensPlugin, TouchRipplePlugin))
            .add_systems(Startup, setup_camera);
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
