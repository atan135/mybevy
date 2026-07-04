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
mod audit;
mod avatar;
mod blueprint;
mod equipment;
mod layout;
mod material_profile;
mod npc;
mod object;
mod object_budget;
mod prefab;
mod primitive;
mod render_assets;
mod skill;
mod static_instance;
mod static_instance_render;
mod static_merge;
mod static_mesh_builder;
mod stats;
mod tiandao;
mod vfx;

pub use asset_path::*;
pub use audit::*;
pub use avatar::*;
pub use blueprint::*;
pub use equipment::*;
pub use layout::*;
pub use material_profile::*;
pub use npc::*;
pub use object::*;
pub use object_budget::*;
pub use prefab::*;
pub use primitive::*;
pub use render_assets::*;
pub use skill::*;
pub use static_instance::*;
pub use static_instance_render::*;
pub use static_merge::*;
pub use static_mesh_builder::*;
pub use stats::*;
pub use tiandao::*;
pub use vfx::*;
