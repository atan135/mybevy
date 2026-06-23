mod bot;
mod config;
pub(in crate::game) mod coordinates;
mod hud;
mod plugin;
mod state;
mod sync;
mod visual;

pub(in crate::game) use config::RobotSyncConfig;
pub(in crate::game) use hud::{format_robot_sync_hud_status, robot_sync_hud_snapshot};
pub(in crate::game) use plugin::RobotSyncPlugin;
pub(in crate::game) use state::RobotSyncSceneState;
pub(in crate::game) use sync::RobotSyncReplayState;
