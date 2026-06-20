use bevy::prelude::*;

#[derive(Clone, Debug, Default, Resource, PartialEq, Eq)]
pub(in crate::game::features::robot_sync) struct RobotSyncReplayState {
    pub(in crate::game::features::robot_sync) buffered_frame_count: usize,
    pub(in crate::game::features::robot_sync) last_frame_id: Option<u32>,
}

impl RobotSyncReplayState {
    pub(in crate::game::features::robot_sync) fn reset(&mut self) {
        *self = Self::default();
    }
}

pub(in crate::game::features::robot_sync) fn reset_robot_sync_replay(
    state: &mut RobotSyncReplayState,
) {
    state.reset();
}
