use bevy::{asset::AssetPlugin, prelude::*};

pub mod authority;
mod game;
pub mod myserver;
pub mod network;

#[bevy_main]
pub fn main() {
    run();
}

pub fn run() {
    App::new()
        .add_plugins(DefaultPlugins.set(project_asset_plugin()))
        .add_plugins(network::NetworkPlugin)
        .add_plugins(authority::AuthorityPlugin)
        .add_plugins(myserver::MyServerPlugin)
        .add_plugins(game::GamePlugin)
        .run();
}

fn project_asset_plugin() -> AssetPlugin {
    #[cfg(target_os = "android")]
    {
        AssetPlugin::default()
    }

    #[cfg(not(target_os = "android"))]
    {
        AssetPlugin {
            file_path: format!("{}/assets", env!("CARGO_MANIFEST_DIR")),
            ..default()
        }
    }
}
