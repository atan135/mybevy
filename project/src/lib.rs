use bevy::{asset::AssetPlugin, prelude::*, window::WindowResolution};

#[cfg(not(target_os = "android"))]
mod config;
pub mod framework;
mod game;

#[bevy_main]
pub fn main() {
    run();
}

pub fn run() {
    let default_plugins = DefaultPlugins.set(project_asset_plugin());

    #[cfg(not(target_os = "android"))]
    let window_config = config::window::resolve_from_env_args();

    #[cfg(not(target_os = "android"))]
    let default_plugins = default_plugins.set(project_window_plugin(&window_config));

    let mut app = App::new();
    app.add_plugins(default_plugins)
        .add_plugins(framework::network::NetworkPlugin);

    #[cfg(not(target_os = "android"))]
    app.insert_resource(window_config);

    app.add_plugins(game::GamePlugin).run();
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

#[cfg(not(target_os = "android"))]
fn project_window_plugin(window_config: &config::window::WindowStartupConfig) -> WindowPlugin {
    for warning in &window_config.warnings {
        eprintln!("window config warning: {warning}");
    }

    eprintln!(
        "window config: device {}x{} scale {:.2}, logical {:.1}x{:.1}, preview {:.2}, physical window {}x{}",
        window_config.size.width,
        window_config.size.height,
        window_config.device_scale,
        window_config.logical_width(),
        window_config.logical_height(),
        window_config.preview_scale,
        window_config.physical_size().width,
        window_config.physical_size().height
    );

    let physical_size = window_config.physical_size();
    WindowPlugin {
        primary_window: Some(Window {
            resolution: WindowResolution::new(physical_size.width, physical_size.height)
                .with_scale_factor_override(window_config.scale_factor_override()),
            ..default()
        }),
        ..default()
    }
}
