use bevy::prelude::Message;

pub use network_types::{
    ConnectionId, HttpMethod, HttpRequest, HttpResponse, KcpConnectConfig, KcpListenConfig,
    KcpSessionOptions, ListenerId, NetworkTransport, RequestId, TcpConnectConfig, TcpListenConfig,
};

#[derive(Clone, Debug, Message)]
pub enum NetworkCommand {
    Http(HttpRequest),
    CancelHttp {
        request_id: RequestId,
    },
    ConnectTcp(TcpConnectConfig),
    ConnectKcp(KcpConnectConfig),
    ListenTcp(TcpListenConfig),
    ListenKcp(KcpListenConfig),
    Send {
        connection_id: ConnectionId,
        payload: Vec<u8>,
    },
    Disconnect {
        connection_id: ConnectionId,
    },
    StopListener {
        listener_id: ListenerId,
    },
}

#[derive(Clone, Debug, Message)]
pub enum NetworkEvent {
    HttpResponse(HttpResponse),
    HttpError {
        request_id: RequestId,
        error: String,
    },
    Connected {
        connection_id: ConnectionId,
        transport: NetworkTransport,
        remote_addr: String,
    },
    ConnectionFailed {
        connection_id: ConnectionId,
        transport: NetworkTransport,
        remote_addr: String,
        error: String,
    },
    Listening {
        listener_id: ListenerId,
        transport: NetworkTransport,
        local_addr: String,
    },
    ListenFailed {
        listener_id: ListenerId,
        transport: NetworkTransport,
        local_addr: String,
        error: String,
    },
    Accepted {
        listener_id: ListenerId,
        connection_id: ConnectionId,
        transport: NetworkTransport,
        remote_addr: String,
    },
    Packet {
        connection_id: ConnectionId,
        transport: NetworkTransport,
        payload: Vec<u8>,
    },
    DataSent {
        connection_id: ConnectionId,
        transport: NetworkTransport,
        bytes: usize,
    },
    SendFailed {
        connection_id: ConnectionId,
        transport: Option<NetworkTransport>,
        error: String,
    },
    Disconnected {
        connection_id: ConnectionId,
        transport: NetworkTransport,
        reason: Option<String>,
    },
    ListenerStopped {
        listener_id: ListenerId,
        transport: NetworkTransport,
        local_addr: String,
        reason: Option<String>,
    },
}
