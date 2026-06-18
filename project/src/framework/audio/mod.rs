//! Shared audio framework boundary.
//!
//! This module is reserved for reusable audio foundations. Concrete game
//! audio rules, cue catalogs, and screen-specific adapters stay in the game
//! layer or later framework extensions.

mod id;
mod plugin;
pub mod prelude;
mod scope;

pub use prelude::*;
