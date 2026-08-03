use bevy::prelude::*;

use crate::framework::scene::prelude::SCENE_CAMERA_3D_ORDER;
use crate::game::navigation::AppUiMode;

const FANGYUAN_PLAYER_PREVIEW_CAMERA_TRANSLATION: Vec3 = Vec3::new(0.0, 2.2, 5.0);
const FANGYUAN_PLAYER_PREVIEW_CAMERA_TARGET: Vec3 = Vec3::new(0.0, 1.0, 0.0);

#[derive(Component)]
pub(super) struct FangyuanPlayerPreviewCamera;

#[derive(Component)]
pub(super) struct FangyuanPlayerPreviewLight;

pub(super) fn setup_fangyuan_player_preview_scene(mut commands: Commands) {
    spawn_fangyuan_player_preview_camera_and_light(&mut commands);
}

fn spawn_fangyuan_player_preview_camera_and_light(commands: &mut Commands) {
    commands.spawn((
        DespawnOnExit(AppUiMode::FangyuanPlayerPreview),
        Camera3d::default(),
        Camera {
            order: SCENE_CAMERA_3D_ORDER,
            clear_color: ClearColorConfig::Default,
            ..default()
        },
        Transform::from_translation(FANGYUAN_PLAYER_PREVIEW_CAMERA_TRANSLATION)
            .looking_at(FANGYUAN_PLAYER_PREVIEW_CAMERA_TARGET, Vec3::Y),
        GlobalTransform::default(),
        FangyuanPlayerPreviewCamera,
        Name::new("FangyuanPlayerPreviewCamera"),
    ));

    commands.spawn((
        DespawnOnExit(AppUiMode::FangyuanPlayerPreview),
        DirectionalLight {
            illuminance: 7_500.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(-2.0, 4.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
        GlobalTransform::default(),
        FangyuanPlayerPreviewLight,
        Name::new("FangyuanPlayerPreviewDirectionalLight"),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_camera_uses_scene_3d_order_and_identity_roll() {
        let mut app = App::new();
        app.add_systems(Startup, setup_fangyuan_player_preview_scene);
        app.update();

        let mut cameras = app
            .world_mut()
            .query_filtered::<(&Camera, &Transform), With<FangyuanPlayerPreviewCamera>>();
        let (camera, transform) = cameras.single(app.world()).unwrap();

        assert_eq!(camera.order, SCENE_CAMERA_3D_ORDER);
        assert_eq!(
            transform.translation,
            FANGYUAN_PLAYER_PREVIEW_CAMERA_TRANSLATION
        );
    }

    #[test]
    fn preview_lighting_spawns_directional_light() {
        let mut app = App::new();
        app.add_systems(Startup, setup_fangyuan_player_preview_scene);
        app.update();

        let mut lights = app
            .world_mut()
            .query_filtered::<&DirectionalLight, With<FangyuanPlayerPreviewLight>>();
        let light = lights.single(app.world()).unwrap();

        assert_eq!(light.illuminance, 7_500.0);
        assert!(!light.shadows_enabled);
    }
}
