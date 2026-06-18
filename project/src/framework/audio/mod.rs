//! Shared audio framework boundary.
//!
//! This module is reserved for reusable audio foundations. Concrete game
//! audio rules, cue catalogs, and screen-specific adapters stay in the game
//! layer or later framework extensions.

mod catalog;
mod command;
mod debug;
mod event;
mod id;
mod mixer;
mod music;
mod playback;
mod plugin;
pub mod prelude;
mod scope;
mod ui;

pub use prelude::*;
