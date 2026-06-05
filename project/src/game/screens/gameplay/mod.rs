mod touch_ripple;

use bevy::prelude::*;

use crate::game::navigation::AppScreen;

pub(super) struct GameplayScreensPlugin;

impl Plugin for GameplayScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppScreen::TouchRipple),
            touch_ripple::setup_touch_ripple_overlay,
        );
    }
}
