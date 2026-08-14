//! Compatibility facade for the MyServer protocol crate.
//!
//! Existing game code intentionally keeps this module path while the generated protobuf types and
//! packet codec build independently from the Bevy runtime.
pub use myserver_protocol::*;
