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
mod bake;
mod blueprint;
mod cache;
mod cache_authority;
mod chunk;
mod chunk_loading;
mod debug_metrics;
mod epoch_inheritance;
mod equipment;
mod fallback;
mod identity;
mod layout;
mod lod;
mod lod_integration;
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
mod streaming_update;
mod tiandao;
mod vfx;

pub use asset_path::*;
pub use audit::*;
pub use avatar::*;
pub use bake::*;
pub use blueprint::*;
pub use cache::*;
pub use cache_authority::*;
pub use chunk::*;
pub use chunk_loading::*;
pub use debug_metrics::*;
pub use epoch_inheritance::*;
pub use equipment::*;
pub use fallback::*;
pub use identity::*;
pub use layout::*;
pub use lod::*;
pub use lod_integration::*;
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
pub use streaming_update::*;
pub use tiandao::*;
pub use vfx::*;
