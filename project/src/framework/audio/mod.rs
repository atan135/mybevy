//! Shared audio framework boundary.
//!
//! This module is reserved for reusable audio foundations. Concrete game
//! audio rules, cue catalogs, and screen-specific adapters stay in the game
//! layer or later framework extensions.

mod bank;
mod battle;
mod catalog;
mod catalog_config;
mod command;
mod debug;
mod event;
mod id;
mod lifecycle;
mod loading;
mod metadata;
mod mixer;
mod music;
mod playback;
mod plugin;
pub mod prelude;
mod scene;
mod scope;
mod spatial;
mod ui;

pub use prelude::*;
