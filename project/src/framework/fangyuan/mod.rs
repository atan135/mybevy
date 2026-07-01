//! Fangyuan unified data model boundary.
//!
//! `blueprint` owns first-package RON loading, validation, and compilation from
//! authoring records into runtime data. `primitive` owns the compiled runtime
//! primitive model stored on gameplay entities. `avatar` owns the gameplay
//! component that binds a blueprint identity and display name to that runtime
//! primitive set.
//!
//! Rendering features should create their own render instance entities from
//! `FangyuanPrimitiveSet`; blueprint records are authoring input and do not
//! carry rendering responsibility.

mod avatar;
mod blueprint;
mod primitive;

pub use avatar::*;
pub use blueprint::*;
pub use primitive::*;
