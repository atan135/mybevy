mod robot_sync_scene;
mod sample_scene;
mod touch_ripple;

use bevy::prelude::*;

use crate::game::navigation::AppUiMode;

pub(super) struct GameplayScreensPlugin;

impl Plugin for GameplayScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppUiMode::WanfaTouchRipple),
            touch_ripple::setup_touch_ripple_overlay,
        )
        .add_systems(
            OnEnter(AppUiMode::SampleScene),
            sample_scene::setup_sample_scene_hud,
        )
        .add_systems(
            OnEnter(AppUiMode::RobotSyncScene),
            robot_sync_scene::setup_robot_sync_scene_hud,
        )
        .add_systems(
            Update,
            (
                sample_scene::handle_sample_scene_hud_buttons,
                sample_scene::route_to_lobby_on_sample_scene_exit,
            )
                .chain()
                .run_if(in_state(AppUiMode::SampleScene)),
        )
        .add_systems(
            Update,
            (
                robot_sync_scene::update_robot_sync_scene_hud_status,
                robot_sync_scene::handle_robot_sync_scene_hud_buttons,
                robot_sync_scene::sync_robot_sync_hud_visibility,
                robot_sync_scene::route_to_lobby_on_robot_sync_scene_exit,
            )
                .chain()
                .run_if(in_state(AppUiMode::RobotSyncScene)),
        );
    }
}
