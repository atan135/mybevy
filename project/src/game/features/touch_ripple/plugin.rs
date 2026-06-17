use bevy::prelude::*;

use crate::{framework::ui::core::UiInputSystems, game::navigation::AppUiMode};

use super::{
    config::{TouchLaunchMode, TouchSyncConfig},
    input::{TouchInputState, capture_local_touch_input},
    sync::{
        TouchMyServerJoinState, apply_authority_touch_frames, follow_touch_myserver_events,
        release_idle_remote_touches, reset_touch_sync_state, send_local_touch_input,
        start_touch_sync,
    },
    visual::{
        TouchReplayState, animate_background, animate_released_discs, animate_ripples,
        animate_touch_players, background_color_at, resize_background, setup_touch_assets,
        setup_touch_background, spawn_drag_ripples,
    },
};

pub(in crate::game) struct TouchRipplePlugin;

impl Plugin for TouchRipplePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(background_color_at(0.0)))
            .init_resource::<TouchSyncConfig>()
            .init_resource::<TouchInputState>()
            .init_resource::<TouchReplayState>()
            .init_resource::<TouchMyServerJoinState>()
            .init_resource::<TouchLaunchMode>()
            .add_systems(Startup, setup_touch_assets)
            .add_systems(
                OnEnter(AppUiMode::WanfaTouchRipple),
                (setup_touch_background, start_touch_sync).chain(),
            )
            .add_systems(OnExit(AppUiMode::WanfaTouchRipple), reset_touch_sync_state)
            .add_systems(
                Update,
                (
                    animate_background,
                    capture_local_touch_input,
                    follow_touch_myserver_events,
                    send_local_touch_input,
                    apply_authority_touch_frames,
                    release_idle_remote_touches,
                    animate_touch_players,
                    spawn_drag_ripples,
                    animate_ripples,
                    animate_released_discs,
                    resize_background,
                )
                    .chain()
                    .after(UiInputSystems::Update)
                    .run_if(in_state(AppUiMode::WanfaTouchRipple)),
            );
    }
}
