mod http;
mod kcp;
mod runtime;
mod tcp;
mod types;

use bevy::prelude::*;

use runtime::NetworkRuntime;

pub use types::{
    ConnectionId, HttpMethod, HttpRequest, HttpResponse, KcpConnectConfig, KcpListenConfig,
    KcpSessionOptions, ListenerId, NetworkCommand, NetworkEvent, NetworkTransport, RequestId,
    TcpConnectConfig, TcpListenConfig,
};

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<NetworkCommand>()
            .add_message::<NetworkEvent>()
            .add_systems(Startup, setup_network_runtime)
            .add_systems(Update, (dispatch_network_commands, publish_network_events));
    }
}

fn setup_network_runtime(mut commands: Commands) {
    match NetworkRuntime::new() {
        Ok(runtime) => {
            commands.insert_resource(runtime);
        }
        Err(err) => {
            error!("{err}");
        }
    }
}

fn dispatch_network_commands(
    runtime: Option<Res<NetworkRuntime>>,
    mut commands: MessageReader<NetworkCommand>,
    mut events: MessageWriter<NetworkEvent>,
) {
    let Some(runtime) = runtime else {
        for command in commands.read() {
            report_unavailable_runtime(command, &mut events);
        }
        return;
    };

    for command in commands.read() {
        if let Err(err) = runtime.send(command.clone()) {
            report_command_error(command, err, &mut events);
        }
    }
}

fn publish_network_events(
    runtime: Option<Res<NetworkRuntime>>,
    mut events: MessageWriter<NetworkEvent>,
) {
    let Some(runtime) = runtime else {
        return;
    };

    for event in runtime.drain_events() {
        events.write(event);
    }
}

fn report_unavailable_runtime(command: &NetworkCommand, events: &mut MessageWriter<NetworkEvent>) {
    report_command_error(
        command,
        "network runtime is unavailable".to_string(),
        events,
    );
}

fn report_command_error(
    command: &NetworkCommand,
    error: String,
    events: &mut MessageWriter<NetworkEvent>,
) {
    match command {
        NetworkCommand::Http(request) => {
            events.write(NetworkEvent::HttpError {
                request_id: request.request_id,
                error,
            });
        }
        NetworkCommand::ConnectTcp(config) => {
            events.write(NetworkEvent::ConnectionFailed {
                connection_id: config.connection_id,
                transport: NetworkTransport::Tcp,
                remote_addr: config.addr.clone(),
                error,
            });
        }
        NetworkCommand::ConnectKcp(config) => {
            events.write(NetworkEvent::ConnectionFailed {
                connection_id: config.connection_id,
                transport: NetworkTransport::Kcp,
                remote_addr: config.addr.clone(),
                error,
            });
        }
        NetworkCommand::ListenTcp(config) => {
            events.write(NetworkEvent::ListenFailed {
                listener_id: config.listener_id,
                transport: NetworkTransport::Tcp,
                local_addr: config.addr.clone(),
                error,
            });
        }
        NetworkCommand::ListenKcp(config) => {
            events.write(NetworkEvent::ListenFailed {
                listener_id: config.listener_id,
                transport: NetworkTransport::Kcp,
                local_addr: config.addr.clone(),
                error,
            });
        }
        NetworkCommand::Send { connection_id, .. } => {
            events.write(NetworkEvent::SendFailed {
                connection_id: *connection_id,
                transport: None,
                error,
            });
        }
        NetworkCommand::Disconnect { connection_id } => {
            events.write(NetworkEvent::SendFailed {
                connection_id: *connection_id,
                transport: None,
                error,
            });
        }
        NetworkCommand::StopListener { listener_id } => {
            events.write(NetworkEvent::ListenFailed {
                listener_id: *listener_id,
                transport: NetworkTransport::Tcp,
                local_addr: String::new(),
                error,
            });
        }
    }
}
