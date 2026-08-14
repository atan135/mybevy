use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

static NEXT_CONNECTION_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_LISTENER_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConnectionId(u64);

impl ConnectionId {
    pub fn new() -> Self {
        Self(NEXT_CONNECTION_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl Default for ConnectionId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ListenerId(u64);

impl ListenerId {
    pub fn new() -> Self {
        Self(NEXT_LISTENER_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl Default for ListenerId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RequestId(u64);

impl RequestId {
    pub fn new() -> Self {
        Self(NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NetworkTransport {
    Tcp,
    Kcp,
}

#[derive(Clone, Debug)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
    Custom(String),
}

#[derive(Clone, Debug)]
pub struct HttpRequest {
    pub request_id: RequestId,
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub timeout: Duration,
    /// Rejects a response before it can grow beyond this many bytes. `None` retains the
    /// historical unrestricted behavior for callers that already enforce their own limits.
    pub max_response_bytes: Option<usize>,
}

impl HttpRequest {
    pub fn new(method: HttpMethod, url: impl Into<String>) -> Self {
        Self {
            request_id: RequestId::new(),
            method,
            url: url.into(),
            headers: Vec::new(),
            body: None,
            timeout: Duration::from_secs(15),
            max_response_bytes: None,
        }
    }

    pub fn get(url: impl Into<String>) -> Self {
        Self::new(HttpMethod::Get, url)
    }

    pub fn post(url: impl Into<String>, body: impl Into<Vec<u8>>) -> Self {
        Self::new(HttpMethod::Post, url).with_body(body)
    }

    pub fn with_request_id(mut self, request_id: RequestId) -> Self {
        self.request_id = request_id;
        self
    }

    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn with_body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = Some(body.into());
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_max_response_bytes(mut self, max_response_bytes: usize) -> Self {
        self.max_response_bytes = Some(max_response_bytes);
        self
    }
}

#[derive(Clone, Debug)]
pub struct HttpResponse {
    pub request_id: RequestId,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct TcpConnectConfig {
    pub connection_id: ConnectionId,
    pub addr: String,
    pub connect_timeout: Duration,
    pub read_buffer_size: usize,
}

#[derive(Clone, Debug)]
pub struct TcpListenConfig {
    pub listener_id: ListenerId,
    pub addr: String,
    pub read_buffer_size: usize,
}

impl TcpListenConfig {
    pub fn new(addr: impl Into<String>) -> Self {
        Self {
            listener_id: ListenerId::new(),
            addr: addr.into(),
            read_buffer_size: 64 * 1024,
        }
    }

    pub fn with_listener_id(mut self, listener_id: ListenerId) -> Self {
        self.listener_id = listener_id;
        self
    }

    pub fn with_read_buffer_size(mut self, size: usize) -> Self {
        self.read_buffer_size = size.max(1);
        self
    }
}

impl TcpConnectConfig {
    pub fn new(addr: impl Into<String>) -> Self {
        Self {
            connection_id: ConnectionId::new(),
            addr: addr.into(),
            connect_timeout: Duration::from_secs(10),
            read_buffer_size: 64 * 1024,
        }
    }

    pub fn with_connection_id(mut self, connection_id: ConnectionId) -> Self {
        self.connection_id = connection_id;
        self
    }

    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    pub fn with_read_buffer_size(mut self, size: usize) -> Self {
        self.read_buffer_size = size.max(1);
        self
    }
}

#[derive(Clone, Debug)]
pub struct KcpSessionOptions {
    pub mtu: usize,
    pub nodelay: bool,
    pub interval: i32,
    pub resend: i32,
    pub no_congestion_control: bool,
    pub send_window: u16,
    pub receive_window: u16,
    pub session_expire: Duration,
    pub flush_write: bool,
    pub flush_acks_input: bool,
    pub stream: bool,
    pub allow_recv_empty_packet: bool,
}

impl Default for KcpSessionOptions {
    fn default() -> Self {
        Self {
            mtu: 1400,
            nodelay: false,
            interval: 40,
            resend: 0,
            no_congestion_control: false,
            send_window: 256,
            receive_window: 256,
            session_expire: Duration::from_secs(90),
            flush_write: false,
            flush_acks_input: false,
            stream: false,
            allow_recv_empty_packet: false,
        }
    }
}

impl KcpSessionOptions {
    pub fn fastest() -> Self {
        Self {
            nodelay: true,
            interval: 10,
            resend: 2,
            no_congestion_control: true,
            session_expire: Duration::from_secs(15),
            flush_write: true,
            flush_acks_input: true,
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug)]
pub struct KcpConnectConfig {
    pub connection_id: ConnectionId,
    pub addr: String,
    pub conv: Option<u32>,
    pub connect_timeout: Duration,
    pub read_buffer_size: usize,
    pub session: KcpSessionOptions,
}

impl KcpConnectConfig {
    pub fn new(addr: impl Into<String>) -> Self {
        Self {
            connection_id: ConnectionId::new(),
            addr: addr.into(),
            conv: None,
            connect_timeout: Duration::from_secs(10),
            read_buffer_size: 64 * 1024,
            session: KcpSessionOptions::default(),
        }
    }

    pub fn with_connection_id(mut self, connection_id: ConnectionId) -> Self {
        self.connection_id = connection_id;
        self
    }

    pub fn with_conv(mut self, conv: u32) -> Self {
        self.conv = Some(conv);
        self
    }

    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    pub fn with_read_buffer_size(mut self, size: usize) -> Self {
        self.read_buffer_size = size.max(1);
        self
    }

    pub fn with_session(mut self, session: KcpSessionOptions) -> Self {
        self.session = session;
        self
    }
}

#[derive(Clone, Debug)]
pub struct KcpListenConfig {
    pub listener_id: ListenerId,
    pub addr: String,
    pub read_buffer_size: usize,
    pub session: KcpSessionOptions,
}

impl KcpListenConfig {
    pub fn new(addr: impl Into<String>) -> Self {
        Self {
            listener_id: ListenerId::new(),
            addr: addr.into(),
            read_buffer_size: 64 * 1024,
            session: KcpSessionOptions::default(),
        }
    }

    pub fn with_listener_id(mut self, listener_id: ListenerId) -> Self {
        self.listener_id = listener_id;
        self
    }

    pub fn with_read_buffer_size(mut self, size: usize) -> Self {
        self.read_buffer_size = size.max(1);
        self
    }

    pub fn with_session(mut self, session: KcpSessionOptions) -> Self {
        self.session = session;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_builders_keep_defaults_and_overrides() {
        let request = HttpRequest::post("https://example.test", b"body".to_vec())
            .with_request_id(RequestId::from_raw(7))
            .with_header("content-type", "application/octet-stream")
            .with_timeout(Duration::from_secs(3))
            .with_max_response_bytes(1024);

        assert_eq!(request.request_id.raw(), 7);
        assert_eq!(request.url, "https://example.test");
        assert!(matches!(request.method, HttpMethod::Post));
        assert_eq!(request.body, Some(b"body".to_vec()));
        assert_eq!(
            request.headers,
            vec![("content-type".into(), "application/octet-stream".into())]
        );
        assert_eq!(request.timeout, Duration::from_secs(3));
        assert_eq!(request.max_response_bytes, Some(1024));
    }

    #[test]
    fn buffer_size_overrides_are_never_zero() {
        assert_eq!(
            TcpConnectConfig::new("127.0.0.1:1")
                .with_read_buffer_size(0)
                .read_buffer_size,
            1
        );
        assert_eq!(
            TcpListenConfig::new("127.0.0.1:1")
                .with_read_buffer_size(0)
                .read_buffer_size,
            1
        );
        assert_eq!(
            KcpConnectConfig::new("127.0.0.1:1")
                .with_read_buffer_size(0)
                .read_buffer_size,
            1
        );
        assert_eq!(
            KcpListenConfig::new("127.0.0.1:1")
                .with_read_buffer_size(0)
                .read_buffer_size,
            1
        );
    }
}
