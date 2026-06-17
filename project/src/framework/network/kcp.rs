use std::net::SocketAddr;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::mpsc,
    time,
};
use tokio_kcp::{KcpConfig, KcpListener, KcpNoDelayConfig, KcpStream};

use super::{
    runtime::{WorkerCommand, send_connection_closed, send_event, send_listener_closed},
    types::{
        ConnectionId, KcpConnectConfig, KcpListenConfig, KcpSessionOptions, ListenerId,
        NetworkEvent, NetworkTransport,
    },
};

pub(super) struct AcceptedKcpConnection {
    pub listener_id: ListenerId,
    pub connection_id: ConnectionId,
    pub stream: KcpStream,
    pub remote_addr: String,
    pub read_buffer_size: usize,
}

pub(super) fn spawn_kcp_connection(
    config: KcpConnectConfig,
    send_rx: mpsc::Receiver<Vec<u8>>,
    shutdown_rx: mpsc::Receiver<()>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    command_tx: mpsc::UnboundedSender<WorkerCommand>,
    generation: u64,
) {
    tokio::spawn(run_kcp_connection(
        config,
        send_rx,
        shutdown_rx,
        event_tx,
        command_tx,
        generation,
    ));
}

pub(super) fn spawn_kcp_listener(
    config: KcpListenConfig,
    shutdown_rx: mpsc::Receiver<()>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    command_tx: mpsc::UnboundedSender<WorkerCommand>,
) {
    tokio::spawn(run_kcp_listener(config, shutdown_rx, event_tx, command_tx));
}

pub(super) fn spawn_accepted_kcp_connection(
    accepted: AcceptedKcpConnection,
    send_rx: mpsc::Receiver<Vec<u8>>,
    shutdown_rx: mpsc::Receiver<()>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    command_tx: mpsc::UnboundedSender<WorkerCommand>,
    generation: u64,
) {
    tokio::spawn(run_kcp_stream(
        accepted.connection_id,
        accepted.stream,
        accepted.remote_addr,
        accepted.read_buffer_size,
        send_rx,
        shutdown_rx,
        event_tx,
        command_tx,
        generation,
    ));
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
                        let _ = command_tx.send(WorkerCommand::AcceptedKcp(AcceptedKcpConnection {
                            listener_id,
                            connection_id,
                            stream,
                            remote_addr: remote_addr.to_string(),
                            read_buffer_size: config.read_buffer_size,
                        }));
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
