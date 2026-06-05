use std::{collections::HashMap, net::SocketAddr, sync::Mutex};

use bevy::prelude::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    runtime::{Builder, Runtime},
    sync::mpsc,
    time,
};
use tokio_kcp::{KcpConfig, KcpListener, KcpNoDelayConfig, KcpStream};

use super::types::{
    ConnectionId, HttpMethod, HttpRequest, HttpResponse, KcpConnectConfig, KcpListenConfig,
    KcpSessionOptions, ListenerId, NetworkCommand, NetworkEvent, NetworkTransport,
    TcpConnectConfig, TcpListenConfig,
};

const COMMAND_CHANNEL_SIZE: usize = 256;

#[derive(Resource)]
pub struct NetworkRuntime {
    runtime: Option<Runtime>,
    command_tx: mpsc::UnboundedSender<WorkerCommand>,
    event_rx: Mutex<mpsc::UnboundedReceiver<NetworkEvent>>,
}

impl NetworkRuntime {
    pub fn new() -> Result<Self, String> {
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .thread_name("project-network")
            .build()
            .map_err(|err| format!("failed to start network runtime: {err}"))?;

        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        runtime.spawn(run_worker(command_rx, command_tx.clone(), event_tx));

        Ok(Self {
            runtime: Some(runtime),
            command_tx,
            event_rx: Mutex::new(event_rx),
        })
    }

    pub fn send(&self, command: NetworkCommand) -> Result<(), String> {
        self.command_tx
            .send(WorkerCommand::Network(command))
            .map_err(|_| "network worker is not running".to_string())
    }

    pub fn drain_events(&self) -> Vec<NetworkEvent> {
        let mut events = Vec::new();
        let Ok(mut event_rx) = self.event_rx.lock() else {
            return events;
        };

        while let Ok(event) = event_rx.try_recv() {
            events.push(event);
        }

        events
    }
}

impl Drop for NetworkRuntime {
    fn drop(&mut self) {
        let _ = self.command_tx.send(WorkerCommand::Shutdown);
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

enum WorkerCommand {
    Network(NetworkCommand),
    AcceptedTcp {
        listener_id: ListenerId,
        connection_id: ConnectionId,
        stream: TcpStream,
        remote_addr: String,
        read_buffer_size: usize,
    },
    AcceptedKcp {
        listener_id: ListenerId,
        connection_id: ConnectionId,
        stream: KcpStream,
        remote_addr: String,
        read_buffer_size: usize,
    },
    ConnectionClosed {
        connection_id: ConnectionId,
        generation: u64,
    },
    ListenerClosed {
        listener_id: ListenerId,
    },
    Shutdown,
}

struct ConnectionHandle {
    transport: NetworkTransport,
    generation: u64,
    send_tx: mpsc::Sender<Vec<u8>>,
    shutdown_tx: mpsc::Sender<()>,
}

struct ListenerHandle {
    transport: NetworkTransport,
    shutdown_tx: mpsc::Sender<()>,
}

async fn run_worker(
    mut command_rx: mpsc::UnboundedReceiver<WorkerCommand>,
    command_tx: mpsc::UnboundedSender<WorkerCommand>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
) {
    let http_client = reqwest::Client::new();
    let mut connections = HashMap::<ConnectionId, ConnectionHandle>::new();
    let mut listeners = HashMap::<ListenerId, ListenerHandle>::new();
    let mut next_generation = 1_u64;

    while let Some(command) = command_rx.recv().await {
        match command {
            WorkerCommand::Network(command) => {
                handle_network_command(
                    command,
                    &http_client,
                    &mut connections,
                    &mut listeners,
                    &event_tx,
                    &command_tx,
                    &mut next_generation,
                )
                .await;
            }
            WorkerCommand::ConnectionClosed {
                connection_id,
                generation,
            } => {
                if connections
                    .get(&connection_id)
                    .is_some_and(|connection| connection.generation == generation)
                {
                    connections.remove(&connection_id);
                }
            }
            WorkerCommand::ListenerClosed { listener_id } => {
                listeners.remove(&listener_id);
            }
            WorkerCommand::AcceptedTcp {
                listener_id,
                connection_id,
                stream,
                remote_addr,
                read_buffer_size,
            } => {
                replace_existing_connection(&mut connections, connection_id);
                let generation = reserve_generation(&mut next_generation);
                let (send_tx, send_rx) = mpsc::channel(COMMAND_CHANNEL_SIZE);
                let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
                connections.insert(
                    connection_id,
                    ConnectionHandle {
                        transport: NetworkTransport::Tcp,
                        generation,
                        send_tx,
                        shutdown_tx,
                    },
                );
                send_event(
                    &event_tx,
                    NetworkEvent::Accepted {
                        listener_id,
                        connection_id,
                        transport: NetworkTransport::Tcp,
                        remote_addr: remote_addr.clone(),
                    },
                );
                tokio::spawn(run_accepted_tcp_connection(
                    connection_id,
                    stream,
                    remote_addr,
                    read_buffer_size,
                    send_rx,
                    shutdown_rx,
                    event_tx.clone(),
                    command_tx.clone(),
                    generation,
                ));
            }
            WorkerCommand::AcceptedKcp {
                listener_id,
                connection_id,
                stream,
                remote_addr,
                read_buffer_size,
            } => {
                replace_existing_connection(&mut connections, connection_id);
                let generation = reserve_generation(&mut next_generation);
                let (send_tx, send_rx) = mpsc::channel(COMMAND_CHANNEL_SIZE);
                let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
                connections.insert(
                    connection_id,
                    ConnectionHandle {
                        transport: NetworkTransport::Kcp,
                        generation,
                        send_tx,
                        shutdown_tx,
                    },
                );
                send_event(
                    &event_tx,
                    NetworkEvent::Accepted {
                        listener_id,
                        connection_id,
                        transport: NetworkTransport::Kcp,
                        remote_addr: remote_addr.clone(),
                    },
                );
                tokio::spawn(run_accepted_kcp_connection(
                    connection_id,
                    stream,
                    remote_addr,
                    read_buffer_size,
                    send_rx,
                    shutdown_rx,
                    event_tx.clone(),
                    command_tx.clone(),
                    generation,
                ));
            }
            WorkerCommand::Shutdown => break,
        }
    }

    for (_, connection) in connections {
        let _ = connection.shutdown_tx.try_send(());
    }
    for (_, listener) in listeners {
        let _ = listener.shutdown_tx.try_send(());
    }
}

async fn handle_network_command(
    command: NetworkCommand,
    http_client: &reqwest::Client,
    connections: &mut HashMap<ConnectionId, ConnectionHandle>,
    listeners: &mut HashMap<ListenerId, ListenerHandle>,
    event_tx: &mpsc::UnboundedSender<NetworkEvent>,
    command_tx: &mpsc::UnboundedSender<WorkerCommand>,
    next_generation: &mut u64,
) {
    match command {
        NetworkCommand::Http(request) => {
            let client = http_client.clone();
            let event_tx = event_tx.clone();
            tokio::spawn(async move {
                let event = execute_http_request(client, request).await;
                send_event(&event_tx, event);
            });
        }
        NetworkCommand::ConnectTcp(config) => {
            let connection_id = config.connection_id;
            replace_existing_connection(connections, connection_id);
            let generation = reserve_generation(next_generation);

            let (send_tx, send_rx) = mpsc::channel(COMMAND_CHANNEL_SIZE);
            let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

            connections.insert(
                connection_id,
                ConnectionHandle {
                    transport: NetworkTransport::Tcp,
                    generation,
                    send_tx,
                    shutdown_tx,
                },
            );

            tokio::spawn(run_tcp_connection(
                config,
                send_rx,
                shutdown_rx,
                event_tx.clone(),
                command_tx.clone(),
                generation,
            ));
        }
        NetworkCommand::ConnectKcp(config) => {
            let connection_id = config.connection_id;
            replace_existing_connection(connections, connection_id);
            let generation = reserve_generation(next_generation);

            let (send_tx, send_rx) = mpsc::channel(COMMAND_CHANNEL_SIZE);
            let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

            connections.insert(
                connection_id,
                ConnectionHandle {
                    transport: NetworkTransport::Kcp,
                    generation,
                    send_tx,
                    shutdown_tx,
                },
            );

            tokio::spawn(run_kcp_connection(
                config,
                send_rx,
                shutdown_rx,
                event_tx.clone(),
                command_tx.clone(),
                generation,
            ));
        }
        NetworkCommand::ListenTcp(config) => {
            replace_existing_listener(listeners, config.listener_id);
            let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
            listeners.insert(
                config.listener_id,
                ListenerHandle {
                    transport: NetworkTransport::Tcp,
                    shutdown_tx,
                },
            );
            tokio::spawn(run_tcp_listener(
                config,
                shutdown_rx,
                event_tx.clone(),
                command_tx.clone(),
            ));
        }
        NetworkCommand::ListenKcp(config) => {
            replace_existing_listener(listeners, config.listener_id);
            let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
            listeners.insert(
                config.listener_id,
                ListenerHandle {
                    transport: NetworkTransport::Kcp,
                    shutdown_tx,
                },
            );
            tokio::spawn(run_kcp_listener(
                config,
                shutdown_rx,
                event_tx.clone(),
                command_tx.clone(),
            ));
        }
        NetworkCommand::Send {
            connection_id,
            payload,
        } => {
            let Some(connection) = connections.get(&connection_id) else {
                send_event(
                    event_tx,
                    NetworkEvent::SendFailed {
                        connection_id,
                        transport: None,
                        error: "connection not found".to_string(),
                    },
                );
                return;
            };

            let transport = connection.transport;
            if let Err(err) = connection.send_tx.send(payload).await {
                send_event(
                    event_tx,
                    NetworkEvent::SendFailed {
                        connection_id,
                        transport: Some(transport),
                        error: format!("connection send queue is closed: {err}"),
                    },
                );
            }
        }
        NetworkCommand::Disconnect { connection_id } => {
            let Some(connection) = connections.remove(&connection_id) else {
                send_event(
                    event_tx,
                    NetworkEvent::SendFailed {
                        connection_id,
                        transport: None,
                        error: "connection not found".to_string(),
                    },
                );
                return;
            };

            let _ = connection.shutdown_tx.send(()).await;
        }
        NetworkCommand::StopListener { listener_id } => {
            let Some(listener) = listeners.remove(&listener_id) else {
                send_event(
                    event_tx,
                    NetworkEvent::ListenFailed {
                        listener_id,
                        transport: NetworkTransport::Tcp,
                        local_addr: String::new(),
                        error: "listener not found".to_string(),
                    },
                );
                return;
            };

            let _transport = listener.transport;
            let _ = listener.shutdown_tx.send(()).await;
        }
    }
}

fn replace_existing_connection(
    connections: &mut HashMap<ConnectionId, ConnectionHandle>,
    connection_id: ConnectionId,
) {
    if let Some(connection) = connections.remove(&connection_id) {
        let _ = connection.shutdown_tx.try_send(());
    }
}

fn replace_existing_listener(
    listeners: &mut HashMap<ListenerId, ListenerHandle>,
    listener_id: ListenerId,
) {
    if let Some(listener) = listeners.remove(&listener_id) {
        let _ = listener.shutdown_tx.try_send(());
    }
}

fn reserve_generation(next_generation: &mut u64) -> u64 {
    let generation = *next_generation;
    *next_generation = next_generation.saturating_add(1).max(1);
    generation
}

async fn execute_http_request(client: reqwest::Client, request: HttpRequest) -> NetworkEvent {
    let request_id = request.request_id;
    let method = match reqwest_method(&request.method) {
        Ok(method) => method,
        Err(error) => {
            return NetworkEvent::HttpError { request_id, error };
        }
    };

    let mut builder = client.request(method, request.url).timeout(request.timeout);

    for (name, value) in request.headers {
        builder = builder.header(name, value);
    }

    if let Some(body) = request.body {
        builder = builder.body(body);
    }

    let result = async {
        let response = builder.send().await?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_string(),
                    value.to_str().unwrap_or_default().to_string(),
                )
            })
            .collect::<Vec<_>>();
        let body = response.bytes().await?.to_vec();

        Ok::<_, reqwest::Error>(HttpResponse {
            request_id,
            status,
            headers,
            body,
        })
    }
    .await;

    match result {
        Ok(response) => NetworkEvent::HttpResponse(response),
        Err(err) => NetworkEvent::HttpError {
            request_id,
            error: err.to_string(),
        },
    }
}

fn reqwest_method(method: &HttpMethod) -> Result<reqwest::Method, String> {
    match method {
        HttpMethod::Get => Ok(reqwest::Method::GET),
        HttpMethod::Post => Ok(reqwest::Method::POST),
        HttpMethod::Put => Ok(reqwest::Method::PUT),
        HttpMethod::Patch => Ok(reqwest::Method::PATCH),
        HttpMethod::Delete => Ok(reqwest::Method::DELETE),
        HttpMethod::Head => Ok(reqwest::Method::HEAD),
        HttpMethod::Options => Ok(reqwest::Method::OPTIONS),
        HttpMethod::Custom(value) => reqwest::Method::from_bytes(value.as_bytes())
            .map_err(|err| format!("invalid HTTP method `{value}`: {err}")),
    }
}

async fn run_tcp_connection(
    config: TcpConnectConfig,
    send_rx: mpsc::Receiver<Vec<u8>>,
    shutdown_rx: mpsc::Receiver<()>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    command_tx: mpsc::UnboundedSender<WorkerCommand>,
    generation: u64,
) {
    let connection_id = config.connection_id;
    let remote_addr = config.addr.clone();

    let connect_result =
        time::timeout(config.connect_timeout, TcpStream::connect(&config.addr)).await;
    let stream = match connect_result {
        Ok(Ok(stream)) => stream,
        Ok(Err(err)) => {
            send_event(
                &event_tx,
                NetworkEvent::ConnectionFailed {
                    connection_id,
                    transport: NetworkTransport::Tcp,
                    remote_addr,
                    error: err.to_string(),
                },
            );
            send_connection_closed(&command_tx, connection_id, generation);
            return;
        }
        Err(_) => {
            send_event(
                &event_tx,
                NetworkEvent::ConnectionFailed {
                    connection_id,
                    transport: NetworkTransport::Tcp,
                    remote_addr,
                    error: format!("connect timeout after {:?}", config.connect_timeout),
                },
            );
            send_connection_closed(&command_tx, connection_id, generation);
            return;
        }
    };

    send_event(
        &event_tx,
        NetworkEvent::Connected {
            connection_id,
            transport: NetworkTransport::Tcp,
            remote_addr: remote_addr.clone(),
        },
    );

    run_tcp_stream(
        connection_id,
        stream,
        remote_addr,
        config.read_buffer_size,
        send_rx,
        shutdown_rx,
        event_tx,
        command_tx,
        generation,
    )
    .await;
}

async fn run_tcp_listener(
    config: TcpListenConfig,
    mut shutdown_rx: mpsc::Receiver<()>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    command_tx: mpsc::UnboundedSender<WorkerCommand>,
) {
    let listener_id = config.listener_id;
    let bind_addr = config.addr.clone();
    let listener = match TcpListener::bind(&bind_addr).await {
        Ok(listener) => listener,
        Err(err) => {
            send_event(
                &event_tx,
                NetworkEvent::ListenFailed {
                    listener_id,
                    transport: NetworkTransport::Tcp,
                    local_addr: bind_addr,
                    error: err.to_string(),
                },
            );
            send_listener_closed(&command_tx, listener_id);
            return;
        }
    };

    let local_addr = listener
        .local_addr()
        .map(|addr| addr.to_string())
        .unwrap_or(bind_addr);
    send_event(
        &event_tx,
        NetworkEvent::Listening {
            listener_id,
            transport: NetworkTransport::Tcp,
            local_addr: local_addr.clone(),
        },
    );

    let mut reason = None;
    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, remote_addr)) => {
                        let connection_id = ConnectionId::new();
                        let _ = command_tx.send(WorkerCommand::AcceptedTcp {
                            listener_id,
                            connection_id,
                            stream,
                            remote_addr: remote_addr.to_string(),
                            read_buffer_size: config.read_buffer_size,
                        });
                    }
                    Err(err) => {
                        reason = Some(err.to_string());
                        break;
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                break;
            }
        }
    }

    send_event(
        &event_tx,
        NetworkEvent::ListenerStopped {
            listener_id,
            transport: NetworkTransport::Tcp,
            local_addr,
            reason,
        },
    );
    send_listener_closed(&command_tx, listener_id);
}

async fn run_accepted_tcp_connection(
    connection_id: ConnectionId,
    stream: TcpStream,
    remote_addr: String,
    read_buffer_size: usize,
    send_rx: mpsc::Receiver<Vec<u8>>,
    shutdown_rx: mpsc::Receiver<()>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    command_tx: mpsc::UnboundedSender<WorkerCommand>,
    generation: u64,
) {
    run_tcp_stream(
        connection_id,
        stream,
        remote_addr,
        read_buffer_size,
        send_rx,
        shutdown_rx,
        event_tx,
        command_tx,
        generation,
    )
    .await;
}

async fn run_tcp_stream(
    connection_id: ConnectionId,
    stream: TcpStream,
    _remote_addr: String,
    read_buffer_size: usize,
    mut send_rx: mpsc::Receiver<Vec<u8>>,
    mut shutdown_rx: mpsc::Receiver<()>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    command_tx: mpsc::UnboundedSender<WorkerCommand>,
    generation: u64,
) {
    let (mut reader, mut writer) = stream.into_split();
    let mut read_buffer = vec![0; read_buffer_size.max(1)];
    let mut reason = None;

    loop {
        tokio::select! {
            read_result = reader.read(&mut read_buffer) => {
                match read_result {
                    Ok(0) => {
                        reason = Some("remote closed".to_string());
                        break;
                    }
                    Ok(bytes) => {
                        send_event(
                            &event_tx,
                            NetworkEvent::Packet {
                                connection_id,
                                transport: NetworkTransport::Tcp,
                                payload: read_buffer[..bytes].to_vec(),
                            },
                        );
                    }
                    Err(err) => {
                        reason = Some(err.to_string());
                        break;
                    }
                }
            }
            payload = send_rx.recv() => {
                let Some(payload) = payload else {
                    reason = Some("send queue closed".to_string());
                    break;
                };

                if let Err(err) = writer.write_all(&payload).await {
                    send_event(
                        &event_tx,
                        NetworkEvent::SendFailed {
                            connection_id,
                            transport: Some(NetworkTransport::Tcp),
                            error: err.to_string(),
                        },
                    );
                    reason = Some(err.to_string());
                    break;
                }

                send_event(
                    &event_tx,
                    NetworkEvent::DataSent {
                        connection_id,
                        transport: NetworkTransport::Tcp,
                        bytes: payload.len(),
                    },
                );
            }
            _ = shutdown_rx.recv() => {
                break;
            }
        }
    }

    send_event(
        &event_tx,
        NetworkEvent::Disconnected {
            connection_id,
            transport: NetworkTransport::Tcp,
            reason,
        },
    );
    send_connection_closed(&command_tx, connection_id, generation);
}

async fn run_kcp_connection(
    config: KcpConnectConfig,
    send_rx: mpsc::Receiver<Vec<u8>>,
    shutdown_rx: mpsc::Receiver<()>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    command_tx: mpsc::UnboundedSender<WorkerCommand>,
    generation: u64,
) {
    let connection_id = config.connection_id;
    let remote_addr = config.addr.clone();

    let socket_addr = match remote_addr.parse::<SocketAddr>() {
        Ok(addr) => addr,
        Err(err) => {
            send_event(
                &event_tx,
                NetworkEvent::ConnectionFailed {
                    connection_id,
                    transport: NetworkTransport::Kcp,
                    remote_addr,
                    error: format!("invalid socket address: {err}"),
                },
            );
            send_connection_closed(&command_tx, connection_id, generation);
            return;
        }
    };

    let kcp_config = to_kcp_config(&config.session);
    let connect_future = async {
        match config.conv {
            Some(conv) => KcpStream::connect_with_conv(&kcp_config, conv, socket_addr).await,
            None => KcpStream::connect(&kcp_config, socket_addr).await,
        }
    };

    let connect_result = time::timeout(config.connect_timeout, connect_future).await;
    let stream = match connect_result {
        Ok(Ok(stream)) => stream,
        Ok(Err(err)) => {
            send_event(
                &event_tx,
                NetworkEvent::ConnectionFailed {
                    connection_id,
                    transport: NetworkTransport::Kcp,
                    remote_addr,
                    error: err.to_string(),
                },
            );
            send_connection_closed(&command_tx, connection_id, generation);
            return;
        }
        Err(_) => {
            send_event(
                &event_tx,
                NetworkEvent::ConnectionFailed {
                    connection_id,
                    transport: NetworkTransport::Kcp,
                    remote_addr,
                    error: format!("connect timeout after {:?}", config.connect_timeout),
                },
            );
            send_connection_closed(&command_tx, connection_id, generation);
            return;
        }
    };

    send_event(
        &event_tx,
        NetworkEvent::Connected {
            connection_id,
            transport: NetworkTransport::Kcp,
            remote_addr: remote_addr.clone(),
        },
    );

    run_kcp_stream(
        connection_id,
        stream,
        remote_addr,
        config.read_buffer_size,
        send_rx,
        shutdown_rx,
        event_tx,
        command_tx,
        generation,
    )
    .await;
}

async fn run_kcp_listener(
    config: KcpListenConfig,
    mut shutdown_rx: mpsc::Receiver<()>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    command_tx: mpsc::UnboundedSender<WorkerCommand>,
) {
    let listener_id = config.listener_id;
    let bind_addr = config.addr.clone();
    let kcp_config = to_kcp_config(&config.session);
    let mut listener = match KcpListener::bind(kcp_config, &bind_addr).await {
        Ok(listener) => listener,
        Err(err) => {
            send_event(
                &event_tx,
                NetworkEvent::ListenFailed {
                    listener_id,
                    transport: NetworkTransport::Kcp,
                    local_addr: bind_addr,
                    error: err.to_string(),
                },
            );
            send_listener_closed(&command_tx, listener_id);
            return;
        }
    };

    send_event(
        &event_tx,
        NetworkEvent::Listening {
            listener_id,
            transport: NetworkTransport::Kcp,
            local_addr: bind_addr.clone(),
        },
    );

    let mut reason = None;
    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, remote_addr)) => {
                        let connection_id = ConnectionId::new();
                        let _ = command_tx.send(WorkerCommand::AcceptedKcp {
                            listener_id,
                            connection_id,
                            stream,
                            remote_addr: remote_addr.to_string(),
                            read_buffer_size: config.read_buffer_size,
                        });
                    }
                    Err(err) => {
                        reason = Some(err.to_string());
                        break;
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                break;
            }
        }
    }

    send_event(
        &event_tx,
        NetworkEvent::ListenerStopped {
            listener_id,
            transport: NetworkTransport::Kcp,
            local_addr: bind_addr,
            reason,
        },
    );
    send_listener_closed(&command_tx, listener_id);
}

async fn run_accepted_kcp_connection(
    connection_id: ConnectionId,
    stream: KcpStream,
    remote_addr: String,
    read_buffer_size: usize,
    send_rx: mpsc::Receiver<Vec<u8>>,
    shutdown_rx: mpsc::Receiver<()>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    command_tx: mpsc::UnboundedSender<WorkerCommand>,
    generation: u64,
) {
    run_kcp_stream(
        connection_id,
        stream,
        remote_addr,
        read_buffer_size,
        send_rx,
        shutdown_rx,
        event_tx,
        command_tx,
        generation,
    )
    .await;
}

async fn run_kcp_stream(
    connection_id: ConnectionId,
    mut stream: KcpStream,
    _remote_addr: String,
    read_buffer_size: usize,
    mut send_rx: mpsc::Receiver<Vec<u8>>,
    mut shutdown_rx: mpsc::Receiver<()>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    command_tx: mpsc::UnboundedSender<WorkerCommand>,
    generation: u64,
) {
    let mut read_buffer = vec![0; read_buffer_size.max(1)];
    let mut reason = None;

    loop {
        tokio::select! {
            read_result = stream.read(&mut read_buffer) => {
                match read_result {
                    Ok(0) => {
                        reason = Some("remote closed".to_string());
                        break;
                    }
                    Ok(bytes) => {
                        send_event(
                            &event_tx,
                            NetworkEvent::Packet {
                                connection_id,
                                transport: NetworkTransport::Kcp,
                                payload: read_buffer[..bytes].to_vec(),
                            },
                        );
                    }
                    Err(err) => {
                        reason = Some(err.to_string());
                        break;
                    }
                }
            }
            payload = send_rx.recv() => {
                let Some(payload) = payload else {
                    reason = Some("send queue closed".to_string());
                    break;
                };

                if let Err(err) = stream.write_all(&payload).await {
                    send_event(
                        &event_tx,
                        NetworkEvent::SendFailed {
                            connection_id,
                            transport: Some(NetworkTransport::Kcp),
                            error: err.to_string(),
                        },
                    );
                    reason = Some(err.to_string());
                    break;
                }

                if let Err(err) = stream.flush().await {
                    send_event(
                        &event_tx,
                        NetworkEvent::SendFailed {
                            connection_id,
                            transport: Some(NetworkTransport::Kcp),
                            error: err.to_string(),
                        },
                    );
                    reason = Some(err.to_string());
                    break;
                }

                send_event(
                    &event_tx,
                    NetworkEvent::DataSent {
                        connection_id,
                        transport: NetworkTransport::Kcp,
                        bytes: payload.len(),
                    },
                );
            }
            _ = shutdown_rx.recv() => {
                break;
            }
        }
    }

    send_event(
        &event_tx,
        NetworkEvent::Disconnected {
            connection_id,
            transport: NetworkTransport::Kcp,
            reason,
        },
    );
    send_connection_closed(&command_tx, connection_id, generation);
}

fn to_kcp_config(options: &KcpSessionOptions) -> KcpConfig {
    KcpConfig {
        mtu: options.mtu,
        nodelay: KcpNoDelayConfig {
            nodelay: options.nodelay,
            interval: options.interval,
            resend: options.resend,
            nc: options.no_congestion_control,
        },
        wnd_size: (options.send_window, options.receive_window),
        session_expire: options.session_expire,
        flush_write: options.flush_write,
        flush_acks_input: options.flush_acks_input,
        stream: options.stream,
        allow_recv_empty_packet: options.allow_recv_empty_packet,
    }
}

fn send_event(event_tx: &mpsc::UnboundedSender<NetworkEvent>, event: NetworkEvent) {
    let _ = event_tx.send(event);
}

fn send_connection_closed(
    command_tx: &mpsc::UnboundedSender<WorkerCommand>,
    connection_id: ConnectionId,
    generation: u64,
) {
    let _ = command_tx.send(WorkerCommand::ConnectionClosed {
        connection_id,
        generation,
    });
}

fn send_listener_closed(
    command_tx: &mpsc::UnboundedSender<WorkerCommand>,
    listener_id: ListenerId,
) {
    let _ = command_tx.send(WorkerCommand::ListenerClosed { listener_id });
}
