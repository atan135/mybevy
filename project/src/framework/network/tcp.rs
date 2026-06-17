use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    time,
};

use super::{
    runtime::{WorkerCommand, send_connection_closed, send_event, send_listener_closed},
    types::{
        ConnectionId, ListenerId, NetworkEvent, NetworkTransport, TcpConnectConfig, TcpListenConfig,
    },
};

pub(super) struct AcceptedTcpConnection {
    pub listener_id: ListenerId,
    pub connection_id: ConnectionId,
    pub stream: TcpStream,
    pub remote_addr: String,
    pub read_buffer_size: usize,
}

pub(super) fn spawn_tcp_connection(
    config: TcpConnectConfig,
    send_rx: mpsc::Receiver<Vec<u8>>,
    shutdown_rx: mpsc::Receiver<()>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    command_tx: mpsc::UnboundedSender<WorkerCommand>,
    generation: u64,
) {
    tokio::spawn(run_tcp_connection(
        config,
        send_rx,
        shutdown_rx,
        event_tx,
        command_tx,
        generation,
    ));
}

pub(super) fn spawn_tcp_listener(
    config: TcpListenConfig,
    shutdown_rx: mpsc::Receiver<()>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    command_tx: mpsc::UnboundedSender<WorkerCommand>,
) {
    tokio::spawn(run_tcp_listener(config, shutdown_rx, event_tx, command_tx));
}

pub(super) fn spawn_accepted_tcp_connection(
    accepted: AcceptedTcpConnection,
    send_rx: mpsc::Receiver<Vec<u8>>,
    shutdown_rx: mpsc::Receiver<()>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    command_tx: mpsc::UnboundedSender<WorkerCommand>,
    generation: u64,
) {
    tokio::spawn(run_tcp_stream(
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
                        let _ = command_tx.send(WorkerCommand::AcceptedTcp(AcceptedTcpConnection {
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
            transport: NetworkTransport::Tcp,
            local_addr,
            reason,
        },
    );
    send_listener_closed(&command_tx, listener_id);
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
