mod plugin;
pub mod protocol;
mod types;

pub use plugin::MyServerPlugin;
pub use types::{
    DEFAULT_AUTH_HTTP_BASE_URL, DEFAULT_GAME_PROXY_HOST, DEFAULT_GAME_PROXY_KCP_PORT,
    DEFAULT_GAME_PROXY_TCP_FALLBACK_PORT, DEFAULT_REQUEST_TIMEOUT, LoginSession,
    MovementClientState, MyServerAutoClientConfig, MyServerAutoClientState, MyServerCommand,
    MyServerConfig, MyServerEvent, MyServerSession,
};
