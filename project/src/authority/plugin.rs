use std::{env, time::Duration};

use bevy::prelude::*;

use crate::myserver::{MyServerCommand, MyServerEvent};
use crate::network::{
    ConnectionId, KcpConnectConfig, KcpListenConfig, KcpSessionOptions, ListenerId, NetworkCommand,
    NetworkEvent, NetworkTransport, TcpConnectConfig, TcpListenConfig,
};

use super::types::{
    AUTHORITY_PROTOCOL_VERSION, AuthorityCommand, AuthorityEndpoint, AuthorityEvent,
    AuthorityFrame, AuthorityMigration, AuthorityPeer, AuthorityRole, AuthoritySession,
    AuthoritySnapshot, AuthorityWireMessage, DEFAULT_AUTHORITY_FPS, PlayerInput,
    encode_authority_message,
};

pub struct AuthorityPlugin;

impl Plugin for AuthorityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AuthoritySession>()
            .init_resource::<AuthorityDevConfig>()
            .init_resource::<AuthorityDevState>()
            .add_message::<AuthorityCommand>()
            .add_message::<AuthorityEvent>()
            .add_systems(Startup, authority_dev_startup)
            .add_systems(
                Update,
                (
                    handle_authority_commands,
                    handle_authority_network_events,
                    handle_myserver_authority_events,
                    tick_local_authority,
                    authority_dev_follow_myserver,
                    authority_dev_auto_input,
                    authority_dev_log_events,
                )
                    .chain(),
            );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AuthorityDevMode {
    Off,
    LocalHost,
    LanHost,
    LanClient,
    MyServer,
}

#[derive(Resource, Debug)]
struct AuthorityDevConfig {
    mode: AuthorityDevMode,
    player_id: String,
    bind_addr: String,
    remote_host: String,
    remote_port: u16,
    transport: NetworkTransport,
    auto_input: bool,
    input_interval: Duration,
    myserver_guest_id: Option<String>,
    myserver_room_id: String,
    myserver_policy_id: String,
}

impl Default for AuthorityDevConfig {
    fn default() -> Self {
        Self {
            mode: env_dev_mode("AUTHORITY_DEV_MODE"),
            player_id: env_string("AUTHORITY_PLAYER_ID", "bevy-player"),
            bind_addr: env_string(
                "AUTHORITY_BIND_ADDR",
                &format!(
                    "{}:{}",
                    super::types::DEFAULT_AUTHORITY_HOST,
                    super::types::DEFAULT_AUTHORITY_PORT
                ),
            ),
            remote_host: env_string(
                "AUTHORITY_REMOTE_HOST",
                super::types::DEFAULT_AUTHORITY_HOST,
            ),
            remote_port: env_u16(
                "AUTHORITY_REMOTE_PORT",
                super::types::DEFAULT_AUTHORITY_PORT,
            ),
            transport: env_transport("AUTHORITY_TRANSPORT").unwrap_or(NetworkTransport::Tcp),
            auto_input: env_bool("AUTHORITY_DEV_AUTO_INPUT", true),
            input_interval: Duration::from_millis(env_u64("AUTHORITY_DEV_INPUT_INTERVAL_MS", 500)),
            myserver_guest_id: env::var("MYSERVER_GUEST_ID")
                .ok()
                .or_else(|| env::var("AUTHORITY_MYSERVER_GUEST_ID").ok())
                .filter(|value| !value.trim().is_empty()),
            myserver_room_id: env::var("AUTHORITY_MYSERVER_ROOM")
                .ok()
                .or_else(|| env::var("MYSERVER_AUTO_JOIN_ROOM").ok())
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "room-default".to_string()),
            myserver_policy_id: env::var("AUTHORITY_MYSERVER_POLICY")
                .ok()
                .or_else(|| env::var("MYSERVER_AUTO_JOIN_POLICY").ok())
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "movement_demo".to_string()),
        }
    }
}

#[derive(Resource, Debug)]
struct AuthorityDevState {
    input_timer: Timer,
    input_seq: u32,
    myserver_join_sent: bool,
    myserver_ready_sent: bool,
    myserver_start_sent: bool,
    last_logged_frame: u32,
}

impl Default for AuthorityDevState {
    fn default() -> Self {
        Self {
            input_timer: Timer::from_seconds(0.5, TimerMode::Repeating),
            input_seq: 0,
            myserver_join_sent: false,
            myserver_ready_sent: false,
            myserver_start_sent: false,
            last_logged_frame: 0,
        }
    }
}

fn handle_authority_commands(
    mut session: ResMut<AuthoritySession>,
    mut commands: MessageReader<AuthorityCommand>,
    mut network_commands: MessageWriter<NetworkCommand>,
    mut myserver_commands: MessageWriter<MyServerCommand>,
    mut events: MessageWriter<AuthorityEvent>,
) {
    for command in commands.read() {
        match command {
            AuthorityCommand::HostLocal { player_id } => {
                start_local_host(&mut session, &mut events, player_id.clone());
            }
            AuthorityCommand::HostLan {
                player_id,
                bind_addr,
                transport,
            } => {
                start_lan_host(
                    &mut session,
                    &mut network_commands,
                    &mut events,
                    player_id.clone(),
                    bind_addr.clone(),
                    *transport,
                );
            }
            AuthorityCommand::Join {
                player_id,
                endpoint,
            } => join_authority(
                &mut session,
                &mut network_commands,
                &mut events,
                player_id.clone(),
                endpoint.clone(),
            ),
            AuthorityCommand::SwitchAuthority {
                endpoint,
                migration,
            } => switch_authority(
                &mut session,
                &mut network_commands,
                &mut events,
                endpoint.clone(),
                migration.clone(),
            ),
            AuthorityCommand::Leave => leave_authority(&mut session, &mut network_commands),
            AuthorityCommand::SendInput {
                frame_id,
                action,
                payload_json,
            } => send_player_input(
                &mut session,
                &mut network_commands,
                &mut myserver_commands,
                &mut events,
                *frame_id,
                action.clone(),
                payload_json.clone(),
            ),
            AuthorityCommand::Tick => {
                if session.role == Some(AuthorityRole::Host) {
                    apply_authority_tick(&mut session, &mut network_commands, &mut events);
                }
            }
        }
    }
}

fn authority_dev_startup(
    config: Res<AuthorityDevConfig>,
    mut state: ResMut<AuthorityDevState>,
    mut authority_commands: MessageWriter<AuthorityCommand>,
    mut myserver_commands: MessageWriter<MyServerCommand>,
) {
    state.input_timer = Timer::new(config.input_interval, TimerMode::Repeating);

    match config.mode {
        AuthorityDevMode::Off => {}
        AuthorityDevMode::LocalHost => {
            info!(
                player_id = %config.player_id,
                "authority dev starting local host"
            );
            authority_commands.write(AuthorityCommand::HostLocal {
                player_id: config.player_id.clone(),
            });
        }
        AuthorityDevMode::LanHost => {
            info!(
                player_id = %config.player_id,
                bind_addr = %config.bind_addr,
                transport = ?config.transport,
                "authority dev starting LAN host"
            );
            authority_commands.write(AuthorityCommand::HostLan {
                player_id: config.player_id.clone(),
                bind_addr: config.bind_addr.clone(),
                transport: config.transport,
            });
        }
        AuthorityDevMode::LanClient => {
            info!(
                player_id = %config.player_id,
                host = %config.remote_host,
                port = config.remote_port,
                transport = ?config.transport,
                "authority dev joining LAN authority"
            );
            authority_commands.write(AuthorityCommand::Join {
                player_id: config.player_id.clone(),
                endpoint: AuthorityEndpoint::Remote {
                    host: config.remote_host.clone(),
                    port: config.remote_port,
                    transport: config.transport,
                },
            });
        }
        AuthorityDevMode::MyServer => {
            info!(
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                "authority dev starting MyServer login"
            );
            authority_commands.write(AuthorityCommand::Join {
                player_id: config.player_id.clone(),
                endpoint: AuthorityEndpoint::MyServer {
                    host: None,
                    port: None,
                    transport: config.transport,
                },
            });
            myserver_commands.write(MyServerCommand::GuestLogin {
                guest_id: config.myserver_guest_id.clone(),
                connect_game: true,
            });
        }
    }
}

fn authority_dev_follow_myserver(
    config: Res<AuthorityDevConfig>,
    mut state: ResMut<AuthorityDevState>,
    mut myserver_events: MessageReader<MyServerEvent>,
    mut myserver_commands: MessageWriter<MyServerCommand>,
) {
    if config.mode != AuthorityDevMode::MyServer {
        return;
    }

    for event in myserver_events.read() {
        match event {
            MyServerEvent::Authenticated { .. } if !state.myserver_join_sent => {
                state.myserver_join_sent = true;
                myserver_commands.write(MyServerCommand::JoinRoom {
                    room_id: config.myserver_room_id.clone(),
                    policy_id: config.myserver_policy_id.clone(),
                });
            }
            MyServerEvent::RoomJoined(response) if response.ok && !state.myserver_ready_sent => {
                state.myserver_ready_sent = true;
                myserver_commands.write(MyServerCommand::SetReady { ready: true });
            }
            MyServerEvent::ReadyChanged(response) if response.ok && !state.myserver_start_sent => {
                state.myserver_start_sent = true;
                myserver_commands.write(MyServerCommand::StartRoom);
            }
            _ => {}
        }
    }
}

fn authority_dev_auto_input(
    config: Res<AuthorityDevConfig>,
    time: Res<Time>,
    session: Res<AuthoritySession>,
    mut state: ResMut<AuthorityDevState>,
    mut commands: MessageWriter<AuthorityCommand>,
) {
    if config.mode == AuthorityDevMode::Off || !config.auto_input {
        return;
    }
    if session.local_player_id.is_none() {
        return;
    }

    state.input_timer.tick(time.delta());
    if !state.input_timer.just_finished() {
        return;
    }

    state.input_seq = state.input_seq.saturating_add(1);
    let frame_id = session.frame_id.saturating_add(1);
    let dir_x = if state.input_seq % 2 == 0 { 1.0 } else { -1.0 };
    let payload_json = serde_json::json!({
        "seq": state.input_seq,
        "dirX": dir_x,
        "dirY": 0.0
    })
    .to_string();

    commands.write(AuthorityCommand::SendInput {
        frame_id,
        action: "move_dir".to_string(),
        payload_json,
    });
}

fn authority_dev_log_events(
    config: Res<AuthorityDevConfig>,
    mut state: ResMut<AuthorityDevState>,
    mut events: MessageReader<AuthorityEvent>,
) {
    if config.mode == AuthorityDevMode::Off {
        return;
    }

    for event in events.read() {
        match event {
            AuthorityEvent::Hosting {
                listener_id,
                endpoint,
            } => {
                info!(?listener_id, ?endpoint, "authority dev hosting");
            }
            AuthorityEvent::Connecting { endpoint } => {
                info!(?endpoint, "authority dev connecting");
            }
            AuthorityEvent::Connected {
                endpoint,
                player_id,
            } => {
                info!(?endpoint, %player_id, "authority dev connected");
            }
            AuthorityEvent::ConnectionFailed { endpoint, error } => {
                error!(?endpoint, %error, "authority dev connection failed");
            }
            AuthorityEvent::PeerJoined {
                player_id,
                connection_id,
            } => {
                info!(%player_id, ?connection_id, "authority dev peer joined");
            }
            AuthorityEvent::PeerLeft { player_id } => {
                info!(%player_id, "authority dev peer left");
            }
            AuthorityEvent::InputAccepted { frame_id } => {
                info!(frame_id = *frame_id, "authority dev input accepted");
            }
            AuthorityEvent::FrameApplied { frame } => {
                if frame.frame_id != state.last_logged_frame {
                    state.last_logged_frame = frame.frame_id;
                    info!(
                        frame_id = frame.frame_id,
                        input_count = frame.inputs.len(),
                        players = ?frame.snapshot.players,
                        "authority dev frame applied"
                    );
                }
            }
            AuthorityEvent::Snapshot { snapshot } => {
                info!(
                    frame_id = snapshot.frame_id,
                    authority = %snapshot.authority_player_id,
                    players = ?snapshot.players,
                    "authority dev snapshot"
                );
            }
            AuthorityEvent::MigrationStarted { migration } => {
                info!(
                    epoch = migration.authority_epoch,
                    frozen_frame_id = migration.frozen_frame_id,
                    new_authority = %migration.new_authority_player_id,
                    "authority dev migration started"
                );
            }
            AuthorityEvent::MigrationCompleted { authority_epoch } => {
                info!(
                    epoch = *authority_epoch,
                    "authority dev migration completed"
                );
            }
            AuthorityEvent::Disconnected { reason } => {
                warn!(?reason, "authority dev disconnected");
            }
            AuthorityEvent::ProtocolError { error } => {
                error!(%error, "authority dev protocol error");
            }
            AuthorityEvent::HostFailed { error } => {
                error!(%error, "authority dev host failed");
            }
        }
    }
}

fn start_local_host(
    session: &mut AuthoritySession,
    events: &mut MessageWriter<AuthorityEvent>,
    player_id: String,
) {
    session.reset();
    let epoch = session.next_epoch();
    session.role = Some(AuthorityRole::Host);
    session.local_loopback = true;
    session.local_player_id = Some(player_id.clone());
    session.authority_player_id = Some(player_id.clone());
    session.fps = DEFAULT_AUTHORITY_FPS;
    session.endpoint = Some(AuthorityEndpoint::LocalLoopback);
    session.peers.insert(
        player_id.clone(),
        AuthorityPeer {
            player_id: player_id.clone(),
            connection_id: None,
            connected: true,
        },
    );

    events.write(AuthorityEvent::Hosting {
        listener_id: None,
        endpoint: AuthorityEndpoint::LocalLoopback,
    });
    events.write(AuthorityEvent::Connected {
        endpoint: AuthorityEndpoint::LocalLoopback,
        player_id: player_id.clone(),
    });
    events.write(AuthorityEvent::Snapshot {
        snapshot: build_snapshot(session, epoch),
    });
}

fn start_lan_host(
    session: &mut AuthoritySession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<AuthorityEvent>,
    player_id: String,
    bind_addr: String,
    transport: NetworkTransport,
) {
    session.reset();
    let epoch = session.next_epoch();
    let listener_id = ListenerId::new();
    session.role = Some(AuthorityRole::Host);
    session.local_player_id = Some(player_id.clone());
    session.authority_player_id = Some(player_id.clone());
    session.fps = DEFAULT_AUTHORITY_FPS;
    session.listener_id = Some(listener_id);
    session.endpoint = Some(AuthorityEndpoint::Remote {
        host: bind_addr.clone(),
        port: 0,
        transport,
    });
    session.peers.insert(
        player_id.clone(),
        AuthorityPeer {
            player_id: player_id.clone(),
            connection_id: None,
            connected: true,
        },
    );

    match transport {
        NetworkTransport::Tcp => {
            network_commands.write(NetworkCommand::ListenTcp(
                TcpListenConfig::new(bind_addr.clone()).with_listener_id(listener_id),
            ));
        }
        NetworkTransport::Kcp => {
            network_commands.write(NetworkCommand::ListenKcp(
                KcpListenConfig::new(bind_addr.clone())
                    .with_listener_id(listener_id)
                    .with_session(KcpSessionOptions {
                        stream: true,
                        ..KcpSessionOptions::fastest()
                    }),
            ));
        }
    }

    events.write(AuthorityEvent::Hosting {
        listener_id: Some(listener_id),
        endpoint: AuthorityEndpoint::Remote {
            host: bind_addr,
            port: 0,
            transport,
        },
    });
    events.write(AuthorityEvent::Connected {
        endpoint: AuthorityEndpoint::LocalLoopback,
        player_id,
    });
    events.write(AuthorityEvent::Snapshot {
        snapshot: build_snapshot(session, epoch),
    });
}

fn join_authority(
    session: &mut AuthoritySession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<AuthorityEvent>,
    player_id: String,
    endpoint: AuthorityEndpoint,
) {
    leave_authority(session, network_commands);
    session.role = Some(AuthorityRole::Client);
    session.local_player_id = Some(player_id.clone());
    session.endpoint = Some(endpoint.clone());

    if matches!(endpoint, AuthorityEndpoint::MyServer { .. }) {
        events.write(AuthorityEvent::Connecting { endpoint });
        return;
    }

    if endpoint == AuthorityEndpoint::LocalLoopback {
        start_local_host(session, events, player_id);
        return;
    }

    let Some(remote_addr) = endpoint.remote_addr() else {
        events.write(AuthorityEvent::ConnectionFailed {
            endpoint,
            error: "authority endpoint does not contain a remote address".to_string(),
        });
        return;
    };
    let Some(transport) = endpoint.transport() else {
        events.write(AuthorityEvent::ConnectionFailed {
            endpoint,
            error: "authority endpoint does not contain a transport".to_string(),
        });
        return;
    };

    let connection_id = ConnectionId::new();
    session.server_connection_id = Some(connection_id);
    events.write(AuthorityEvent::Connecting {
        endpoint: endpoint.clone(),
    });

    match transport {
        NetworkTransport::Tcp => {
            network_commands.write(NetworkCommand::ConnectTcp(
                TcpConnectConfig::new(remote_addr).with_connection_id(connection_id),
            ));
        }
        NetworkTransport::Kcp => {
            network_commands.write(NetworkCommand::ConnectKcp(
                KcpConnectConfig::new(remote_addr)
                    .with_connection_id(connection_id)
                    .with_session(KcpSessionOptions {
                        stream: true,
                        ..KcpSessionOptions::fastest()
                    }),
            ));
        }
    }
}

fn switch_authority(
    session: &mut AuthoritySession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<AuthorityEvent>,
    endpoint: AuthorityEndpoint,
    migration: AuthorityMigration,
) {
    events.write(AuthorityEvent::MigrationStarted {
        migration: migration.clone(),
    });
    session.authority_epoch = migration.authority_epoch;
    session.frame_id = migration.frozen_frame_id;
    session.authority_player_id = Some(migration.new_authority_player_id.clone());
    session.pending_inputs.clear();
    for input in &migration.pending_inputs {
        session
            .pending_inputs
            .entry(input.frame_id)
            .or_default()
            .push(input.clone());
    }

    if session.local_player_id.as_deref() == Some(migration.new_authority_player_id.as_str()) {
        session.role = Some(AuthorityRole::Host);
        session.local_loopback = matches!(endpoint, AuthorityEndpoint::LocalLoopback);
        match endpoint.clone() {
            AuthorityEndpoint::Remote {
                host,
                port,
                transport,
            } => {
                let listener_id = ListenerId::new();
                session.listener_id = Some(listener_id);
                let bind_addr = format!("{host}:{port}");
                match transport {
                    NetworkTransport::Tcp => {
                        network_commands.write(NetworkCommand::ListenTcp(
                            TcpListenConfig::new(bind_addr).with_listener_id(listener_id),
                        ));
                    }
                    NetworkTransport::Kcp => {
                        network_commands.write(NetworkCommand::ListenKcp(
                            KcpListenConfig::new(bind_addr)
                                .with_listener_id(listener_id)
                                .with_session(KcpSessionOptions {
                                    stream: true,
                                    ..KcpSessionOptions::fastest()
                                }),
                        ));
                    }
                }
            }
            AuthorityEndpoint::LocalLoopback | AuthorityEndpoint::MyServer { .. } => {}
        }
        events.write(AuthorityEvent::MigrationCompleted {
            authority_epoch: session.authority_epoch,
        });
        events.write(AuthorityEvent::Snapshot {
            snapshot: migration.snapshot,
        });
        return;
    }

    let player_id = session.local_player_id.clone().unwrap_or_default();
    join_authority(session, network_commands, events, player_id, endpoint);
}

fn leave_authority(
    session: &mut AuthoritySession,
    network_commands: &mut MessageWriter<NetworkCommand>,
) {
    if let Some(connection_id) = session.server_connection_id.take() {
        network_commands.write(NetworkCommand::Disconnect { connection_id });
    }
    for peer in session.peers.values() {
        if let Some(connection_id) = peer.connection_id {
            network_commands.write(NetworkCommand::Disconnect { connection_id });
        }
    }
    if let Some(listener_id) = session.listener_id.take() {
        network_commands.write(NetworkCommand::StopListener { listener_id });
    }
    session.reset();
}

fn send_player_input(
    session: &mut AuthoritySession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    myserver_commands: &mut MessageWriter<MyServerCommand>,
    events: &mut MessageWriter<AuthorityEvent>,
    frame_id: u32,
    action: String,
    payload_json: String,
) {
    let Some(player_id) = session.local_player_id.clone() else {
        events.write(AuthorityEvent::ProtocolError {
            error: "cannot send input without a local player id".to_string(),
        });
        return;
    };

    let input = PlayerInput {
        player_id,
        frame_id,
        action,
        payload_json,
    };

    if session.role == Some(AuthorityRole::Host) {
        queue_host_input(session, input.clone());
        events.write(AuthorityEvent::InputAccepted { frame_id });
        return;
    }

    if matches!(session.endpoint, Some(AuthorityEndpoint::MyServer { .. })) {
        myserver_commands.write(MyServerCommand::SendPlayerInput {
            frame_id,
            action: input.action,
            payload_json: input.payload_json,
        });
        return;
    }

    let Some(connection_id) = session.server_connection_id else {
        events.write(AuthorityEvent::ProtocolError {
            error: "cannot send input without an authority connection".to_string(),
        });
        return;
    };

    send_wire(
        network_commands,
        connection_id,
        &AuthorityWireMessage::Input(input),
        events,
    );
}

fn handle_authority_network_events(
    mut session: ResMut<AuthoritySession>,
    mut network_events: MessageReader<NetworkEvent>,
    mut network_commands: MessageWriter<NetworkCommand>,
    mut events: MessageWriter<AuthorityEvent>,
) {
    for event in network_events.read() {
        match event {
            NetworkEvent::Listening {
                listener_id,
                local_addr,
                ..
            } if Some(*listener_id) == session.listener_id => {
                let endpoint = endpoint_from_local_addr(local_addr, event);
                session.endpoint = Some(endpoint.clone());
                events.write(AuthorityEvent::Hosting {
                    listener_id: Some(*listener_id),
                    endpoint,
                });
            }
            NetworkEvent::ListenFailed {
                listener_id, error, ..
            } if Some(*listener_id) == session.listener_id => {
                events.write(AuthorityEvent::HostFailed {
                    error: error.clone(),
                });
            }
            NetworkEvent::Accepted {
                listener_id,
                connection_id,
                ..
            } if Some(*listener_id) == session.listener_id => {
                session
                    .packet_codecs
                    .entry(*connection_id)
                    .or_default()
                    .clear();
            }
            NetworkEvent::Connected { connection_id, .. }
                if Some(*connection_id) == session.server_connection_id =>
            {
                let endpoint = session
                    .endpoint
                    .clone()
                    .unwrap_or(AuthorityEndpoint::LocalLoopback);
                let player_id = session.local_player_id.clone().unwrap_or_default();
                send_wire(
                    &mut network_commands,
                    *connection_id,
                    &AuthorityWireMessage::Hello {
                        protocol_version: AUTHORITY_PROTOCOL_VERSION,
                        player_id: player_id.clone(),
                        authority_epoch: session.authority_epoch,
                    },
                    &mut events,
                );
                events.write(AuthorityEvent::Connected {
                    endpoint,
                    player_id,
                });
            }
            NetworkEvent::ConnectionFailed {
                connection_id,
                error,
                ..
            } if Some(*connection_id) == session.server_connection_id => {
                let endpoint = session
                    .endpoint
                    .clone()
                    .unwrap_or(AuthorityEndpoint::LocalLoopback);
                events.write(AuthorityEvent::ConnectionFailed {
                    endpoint,
                    error: error.clone(),
                });
            }
            NetworkEvent::Packet {
                connection_id,
                payload,
                ..
            } if session.role == Some(AuthorityRole::Host)
                && session.packet_codecs.contains_key(connection_id) =>
            {
                handle_host_packet(
                    &mut session,
                    &mut network_commands,
                    &mut events,
                    *connection_id,
                    payload,
                );
            }
            NetworkEvent::Packet {
                connection_id,
                payload,
                ..
            } if Some(*connection_id) == session.server_connection_id => {
                handle_client_packet(&mut session, &mut events, *connection_id, payload);
            }
            NetworkEvent::Disconnected {
                connection_id,
                reason,
                ..
            } if Some(*connection_id) == session.server_connection_id => {
                session.server_connection_id = None;
                events.write(AuthorityEvent::Disconnected {
                    reason: reason.clone(),
                });
            }
            NetworkEvent::Disconnected {
                connection_id,
                reason: _,
                ..
            } if session.role == Some(AuthorityRole::Host)
                && session.packet_codecs.contains_key(connection_id) =>
            {
                session.packet_codecs.remove(connection_id);
                if let Some(player_id) = session.connection_players.remove(connection_id) {
                    if let Some(peer) = session.peers.get_mut(&player_id) {
                        peer.connected = false;
                    }
                    events.write(AuthorityEvent::PeerLeft {
                        player_id: player_id.clone(),
                    });
                    broadcast_snapshot_event(
                        &mut session,
                        &mut network_commands,
                        &mut events,
                        "peer disconnected",
                    );
                }
            }
            _ => {}
        }
    }
}

fn handle_myserver_authority_events(
    mut session: ResMut<AuthoritySession>,
    mut myserver_events: MessageReader<MyServerEvent>,
    mut events: MessageWriter<AuthorityEvent>,
) {
    let is_myserver_endpoint = matches!(session.endpoint, Some(AuthorityEndpoint::MyServer { .. }));

    for event in myserver_events.read() {
        match event {
            MyServerEvent::Connecting {
                transport,
                remote_addr,
                ..
            } if is_myserver_endpoint => {
                let (host, port) = parse_addr(remote_addr);
                events.write(AuthorityEvent::Connecting {
                    endpoint: AuthorityEndpoint::MyServer {
                        host: Some(host),
                        port: Some(port),
                        transport: *transport,
                    },
                });
            }
            MyServerEvent::Authenticated { player_id } => {
                if !is_myserver_endpoint {
                    continue;
                }
                session.role = Some(AuthorityRole::Client);
                session.local_player_id = Some(player_id.clone());
                session.authority_player_id = Some("myserver".to_string());
                events.write(AuthorityEvent::Connected {
                    endpoint: session
                        .endpoint
                        .clone()
                        .unwrap_or(AuthorityEndpoint::MyServer {
                            host: None,
                            port: None,
                            transport: NetworkTransport::Tcp,
                        }),
                    player_id: player_id.clone(),
                });
            }
            MyServerEvent::RoomStatePush(push) if is_myserver_endpoint => {
                if let Some(snapshot) = push.snapshot.as_ref() {
                    session.frame_id = snapshot.current_frame_id;
                    events.write(AuthorityEvent::Snapshot {
                        snapshot: AuthoritySnapshot {
                            authority_epoch: session.authority_epoch,
                            frame_id: snapshot.current_frame_id,
                            authority_player_id: snapshot.owner_player_id.clone(),
                            players: snapshot
                                .members
                                .iter()
                                .map(|member| member.player_id.clone())
                                .collect(),
                            game_state_json: snapshot.game_state.clone(),
                        },
                    });
                }
            }
            MyServerEvent::FrameBundlePush(push) if is_myserver_endpoint => {
                let inputs = push
                    .inputs
                    .iter()
                    .map(|input| PlayerInput {
                        player_id: input.player_id.clone(),
                        frame_id: input.frame_id,
                        action: input.action.clone(),
                        payload_json: input.payload_json.clone(),
                    })
                    .collect::<Vec<_>>();
                let snapshot = push.snapshot.as_ref().map_or_else(
                    || AuthoritySnapshot {
                        authority_epoch: session.authority_epoch,
                        frame_id: push.frame_id,
                        authority_player_id: session
                            .authority_player_id
                            .clone()
                            .unwrap_or_default(),
                        players: session.peers.keys().cloned().collect(),
                        game_state_json: "{}".to_string(),
                    },
                    |snapshot| AuthoritySnapshot {
                        authority_epoch: session.authority_epoch,
                        frame_id: snapshot.current_frame_id,
                        authority_player_id: snapshot.owner_player_id.clone(),
                        players: snapshot
                            .members
                            .iter()
                            .map(|member| member.player_id.clone())
                            .collect(),
                        game_state_json: snapshot.game_state.clone(),
                    },
                );
                session.frame_id = session.frame_id.max(push.frame_id);
                events.write(AuthorityEvent::FrameApplied {
                    frame: AuthorityFrame {
                        authority_epoch: session.authority_epoch,
                        frame_id: push.frame_id,
                        fps: push.fps as u16,
                        inputs,
                        snapshot,
                    },
                });
            }
            MyServerEvent::Disconnected { reason } if is_myserver_endpoint => {
                events.write(AuthorityEvent::Disconnected {
                    reason: reason.clone(),
                });
            }
            MyServerEvent::ConnectionFailed { error, .. } if is_myserver_endpoint => {
                events.write(AuthorityEvent::ConnectionFailed {
                    endpoint: session
                        .endpoint
                        .clone()
                        .unwrap_or(AuthorityEndpoint::MyServer {
                            host: None,
                            port: None,
                            transport: NetworkTransport::Tcp,
                        }),
                    error: error.clone(),
                });
            }
            MyServerEvent::AuthorityMigrationStartPush(push) if is_myserver_endpoint => {
                if let Some(payload) = push.payload.as_ref() {
                    if let Some(migration) = migration_from_myserver_payload(payload) {
                        events.write(AuthorityEvent::MigrationStarted { migration });
                    }
                }
            }
            MyServerEvent::AuthorityMigrationCompletePush(push) if is_myserver_endpoint => {
                session.authority_epoch = push.authority_epoch;
                events.write(AuthorityEvent::MigrationCompleted {
                    authority_epoch: push.authority_epoch,
                });
                if let Some(snapshot) = push.snapshot.as_ref() {
                    events.write(AuthorityEvent::Snapshot {
                        snapshot: snapshot_from_myserver_authority(snapshot),
                    });
                }
            }
            _ => {}
        }
    }
}

fn migration_from_myserver_payload(
    payload: &crate::myserver::protocol::pb::AuthorityMigrationPayload,
) -> Option<AuthorityMigration> {
    let snapshot = payload.snapshot.as_ref()?;
    let new_authority = payload.new_authority.as_ref()?;
    Some(AuthorityMigration {
        authority_epoch: payload.authority_epoch,
        frozen_frame_id: payload.frozen_frame_id,
        new_authority_player_id: new_authority.player_id.clone(),
        snapshot: snapshot_from_myserver_authority(snapshot),
        pending_inputs: payload
            .pending_inputs
            .iter()
            .map(|input| PlayerInput {
                player_id: input.player_id.clone(),
                frame_id: input.frame_id,
                action: input.action.clone(),
                payload_json: input.payload_json.clone(),
            })
            .collect(),
        checksum: payload.checksum.clone(),
    })
}

fn snapshot_from_myserver_authority(
    snapshot: &crate::myserver::protocol::pb::AuthoritySnapshot,
) -> AuthoritySnapshot {
    AuthoritySnapshot {
        authority_epoch: snapshot.authority_epoch,
        frame_id: snapshot.frame_id,
        authority_player_id: snapshot.authority_player_id.clone(),
        players: snapshot.player_ids.clone(),
        game_state_json: snapshot.game_state_json.clone(),
    }
}

fn endpoint_from_local_addr(local_addr: &str, event: &NetworkEvent) -> AuthorityEndpoint {
    let transport = match event {
        NetworkEvent::Listening { transport, .. } => *transport,
        _ => NetworkTransport::Tcp,
    };
    let (host, port) = parse_addr(local_addr);
    AuthorityEndpoint::Remote {
        host,
        port,
        transport,
    }
}

fn parse_addr(addr: &str) -> (String, u16) {
    let Some((host, port)) = addr.rsplit_once(':') else {
        return (addr.to_string(), 0);
    };
    (host.to_string(), port.parse::<u16>().unwrap_or_default())
}

fn handle_host_packet(
    session: &mut AuthoritySession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<AuthorityEvent>,
    connection_id: ConnectionId,
    payload: &[u8],
) {
    let messages = {
        let codec = session.packet_codecs.entry(connection_id).or_default();
        match codec.push_bytes(payload) {
            Ok(messages) => messages,
            Err(error) => {
                events.write(AuthorityEvent::ProtocolError { error });
                return;
            }
        }
    };

    for message in messages {
        match message {
            AuthorityWireMessage::Hello {
                protocol_version,
                player_id,
                ..
            } => {
                if protocol_version != AUTHORITY_PROTOCOL_VERSION {
                    send_wire(
                        network_commands,
                        connection_id,
                        &AuthorityWireMessage::Error {
                            message: format!(
                                "unsupported authority protocol version: {protocol_version}"
                            ),
                        },
                        events,
                    );
                    continue;
                }
                register_peer(session, connection_id, player_id.clone());
                let snapshot = build_snapshot(session, session.authority_epoch);
                send_wire(
                    network_commands,
                    connection_id,
                    &AuthorityWireMessage::Welcome {
                        protocol_version: AUTHORITY_PROTOCOL_VERSION,
                        player_id: player_id.clone(),
                        authority_epoch: session.authority_epoch,
                        snapshot: snapshot.clone(),
                    },
                    events,
                );
                broadcast_to_peers(
                    session,
                    network_commands,
                    &AuthorityWireMessage::PlayerJoined {
                        player_id: player_id.clone(),
                        snapshot,
                    },
                    Some(connection_id),
                    events,
                );
                events.write(AuthorityEvent::PeerJoined {
                    player_id,
                    connection_id: Some(connection_id),
                });
            }
            AuthorityWireMessage::Input(input) => {
                let frame_id = input.frame_id;
                queue_host_input(session, input);
                send_wire(
                    network_commands,
                    connection_id,
                    &AuthorityWireMessage::InputAccepted { frame_id },
                    events,
                );
            }
            _ => {}
        }
    }
}

fn handle_client_packet(
    session: &mut AuthoritySession,
    events: &mut MessageWriter<AuthorityEvent>,
    _connection_id: ConnectionId,
    payload: &[u8],
) {
    let messages = match session.local_client_codec.push_bytes(payload) {
        Ok(messages) => messages,
        Err(error) => {
            events.write(AuthorityEvent::ProtocolError { error });
            return;
        }
    };

    for message in messages {
        match message {
            AuthorityWireMessage::Welcome {
                authority_epoch,
                snapshot,
                ..
            } => {
                session.authority_epoch = authority_epoch;
                session.authority_player_id = Some(snapshot.authority_player_id.clone());
                events.write(AuthorityEvent::Snapshot { snapshot });
            }
            AuthorityWireMessage::PlayerJoined {
                player_id,
                snapshot,
            } => {
                events.write(AuthorityEvent::PeerJoined {
                    player_id,
                    connection_id: None,
                });
                events.write(AuthorityEvent::Snapshot { snapshot });
            }
            AuthorityWireMessage::PlayerLeft {
                player_id,
                snapshot,
            } => {
                events.write(AuthorityEvent::PeerLeft { player_id });
                events.write(AuthorityEvent::Snapshot { snapshot });
            }
            AuthorityWireMessage::InputAccepted { frame_id } => {
                events.write(AuthorityEvent::InputAccepted { frame_id });
            }
            AuthorityWireMessage::Frame(frame) => {
                session.frame_id = session.frame_id.max(frame.frame_id);
                events.write(AuthorityEvent::FrameApplied { frame });
            }
            AuthorityWireMessage::Snapshot(snapshot) => {
                session.frame_id = session.frame_id.max(snapshot.frame_id);
                events.write(AuthorityEvent::Snapshot { snapshot });
            }
            AuthorityWireMessage::MigrationStart(migration) => {
                events.write(AuthorityEvent::MigrationStarted { migration });
            }
            AuthorityWireMessage::MigrationComplete { authority_epoch } => {
                events.write(AuthorityEvent::MigrationCompleted { authority_epoch });
            }
            AuthorityWireMessage::Error { message } => {
                events.write(AuthorityEvent::ProtocolError { error: message });
            }
            _ => {}
        }
    }
}

fn tick_local_authority(
    mut session: ResMut<AuthoritySession>,
    mut network_commands: MessageWriter<NetworkCommand>,
    mut events: MessageWriter<AuthorityEvent>,
) {
    if session.role != Some(AuthorityRole::Host) {
        return;
    }
    apply_authority_tick(&mut session, &mut network_commands, &mut events);
}

fn apply_authority_tick(
    session: &mut AuthoritySession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<AuthorityEvent>,
) {
    if session.peers.is_empty() {
        return;
    }

    let frame_id = session.frame_id.saturating_add(1);
    session.frame_id = frame_id;
    let mut inputs = session.pending_inputs.remove(&frame_id).unwrap_or_default();
    inputs.sort_by(|left, right| left.player_id.cmp(&right.player_id));
    let snapshot = build_snapshot(session, session.authority_epoch);
    let frame = AuthorityFrame {
        authority_epoch: session.authority_epoch,
        frame_id,
        fps: session.fps,
        inputs,
        snapshot,
    };

    broadcast_to_peers(
        session,
        network_commands,
        &AuthorityWireMessage::Frame(frame.clone()),
        None,
        events,
    );
    events.write(AuthorityEvent::FrameApplied { frame });
}

fn register_peer(session: &mut AuthoritySession, connection_id: ConnectionId, player_id: String) {
    session
        .connection_players
        .insert(connection_id, player_id.clone());
    session.peers.insert(
        player_id.clone(),
        AuthorityPeer {
            player_id,
            connection_id: Some(connection_id),
            connected: true,
        },
    );
}

fn queue_host_input(session: &mut AuthoritySession, input: PlayerInput) {
    session
        .pending_inputs
        .entry(input.frame_id)
        .or_default()
        .retain(|existing| existing.player_id != input.player_id);
    session
        .pending_inputs
        .entry(input.frame_id)
        .or_default()
        .push(input);
}

fn build_snapshot(session: &AuthoritySession, authority_epoch: u64) -> AuthoritySnapshot {
    let mut players = session
        .peers
        .values()
        .filter(|peer| peer.connected)
        .map(|peer| peer.player_id.clone())
        .collect::<Vec<_>>();
    players.sort();

    AuthoritySnapshot {
        authority_epoch,
        frame_id: session.frame_id,
        authority_player_id: session.authority_player_id.clone().unwrap_or_default(),
        players,
        game_state_json: "{}".to_string(),
    }
}

fn broadcast_snapshot_event(
    session: &mut AuthoritySession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<AuthorityEvent>,
    _reason: &str,
) {
    let snapshot = build_snapshot(session, session.authority_epoch);
    broadcast_to_peers(
        session,
        network_commands,
        &AuthorityWireMessage::Snapshot(snapshot.clone()),
        None,
        events,
    );
    events.write(AuthorityEvent::Snapshot { snapshot });
}

fn broadcast_to_peers(
    session: &AuthoritySession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    message: &AuthorityWireMessage,
    except_connection_id: Option<ConnectionId>,
    events: &mut MessageWriter<AuthorityEvent>,
) {
    for peer in session.peers.values() {
        let Some(connection_id) = peer.connection_id else {
            continue;
        };
        if Some(connection_id) == except_connection_id {
            continue;
        }
        send_wire(network_commands, connection_id, message, events);
    }
}

fn send_wire(
    network_commands: &mut MessageWriter<NetworkCommand>,
    connection_id: ConnectionId,
    message: &AuthorityWireMessage,
    events: &mut MessageWriter<AuthorityEvent>,
) {
    match encode_authority_message(message) {
        Ok(payload) => {
            network_commands.write(NetworkCommand::Send {
                connection_id,
                payload,
            });
        }
        Err(error) => {
            events.write(AuthorityEvent::ProtocolError { error });
        }
    }
}

fn env_dev_mode(name: &str) -> AuthorityDevMode {
    match env::var(name)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "local" | "local-host" | "localhost" => AuthorityDevMode::LocalHost,
        "lan-host" | "host" => AuthorityDevMode::LanHost,
        "lan-client" | "client" | "join" => AuthorityDevMode::LanClient,
        "myserver" | "server" => AuthorityDevMode::MyServer,
        _ => AuthorityDevMode::Off,
    }
}

fn env_string(name: &str, default: &str) -> String {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn env_bool(name: &str, default: bool) -> bool {
    env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.as_str(),
                "1" | "true" | "TRUE" | "True" | "yes" | "YES"
            )
        })
        .unwrap_or(default)
}

fn env_u16(name: &str, default: u16) -> u16 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default)
}

fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_transport(name: &str) -> Option<NetworkTransport> {
    match env::var(name).ok()?.trim().to_ascii_lowercase().as_str() {
        "tcp" => Some(NetworkTransport::Tcp),
        "kcp" => Some(NetworkTransport::Kcp),
        _ => None,
    }
}
