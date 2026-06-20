use bevy::prelude::*;

#[derive(Clone, Debug, Default, Resource, PartialEq, Eq)]
pub(in crate::game::features::robot_sync) struct RobotSyncBotState {
    pub(in crate::game::features::robot_sync) local_bot_slots: usize,
}

impl RobotSyncBotState {
    pub(in crate::game::features::robot_sync) fn clear(&mut self) {
        self.local_bot_slots = 0;
    }
}

pub(in crate::game::features::robot_sync) fn clear_robot_sync_bots(state: &mut RobotSyncBotState) {
    state.clear();
}
