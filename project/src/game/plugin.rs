use bevy::{prelude::*, ui::IsDefaultUiCamera};

use crate::framework::{audio::AudioPlugin, scene::ScenePlugin};

use super::{
    audio::GameAudioPlugin, authority::AuthorityPlugin, features::touch_ripple::TouchRipplePlugin,
    myserver::MyServerPlugin, scenes::GameScenesPlugin, screens::ScreensPlugin,
};

pub struct GamePlugin;

pub const GLOBAL_UI_CAMERA_ORDER: isize = 1;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            AudioPlugin,
            GameAudioPlugin,
            ScenePlugin,
            GameScenesPlugin,
            MyServerPlugin,
            AuthorityPlugin,
            ScreensPlugin,
            TouchRipplePlugin,
        ))
        .add_systems(Startup, setup_camera);
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::None,
            order: GLOBAL_UI_CAMERA_ORDER,
            ..Default::default()
        },
        IsDefaultUiCamera,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::scene::{SCENE_CAMERA_2D_ORDER, SCENE_CAMERA_3D_ORDER};

    #[test]
    fn global_ui_camera_order_is_above_scene_cameras() {
        assert!(SCENE_CAMERA_3D_ORDER < SCENE_CAMERA_2D_ORDER);
        assert!(SCENE_CAMERA_2D_ORDER < GLOBAL_UI_CAMERA_ORDER);
    }

    #[test]
    fn setup_camera_spawns_overlay_ui_camera() {
        let mut app = App::new();
        app.add_systems(Startup, setup_camera);
        app.update();

        let mut cameras = app
            .world_mut()
            .query_filtered::<(&Camera, &Camera2d), With<IsDefaultUiCamera>>();
        let (camera, _) = cameras.single(app.world()).unwrap();

        assert_eq!(camera.order, GLOBAL_UI_CAMERA_ORDER);
        assert!(matches!(camera.clear_color, ClearColorConfig::None));
    }
}
