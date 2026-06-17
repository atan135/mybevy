use std::{collections::HashMap, sync::Mutex};

use bevy::prelude::*;
use tokio::{
    runtime::{Builder, Runtime},
    sync::mpsc,
};

use super::{
    http, kcp, tcp,
    types::{ConnectionId, ListenerId, NetworkCommand, NetworkEvent, NetworkTransport},
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

pub(super) enum WorkerCommand {
    Network(NetworkCommand),
    AcceptedTcp(tcp::AcceptedTcpConnection),
    AcceptedKcp(kcp::AcceptedKcpConnection),
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
            WorkerCommand::AcceptedTcp(accepted) => {
                let connection_id = accepted.connection_id;
                let listener_id = accepted.listener_id;
                let remote_addr = accepted.remote_addr.clone();
                let (generation, send_rx, shutdown_rx) = register_connection_with_receivers(
                    &mut connections,
                    connection_id,
                    NetworkTransport::Tcp,
                    &mut next_generation,
                );

                send_event(
                    &event_tx,
                    NetworkEvent::Accepted {
                        listener_id,
                        connection_id,
                        transport: NetworkTransport::Tcp,
                        remote_addr,
                    },
                );
                tcp::spawn_accepted_tcp_connection(
                    accepted,
                    send_rx,
                    shutdown_rx,
                    event_tx.clone(),
                    command_tx.clone(),
                    generation,
                );
            }
            WorkerCommand::AcceptedKcp(accepted) => {
                let connection_id = accepted.connection_id;
                let listener_id = accepted.listener_id;
                let remote_addr = accepted.remote_addr.clone();
                let (generation, send_rx, shutdown_rx) = register_connection_with_receivers(
                    &mut connections,
                    connection_id,
                    NetworkTransport::Kcp,
                    &mut next_generation,
                );

                send_event(
                    &event_tx,
                    NetworkEvent::Accepted {
                        listener_id,
                        connection_id,
                        transport: NetworkTransport::Kcp,
                        remote_addr,
                    },
                );
                kcp::spawn_accepted_kcp_connection(
                    accepted,
                    send_rx,
                    shutdown_rx,
                    event_tx.clone(),
                    command_tx.clone(),
                    generation,
                );
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
            http::spawn_http_request(http_client.clone(), request, event_tx.clone());
        }
        NetworkCommand::ConnectTcp(config) => {
            let connection_id = config.connection_id;
            let (generation, send_rx, shutdown_rx) = register_connection_with_receivers(
                connections,
                connection_id,
                NetworkTransport::Tcp,
                next_generation,
            );

            tcp::spawn_tcp_connection(
                config,
                send_rx,
                shutdown_rx,
                event_tx.clone(),
                command_tx.clone(),
                generation,
            );
        }
        NetworkCommand::ConnectKcp(config) => {
            let connection_id = config.connection_id;
            let (generation, send_rx, shutdown_rx) = register_connection_with_receivers(
                connections,
                connection_id,
                NetworkTransport::Kcp,
                next_generation,
            );

            kcp::spawn_kcp_connection(
                config,
                send_rx,
                shutdown_rx,
                event_tx.clone(),
                command_tx.clone(),
                generation,
            );
        }
        NetworkCommand::ListenTcp(config) => {
            let shutdown_rx =
                register_listener(listeners, config.listener_id, NetworkTransport::Tcp);
            tcp::spawn_tcp_listener(config, shutdown_rx, event_tx.clone(), command_tx.clone());
        }
        NetworkCommand::ListenKcp(config) => {
            let shutdown_rx =
                register_listener(listeners, config.listener_id, NetworkTransport::Kcp);
            kcp::spawn_kcp_listener(config, shutdown_rx, event_tx.clone(), command_tx.clone());
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

fn register_connection_with_receivers(
    connections: &mut HashMap<ConnectionId, ConnectionHandle>,
    connection_id: ConnectionId,
    transport: NetworkTransport,
    next_generation: &mut u64,
) -> (u64, mpsc::Receiver<Vec<u8>>, mpsc::Receiver<()>) {
    replace_existing_connection(connections, connection_id);
    let generation = reserve_generation(next_generation);
    let (send_tx, send_rx) = mpsc::channel(COMMAND_CHANNEL_SIZE);
    let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

    connections.insert(
        connection_id,
        ConnectionHandle {
            transport,
            generation,
            send_tx,
            shutdown_tx,
        },
    );

    (generation, send_rx, shutdown_rx)
}

fn register_listener(
    listeners: &mut HashMap<ListenerId, ListenerHandle>,
    listener_id: ListenerId,
    transport: NetworkTransport,
) -> mpsc::Receiver<()> {
    replace_existing_listener(listeners, listener_id);
    let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
    listeners.insert(
        listener_id,
        ListenerHandle {
            transport,
            shutdown_tx,
        },
    );
    shutdown_rx
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

pub(super) fn send_event(event_tx: &mpsc::UnboundedSender<NetworkEvent>, event: NetworkEvent) {
    let _ = event_tx.send(event);
}

pub(super) fn send_connection_closed(
    command_tx: &mpsc::UnboundedSender<WorkerCommand>,
    connection_id: ConnectionId,
    generation: u64,
) {
    let _ = command_tx.send(WorkerCommand::ConnectionClosed {
        connection_id,
        generation,
    });
}

pub(super) fn send_listener_closed(
    command_tx: &mpsc::UnboundedSender<WorkerCommand>,
    listener_id: ListenerId,
) {
    let _ = command_tx.send(WorkerCommand::ListenerClosed { listener_id });
}
