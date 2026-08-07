use std::{io, net::SocketAddr, time::Duration};

use tokio::{
    io::{AsyncReadExt, AsyncWrite, AsyncWriteExt},
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
    pub outbound_timeout: Duration,
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
        accepted.outbound_timeout,
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

    let kcp_config = to_kcp_config(&config.session);
    let outbound_timeout = config.session.session_expire;
    let connect_future = async {
        let socket_addrs = resolve_socket_addrs(&remote_addr)
            .await
            .map_err(|error| format!("address resolution failed: {error}"))?;
        let mut last_error = None;

        for socket_addr in socket_addrs {
            let result = match config.conv {
                Some(conv) => KcpStream::connect_with_conv(&kcp_config, conv, socket_addr).await,
                None => KcpStream::connect(&kcp_config, socket_addr).await,
            };
            match result {
                Ok(stream) => return Ok(stream),
                Err(error) => last_error = Some(error.to_string()),
            }
        }

        Err(last_error.unwrap_or_else(|| "address resolution returned no addresses".to_string()))
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
        outbound_timeout,
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
    let outbound_timeout = config.session.session_expire;
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
                            outbound_timeout,
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
    outbound_timeout: Duration,
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
            biased;
            // `tokio_kcp` keeps an internal pending read waker. Prioritize control
            // and outbound work so that a quiet peer cannot keep a queued payload
            // behind repeated read wakeups.
            payload = send_rx.recv() => {
                let Some(payload) = payload else {
                    reason = Some("send queue closed".to_string());
                    break;
                };

                if let Err(error) = write_kcp_payload(&mut stream, &payload, outbound_timeout).await {
                    send_event(
                        &event_tx,
                        NetworkEvent::SendFailed {
                            connection_id,
                            transport: Some(NetworkTransport::Kcp),
                            error: error.clone(),
                        },
                    );
                    reason = Some(error);
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

async fn resolve_socket_addrs(remote_addr: &str) -> io::Result<Vec<SocketAddr>> {
    Ok(tokio::net::lookup_host(remote_addr).await?.collect())
}

async fn write_kcp_payload(
    stream: &mut KcpStream,
    payload: &[u8],
    outbound_timeout: Duration,
) -> Result<(), String> {
    write_payload_with_timeout(stream, payload, outbound_timeout).await
}

async fn write_payload_with_timeout(
    stream: &mut (impl AsyncWrite + Unpin),
    payload: &[u8],
    outbound_timeout: Duration,
) -> Result<(), String> {
    time::timeout(outbound_timeout, async {
        stream.write_all(payload).await?;
        stream.flush().await
    })
    .await
    .map_err(|_| format!("KCP outbound write timed out after {outbound_timeout:?}"))?
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::{
        io::{AsyncReadExt, duplex},
        runtime::Runtime,
        sync::mpsc,
        time::{self, timeout},
    };

    use super::*;

    #[test]
    fn queued_payload_is_written_before_any_inbound_packet_arrives() {
        Runtime::new().unwrap().block_on(async {
            let mut config = KcpConfig::default();
            config.nodelay = KcpNoDelayConfig::fastest();
            config.flush_write = true;
            config.flush_acks_input = true;
            config.stream = true;

            let mut listener = KcpListener::bind(config.clone(), "127.0.0.1:0")
                .await
                .unwrap();
            let remote_addr = listener.local_addr().unwrap();
            let stream = KcpStream::connect(&config, remote_addr).await.unwrap();
            let (send_tx, send_rx) = mpsc::channel(1);
            let (_shutdown_tx, shutdown_rx) = mpsc::channel(1);
            let (event_tx, _event_rx) = mpsc::unbounded_channel();
            let (command_tx, _command_rx) = mpsc::unbounded_channel();
            let connection_id = ConnectionId::from_raw(1);

            tokio::spawn(run_kcp_stream(
                connection_id,
                stream,
                remote_addr.to_string(),
                1024,
                Duration::from_secs(1),
                send_rx,
                shutdown_rx,
                event_tx,
                command_tx,
                1,
            ));

            let expected = b"queued-kcp-payload".to_vec();
            send_tx.send(expected.clone()).await.unwrap();

            let (mut server_stream, _) = timeout(Duration::from_secs(2), listener.accept())
                .await
                .expect("KCP listener must accept the queued client payload")
                .unwrap();
            let mut received = vec![0; expected.len()];
            let read = timeout(Duration::from_secs(2), server_stream.read(&mut received))
                .await
                .expect("queued KCP payload must be written without server-first traffic")
                .unwrap();

            assert_eq!(&received[..read], expected.as_slice());

            // Give the worker one scheduling turn before the test runtime drops it.
            time::sleep(Duration::from_millis(1)).await;
        });
    }

    #[test]
    fn stalled_outbound_write_fails_within_the_session_deadline() {
        Runtime::new().unwrap().block_on(async {
            let (mut writer, _reader) = duplex(1);
            let timeout = Duration::from_millis(10);

            let error = write_payload_with_timeout(&mut writer, b"blocked", timeout)
                .await
                .expect_err("a blackhole peer must not stall the worker indefinitely");

            assert_eq!(error, "KCP outbound write timed out after 10ms");
        });
    }

    #[test]
    fn kcp_remote_address_resolution_accepts_dns_hosts() {
        Runtime::new().unwrap().block_on(async {
            let addresses = resolve_socket_addrs("localhost:4000").await.unwrap();

            assert!(!addresses.is_empty());
            assert!(addresses.iter().all(|address| address.port() == 4000));
        });
    }

    #[test]
    fn kcp_remote_address_resolution_rejects_missing_ports() {
        Runtime::new().unwrap().block_on(async {
            let error = resolve_socket_addrs("localhost").await.unwrap_err();

            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        });
    }
}
