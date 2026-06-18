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
            Update,
            (
                sample_scene::handle_sample_scene_hud_buttons,
                sample_scene::route_to_lobby_on_sample_scene_exit,
            )
                .chain()
                .run_if(in_state(AppUiMode::SampleScene)),
        );
    }
}
