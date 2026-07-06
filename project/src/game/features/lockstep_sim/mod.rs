pub(in crate::game) mod adapter;
mod config;
mod input;
mod payload;
mod plugin;
mod replay;
mod snapshot;
mod state;
mod sync;

pub(in crate::game) use plugin::LockstepSimPlugin;
