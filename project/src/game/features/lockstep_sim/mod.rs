pub(in crate::game) mod adapter;
mod combat_events;
mod config;
mod input;
mod payload;
mod plugin;
mod replay;
mod snapshot;
mod state;
mod sync;
mod visual;

pub(in crate::game) use plugin::LockstepSimPlugin;
