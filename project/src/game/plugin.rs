use bevy::prelude::*;

use crate::framework::scene::ScenePlugin;

use super::{
    authority::AuthorityPlugin, features::touch_ripple::TouchRipplePlugin,
    myserver::MyServerPlugin, screens::ScreensPlugin,
};

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ScenePlugin,
            MyServerPlugin,
            AuthorityPlugin,
            ScreensPlugin,
            TouchRipplePlugin,
        ))
        .add_systems(Startup, setup_camera);
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
