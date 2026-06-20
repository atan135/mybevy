use bevy::prelude::*;

#[derive(Clone, Debug, Default, Resource, PartialEq, Eq)]
pub(in crate::game::features::robot_sync) struct RobotSyncVisualState {
    pub(in crate::game::features::robot_sync) tracked_robot_entities: usize,
}

impl RobotSyncVisualState {
    pub(in crate::game::features::robot_sync) fn clear(&mut self) {
        self.tracked_robot_entities = 0;
    }
}

pub(in crate::game::features::robot_sync) fn clear_robot_sync_visuals(
    state: &mut RobotSyncVisualState,
) {
    state.clear();
}
