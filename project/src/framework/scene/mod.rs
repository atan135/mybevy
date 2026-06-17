//! Shared scene framework boundary.
//!
//! This module contains reusable scene capabilities only: scene identity,
//! commands, events, registration metadata, roots, loading state, camera data,
//! spawn/anchor data, triggers, and diagnostics. Concrete gameplay scenes,
//! level content, feature state, and screen-specific UI stay in the game layer.

mod authority;
mod camera;
mod command;
mod debug;
mod event;
mod id;
mod lifecycle;
mod loading;
mod manifest;
mod plugin;
pub mod prelude;
mod registry;
mod root;
mod spawn;
mod streaming;
mod trigger;

pub use prelude::*;
