mod detail;
mod host;
mod list;
mod login_reward;

use bevy::prelude::*;

use crate::framework::ui::{core::binding::UiBindingSystems, document::UiDocumentRuntimeSystems};
use crate::game::myserver::MyServerUpdateSet;

pub(in crate::game) use host::{
    ACTIVITY_DETAIL_ROUTE, ACTIVITY_LIST_ROUTE, ActivityScreenCommand, ActivityScreenState,
    LOGIN_REWARD_DETAIL_ROUTE,
};

pub(super) struct ActivityScreensPlugin;

impl Plugin for ActivityScreensPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<host::ActivityScreenHostContract>()
            .init_resource::<ActivityScreenState>()
            .init_resource::<login_reward::LoginRewardClaimTracker>()
            .add_message::<ActivityScreenCommand>()
            .add_systems(Startup, host::register_activity_screen_contract)
            .add_systems(
                Update,
                (
                    host::handle_activity_screen_commands,
                    host::apply_activity_screen_events.after(MyServerUpdateSet::AutoClient),
                    host::track_activity_screen_host_events
                        .after(UiDocumentRuntimeSystems::Reconcile),
                    list::handle_activity_list_document_actions
                        .after(UiDocumentRuntimeSystems::Reconcile),
                    detail::handle_activity_detail_document_actions
                        .after(UiDocumentRuntimeSystems::Reconcile),
                    login_reward::handle_login_reward_document_actions
                        .after(UiDocumentRuntimeSystems::Reconcile),
                    login_reward::apply_login_reward_activity_events
                        .after(MyServerUpdateSet::AutoClient),
                ),
            )
            .add_systems(
                Update,
                list::sync_activity_list_bindings
                    .after(MyServerUpdateSet::AutoClient)
                    .before(UiBindingSystems::Apply),
            )
            .add_systems(
                Update,
                detail::sync_activity_detail_bindings
                    .after(MyServerUpdateSet::AutoClient)
                    .before(UiBindingSystems::Apply),
            )
            .add_systems(
                Update,
                login_reward::sync_login_reward_bindings
                    .after(MyServerUpdateSet::AutoClient)
                    .before(UiBindingSystems::Apply),
            );
    }
}
