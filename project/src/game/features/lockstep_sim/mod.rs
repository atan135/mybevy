pub(in crate::game) mod adapter;
mod config;
mod plugin;
mod snapshot;
mod state;
mod sync;

pub(in crate::game) use plugin::LockstepSimPlugin;
