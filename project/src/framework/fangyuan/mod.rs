//! Fangyuan primitive data model boundary.
//!
//! Fangyuan player appearance is stored as primitive data on the gameplay
//! entity. Individual primitives are not gameplay entities.

mod avatar;
mod blueprint;
mod primitive;

pub use avatar::*;
pub use blueprint::*;
pub use primitive::*;
