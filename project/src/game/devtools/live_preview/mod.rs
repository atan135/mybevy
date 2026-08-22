//! Game-owned adapter boundary for live preview.
//!
//! Implementations belong to the game layer and may read game facts, then
//! return sanitized framework DTOs. The framework core never names these
//! traits' concrete game sources.

mod authority;
mod network;
mod player;

pub use authority::AuthorityPreviewAdapter;
pub(crate) use network::GameNetworkLivePreviewPlugin;
pub use network::NetworkPreviewAdapter;
pub(crate) use player::GameLivePreviewPlugin;
pub use player::PlayerPreviewAdapter;
