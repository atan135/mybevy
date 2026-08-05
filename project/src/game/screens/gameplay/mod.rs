mod fangyuan_home;
mod fangyuan_player_preview;
pub(in crate::game) mod host;
mod robot_sync_scene;
mod sample_scene;

use bevy::prelude::*;

use crate::framework::fangyuan::FangyuanDebugPanelState;
use crate::framework::ui::{core::binding::UiBindingSystems, document::UiDocumentRuntimeSystems};
use crate::game::navigation::AppUiMode;

pub(super) struct GameplayScreensPlugin;

impl Plugin for GameplayScreensPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FangyuanDebugPanelState>()
            .init_resource::<robot_sync_scene::RobotSyncHudVisibility>()
            .init_resource::<host::GameplayHudHostContract>()
            .add_systems(Startup, host::register_gameplay_hud_contracts);

        app.add_systems(
            OnEnter(AppUiMode::RobotSyncScene),
            host::reset_robot_sync_hud_visibility,
        )
        .add_systems(
            OnEnter(AppUiMode::FangyuanPlayerPreview),
            fangyuan_player_preview::setup_fangyuan_player_preview_scene,
        )
        .add_systems(
            OnExit(AppUiMode::WanfaTouchRipple),
            host::cleanup_gameplay_hud_focus,
        )
        .add_systems(
            OnExit(AppUiMode::SampleScene),
            host::cleanup_gameplay_hud_focus,
        )
        .add_systems(
            OnExit(AppUiMode::RobotSyncScene),
            host::cleanup_gameplay_hud_focus,
        )
        .add_systems(
            OnExit(AppUiMode::FangyuanHome),
            host::cleanup_gameplay_hud_focus,
        )
        .add_systems(
            OnExit(AppUiMode::FangyuanPlayerPreview),
            host::cleanup_gameplay_hud_focus,
        )
        .add_systems(
            Update,
            host::handle_gameplay_hud_document_actions.after(UiDocumentRuntimeSystems::Reconcile),
        )
        .add_systems(
            Update,
            host::recover_from_main_world_hud_failure.after(UiDocumentRuntimeSystems::Reconcile),
        )
        .add_systems(
            Update,
            sample_scene::route_to_lobby_on_sample_scene_exit
                .run_if(in_state(AppUiMode::SampleScene)),
        )
        .add_systems(
            Update,
            (
                robot_sync_scene::update_robot_sync_scene_hud_status,
                robot_sync_scene::sync_robot_sync_hud_visibility_bindings,
                robot_sync_scene::route_to_lobby_on_robot_sync_scene_exit,
            )
                .chain()
                .before(UiBindingSystems::Apply)
                .run_if(in_state(AppUiMode::RobotSyncScene)),
        )
        .add_systems(
            Update,
            (
                fangyuan_home::update_fangyuan_home_hud_status,
                fangyuan_home::update_fangyuan_home_debug_panel,
                fangyuan_home::route_to_lobby_on_fangyuan_home_exit,
            )
                .chain()
                .before(UiBindingSystems::Apply)
                .run_if(in_state(AppUiMode::FangyuanHome)),
        );

        #[cfg(all(debug_assertions, not(target_os = "android")))]
        app.add_systems(
            OnEnter(AppUiMode::RobotSyncScene),
            host::prepare_gameplay_hud_audit_fixture.after(host::reset_robot_sync_hud_visibility),
        )
        .add_systems(
            OnEnter(AppUiMode::FangyuanHome),
            host::prepare_gameplay_hud_audit_fixture,
        )
        .add_systems(
            Update,
            host::mark_fangyuan_home_audit_scroll
                .after(UiDocumentRuntimeSystems::Reconcile)
                .run_if(in_state(AppUiMode::FangyuanHome)),
        );
    }
}
