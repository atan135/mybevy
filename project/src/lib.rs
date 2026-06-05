use bevy::prelude::*;

mod game;
pub mod myserver;
pub mod network;

#[bevy_main]
pub fn main() {
    run();
}

pub fn run() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(network::NetworkPlugin)
        .add_plugins(myserver::MyServerPlugin)
        .add_plugins(game::GamePlugin)
        .run();
}
