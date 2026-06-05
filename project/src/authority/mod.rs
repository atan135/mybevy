mod plugin;
mod types;

pub use plugin::AuthorityPlugin;
pub use types::{
    AUTHORITY_PACKET_MAX_BODY_LEN, AUTHORITY_PROTOCOL_VERSION, AuthorityCommand, AuthorityEndpoint,
    AuthorityEvent, AuthorityFrame, AuthorityMigration, AuthorityPacketCodec, AuthorityPeer,
    AuthorityRole, AuthoritySession, AuthoritySnapshot, AuthorityWireMessage,
    DEFAULT_AUTHORITY_FPS, DEFAULT_AUTHORITY_HOST, DEFAULT_AUTHORITY_PORT, PlayerInput,
    encode_authority_message,
};
