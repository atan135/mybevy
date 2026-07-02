//! Fangyuan unified data model boundary.
//!
//! `blueprint` owns first-package RON loading, validation, and compilation from
//! authoring records into runtime data. `object` owns the shared logical root
//! state. `primitive` owns the compiled runtime primitive model stored on
//! gameplay entities. `avatar` owns the gameplay component that binds a
//! blueprint identity and display name to that runtime primitive set.
//!
//! Rendering features should create their own render instance entities from
//! `FangyuanPrimitiveSet`; blueprint records are authoring input and do not
//! carry rendering responsibility.

mod asset_path;
mod avatar;
mod blueprint;
mod layout;
mod object;
mod prefab;
mod primitive;
mod render_assets;
mod stats;

pub use asset_path::*;
pub use avatar::*;
pub use blueprint::*;
pub use layout::*;
pub use object::*;
pub use prefab::*;
pub use primitive::*;
pub use render_assets::*;
pub use stats::*;
