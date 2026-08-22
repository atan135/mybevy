use bevy::prelude::*;

use crate::framework::devtools::live_preview::{
    LivePreviewClock, LivePreviewCollectionBuffer, LivePreviewSnapshotHub, LivePreviewTimeline,
    LivePreviewTimelineEvent, LivePreviewTimelineSeverity, LivePreviewTimelineType,
    NetworkPreviewState, PreviewSection, StablePreviewId,
};
use crate::framework::network::{NetworkEvent, NetworkTransport};
use crate::game::authority::{AuthorityEndpoint, AuthorityEvent, AuthorityRole, AuthoritySession};
use crate::game::myserver::{
    AccountLoginState, CharacterSelectionState, GameConnectionState, MyServerErrorKind,
    MyServerEvent, MyServerProfiles, MyServerSession, ReconnectCause, RegistrationState,
};
use crate::game::scenes::{
    main_world_entry::{MainWorldEntryPhase, MainWorldEntryState},
    main_world_snapshot::MainWorldSnapshotBusState,
};

/// Game-owned allowlist adapter. It never exposes credentials, request bodies,
/// endpoint addresses, or raw server error text.
pub trait NetworkPreviewAdapter {
    fn collect_network_preview(&self) -> NetworkPreviewState;
}

impl crate::game::devtools::live_preview::AuthorityPreviewAdapter
    for MyServerNetworkPreviewAdapter<'_>
{
    fn collect_authority_frame(&self) -> Option<u64> {
        self.authority.map(|authority| authority.frame_id as u64)
    }
}

pub(in crate::game) struct MyServerNetworkPreviewAdapter<'a> {
    pub session: &'a MyServerSession,
    pub profiles: Option<&'a MyServerProfiles>,
    pub authority: Option<&'a AuthoritySession>,
    pub entry: Option<&'a MainWorldEntryState>,
    pub snapshot_bus: Option<&'a MainWorldSnapshotBusState>,
    pub last_successful_receive_ms: Option<u64>,
    pub last_error_category: Option<&'a str>,
    pub authority_last_activity_age_ms: Option<u64>,
    pub session_status: Option<&'a str>,
}

impl NetworkPreviewAdapter for MyServerNetworkPreviewAdapter<'_> {
    fn collect_network_preview(&self) -> NetworkPreviewState {
        collect_network_preview_state(self)
    }
}

#[derive(Clone, Debug, Default, Resource)]
pub(in crate::game) struct NetworkPreviewCollectorState {
    pub last_state: Option<NetworkPreviewState>,
    pub revision: u64,
    pub last_successful_receive_ms: Option<u64>,
    pub last_error_category: Option<String>,
    pub last_authority_serial: u64,
    pub last_authority_activity_ms: Option<u64>,
    pub session_status: Option<String>,
}

pub(crate) struct GameNetworkLivePreviewPlugin;

impl Plugin for GameNetworkLivePreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetworkPreviewCollectorState>()
            .add_systems(
                PostUpdate,
                collect_network_preview
                    .in_set(crate::framework::devtools::live_preview::LivePreviewSet::Collect),
            );
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_network_preview(
    clock: Res<LivePreviewClock>,
    session: Option<Res<MyServerSession>>,
    profiles: Option<Res<MyServerProfiles>>,
    authority: Option<Res<AuthoritySession>>,
    entry: Option<Res<MainWorldEntryState>>,
    snapshot_bus: Option<Res<MainWorldSnapshotBusState>>,
    mut myserver_events: MessageReader<MyServerEvent>,
    mut network_events: MessageReader<NetworkEvent>,
    mut authority_events: MessageReader<AuthorityEvent>,
    mut collector_state: ResMut<NetworkPreviewCollectorState>,
    mut buffer: ResMut<LivePreviewCollectionBuffer>,
    hub: Res<LivePreviewSnapshotHub>,
    mut timeline: ResMut<LivePreviewTimeline>,
) {
    let now_ms = clock.monotonic_ms();
    for event in myserver_events.read() {
        if myserver_event_is_receive(event) {
            collector_state.last_successful_receive_ms = Some(now_ms);
        }
        if let Some(category) = myserver_event_error_category(event) {
            collector_state.last_error_category = Some(category.to_owned());
        }
        if matches!(event, MyServerEvent::SessionKicked { .. }) {
            collector_state.session_status = Some("session_kicked".to_owned());
        }
        if let Some(summary) = myserver_timeline_event(event) {
            push_network_timeline(
                &mut timeline,
                now_ms,
                next_publish_sequence(hub.read().sequence),
                summary,
            );
        }
    }
    for event in network_events.read() {
        match event {
            NetworkEvent::Packet { .. }
            | NetworkEvent::Connected { .. }
            | NetworkEvent::HttpResponse(_) => {
                collector_state.last_successful_receive_ms = Some(now_ms);
            }
            NetworkEvent::HttpError { .. }
            | NetworkEvent::ConnectionFailed { .. }
            | NetworkEvent::SendFailed { .. }
            | NetworkEvent::Disconnected { .. } => {
                collector_state.last_error_category =
                    Some(network_event_error_category(event).to_owned());
            }
            _ => {}
        }
    }
    for event in authority_events.read() {
        if let Some(summary) = authority_timeline_event(event) {
            push_network_timeline(
                &mut timeline,
                now_ms,
                next_publish_sequence(hub.read().sequence),
                summary,
            );
        }
    }

    if let Some(bus) = snapshot_bus.as_deref()
        && bus.authority_activity_serial != collector_state.last_authority_serial
    {
        let was_stale = collector_state
            .last_authority_activity_ms
            .is_some_and(|last| now_ms.saturating_sub(last) > 2_000);
        collector_state.last_authority_serial = bus.authority_activity_serial;
        collector_state.last_authority_activity_ms = Some(now_ms);
        if was_stale {
            push_network_timeline(
                &mut timeline,
                now_ms,
                next_publish_sequence(hub.read().sequence),
                "authority recovered",
            );
        }
    }

    let authority_age = collector_state
        .last_authority_activity_ms
        .map(|last| now_ms.saturating_sub(last));
    let next_state = session.as_deref().map_or_else(
        || NetworkPreviewState {
            login_status: Some("not_logged_in".to_owned()),
            registration_status: Some("idle".to_owned()),
            character_selection_status: Some("not_loaded".to_owned()),
            connection_state: Some("not_connected".to_owned()),
            connected: Some(false),
            authenticated: Some(false),
            reconnecting: Some(false),
            authority_sync_health: Some("not_applicable".to_owned()),
            session_status: collector_state.session_status.clone(),
            ..Default::default()
        },
        |session| {
            MyServerNetworkPreviewAdapter {
                session,
                profiles: profiles.as_deref(),
                authority: authority.as_deref(),
                entry: entry.as_deref(),
                snapshot_bus: snapshot_bus.as_deref(),
                last_successful_receive_ms: collector_state.last_successful_receive_ms,
                last_error_category: collector_state.last_error_category.as_deref(),
                authority_last_activity_age_ms: authority_age,
                session_status: collector_state.session_status.as_deref(),
            }
            .collect_network_preview()
        },
    );

    if collector_state.last_state.as_ref() == Some(&next_state) && collector_state.revision != 0 {
        return;
    }
    let previous = collector_state.last_state.replace(next_state.clone());
    collector_state.revision = collector_state.revision.saturating_add(1).max(1);
    buffer.set_network(PreviewSection::available(
        collector_state.revision,
        next_state.clone(),
    ));
    if let Some(previous) = previous.as_ref() {
        if previous.connection_state != next_state.connection_state {
            push_network_timeline(
                &mut timeline,
                now_ms,
                next_publish_sequence(hub.read().sequence),
                "network connection state changed",
            );
        }
        if previous.room_id != next_state.room_id {
            push_network_timeline(
                &mut timeline,
                now_ms,
                next_publish_sequence(hub.read().sequence),
                if next_state.room_id.is_some() {
                    "network room joined"
                } else {
                    "network room left"
                },
            );
        }
        if next_state.authority_sync_health.as_deref() == Some("stale")
            && previous.authority_sync_health != next_state.authority_sync_health
        {
            push_network_timeline(
                &mut timeline,
                now_ms,
                next_publish_sequence(hub.read().sequence),
                "authority stale",
            );
        }
    }
}

fn collect_network_preview_state(
    adapter: &MyServerNetworkPreviewAdapter<'_>,
) -> NetworkPreviewState {
    let session = adapter.session;
    let endpoint_environment = adapter
        .profiles
        .map(|profiles| environment_label(profiles.selected()).to_owned());
    let endpoint_kind = session
        .game_endpoint
        .as_ref()
        .map(|_| "game_proxy".to_owned());
    let reconnecting = matches!(
        session.game_connection_state,
        GameConnectionState::Reconnecting | GameConnectionState::ReconnectFailed
    ) || session.reconnect_after_auth.is_some();
    let reconnect_phase = session.reconnect_after_auth.as_ref().map(|plan| {
        match plan.cause {
            ReconnectCause::ServerRedirect { .. } => "server_redirect",
            ReconnectCause::TransportRecovery => "transport_recovery",
        }
        .to_owned()
    });
    let authority_endpoint_kind = adapter
        .authority
        .and_then(|authority| authority.endpoint.as_ref().map(authority_endpoint_kind));
    let authority_role = adapter
        .authority
        .and_then(|authority| authority.role.map(authority_role_label));
    let authority_epoch = adapter.authority.map(|authority| authority.authority_epoch);
    let authority_frame = adapter
        .authority
        .map(|authority| authority.frame_id as u64)
        .or_else(|| adapter.entry.map(|entry| entry.snapshot_generation as u64));
    let has_authority = authority_endpoint_kind.is_some()
        || adapter
            .entry
            .is_some_and(|entry| entry.authority_connection_id.is_some())
        || adapter
            .snapshot_bus
            .is_some_and(|bus| bus.authority_activity_serial > 0);

    NetworkPreviewState {
        login_status: Some(account_login_label(session.account_login_state).to_owned()),
        registration_status: Some(registration_label(session.registration_state).to_owned()),
        character_selection_status: Some(
            selection_label(session.character_selection_state).to_owned(),
        ),
        connection_state: Some(connection_label(session.game_connection_state).to_owned()),
        transport: session.transport.map(transport_label).map(str::to_owned),
        connected: Some(session.connected),
        authenticated: Some(session.authenticated),
        room_id: session
            .room_id
            .as_deref()
            .and_then(non_empty)
            .map(StablePreviewId::new),
        endpoint_kind,
        endpoint_environment,
        endpoint_detail: None,
        pending_request_count: Some((session.pending.len() + session.pending_http.len()) as u32),
        last_successful_receive_ms: adapter.last_successful_receive_ms,
        last_error_category: adapter.last_error_category.map(str::to_owned),
        reconnecting: Some(reconnecting),
        reconnect_phase,
        authority_endpoint_kind,
        authority_role,
        authority_epoch,
        authority_frame,
        authority_last_activity_age_ms: adapter.authority_last_activity_age_ms,
        authority_sync_health: Some(
            authority_health(
                has_authority,
                adapter.entry,
                adapter.authority_last_activity_age_ms,
            )
            .to_owned(),
        ),
        session_status: adapter.session_status.map(str::to_owned),
    }
}

fn authority_health(
    has_authority: bool,
    entry: Option<&MainWorldEntryState>,
    age_ms: Option<u64>,
) -> &'static str {
    if !has_authority {
        return "not_applicable";
    }
    if entry.is_some_and(|entry| {
        entry.phase == MainWorldEntryPhase::Recovering || entry.reconnect_requested
    }) {
        return "recovering";
    }
    match age_ms {
        Some(age) if age > 2_000 => "stale",
        Some(_) => "healthy",
        None => "awaiting_snapshot",
    }
}

fn authority_endpoint_kind(endpoint: &AuthorityEndpoint) -> String {
    match endpoint {
        AuthorityEndpoint::LocalLoopback => "local".to_owned(),
        AuthorityEndpoint::Remote { .. } => "lan".to_owned(),
        AuthorityEndpoint::MyServer { .. } => "myserver".to_owned(),
    }
}

fn authority_role_label(role: AuthorityRole) -> String {
    match role {
        AuthorityRole::None => "none",
        AuthorityRole::Host => "host",
        AuthorityRole::Client => "client",
    }
    .to_owned()
}

fn account_login_label(state: AccountLoginState) -> &'static str {
    match state {
        AccountLoginState::NotLoggedIn => "not_logged_in",
        AccountLoginState::LoggingIn => "logging_in",
        AccountLoginState::LoggedIn => "logged_in",
        AccountLoginState::LoginFailed => "login_failed",
        AccountLoginState::Blocked => "blocked",
        AccountLoginState::Expired => "expired",
        AccountLoginState::LoggedOut => "logged_out",
    }
}

fn registration_label(state: RegistrationState) -> &'static str {
    match state {
        RegistrationState::Idle => "idle",
        RegistrationState::Registering => "registering",
        RegistrationState::PendingReview => "pending_review",
        RegistrationState::Failed => "failed",
    }
}

fn selection_label(state: CharacterSelectionState) -> &'static str {
    match state {
        CharacterSelectionState::NotLoaded => "not_loaded",
        CharacterSelectionState::Loading => "loading",
        CharacterSelectionState::NoCharacters => "no_characters",
        CharacterSelectionState::Creating => "creating",
        CharacterSelectionState::AwaitingSelection => "awaiting_selection",
        CharacterSelectionState::LoadingProfile => "loading_profile",
        CharacterSelectionState::Selecting => "selecting",
        CharacterSelectionState::Selected => "selected",
        CharacterSelectionState::Blocked => "blocked",
        CharacterSelectionState::SelectionFailed => "selection_failed",
    }
}

fn connection_label(state: GameConnectionState) -> &'static str {
    match state {
        GameConnectionState::NotConnected => "not_connected",
        GameConnectionState::Connecting => "connecting",
        GameConnectionState::Connected => "connected",
        GameConnectionState::Authenticating => "authenticating",
        GameConnectionState::Authenticated => "authenticated",
        GameConnectionState::Disconnected => "disconnected",
        GameConnectionState::Reconnecting => "reconnecting",
        GameConnectionState::ReconnectFailed => "reconnect_failed",
    }
}

fn environment_label(environment: crate::game::myserver::MyServerEnvironment) -> &'static str {
    match environment {
        crate::game::myserver::MyServerEnvironment::Local => "local",
        crate::game::myserver::MyServerEnvironment::Production => "production",
    }
}

fn transport_label(transport: NetworkTransport) -> &'static str {
    match transport {
        NetworkTransport::Tcp => "tcp",
        NetworkTransport::Kcp => "kcp",
    }
}

fn non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn myserver_event_is_receive(event: &MyServerEvent) -> bool {
    matches!(
        event,
        MyServerEvent::LoginSucceeded(_)
            | MyServerEvent::RegistrationPendingReview { .. }
            | MyServerEvent::CharacterListLoaded { .. }
            | MyServerEvent::CharacterProfileLoaded { .. }
            | MyServerEvent::CharacterSelected { .. }
            | MyServerEvent::Connected { .. }
            | MyServerEvent::Authenticated { .. }
            | MyServerEvent::ReauthenticatedForReconnect { .. }
            | MyServerEvent::RoomJoined(_)
            | MyServerEvent::RoomJoinedScoped { .. }
            | MyServerEvent::RoomLeft(_)
            | MyServerEvent::RoomLeftScoped { .. }
            | MyServerEvent::RoomReconnected(_)
            | MyServerEvent::RoomReconnectedScoped { .. }
            | MyServerEvent::Pong(_)
            | MyServerEvent::MovementSnapshotPush(_)
            | MyServerEvent::FrameBundlePush(_)
            | MyServerEvent::RoomStatePush(_)
    )
}

fn myserver_event_error_category(event: &MyServerEvent) -> Option<&'static str> {
    match event {
        MyServerEvent::DisplayError { error } => Some(error_kind_category(error.kind)),
        MyServerEvent::LoginFailed { .. } => Some("login_failed"),
        MyServerEvent::ConnectionFailed { .. } => Some("connection_failed"),
        MyServerEvent::AuthFailed { .. } | MyServerEvent::GameAuthRejected { .. } => {
            Some("auth_rejected")
        }
        MyServerEvent::NetworkFailed { .. }
        | MyServerEvent::RequestFailed { .. }
        | MyServerEvent::ScopedRequestFailed { .. } => Some("request_failed"),
        MyServerEvent::ProtocolError { .. } => Some("protocol_error"),
        MyServerEvent::Disconnected { .. } => Some("disconnected"),
        MyServerEvent::Error { error_code, .. } => Some(classify_error_code(error_code)),
        MyServerEvent::SessionKicked { .. } => Some("session_kicked"),
        _ => None,
    }
}

fn error_kind_category(kind: MyServerErrorKind) -> &'static str {
    match kind {
        MyServerErrorKind::ConnectionTimeout => "connection_timeout",
        MyServerErrorKind::TransportFailed => "transport_failed",
        MyServerErrorKind::ProtocolError
        | MyServerErrorKind::ProtobufDecodeFailed
        | MyServerErrorKind::JsonParseFailed => "protocol_error",
        MyServerErrorKind::GameAuthRejected
        | MyServerErrorKind::TicketExpired
        | MyServerErrorKind::MissingCharacterId
        | MyServerErrorKind::Unauthorized => "auth_rejected",
        MyServerErrorKind::RoomJoinFailed => "room_join_failed",
        MyServerErrorKind::ServerRedirectFailed => "redirect_failed",
        MyServerErrorKind::SessionKicked => "session_kicked",
        _ => "server_error",
    }
}

fn classify_error_code(code: &str) -> &'static str {
    let code = code.to_ascii_uppercase();
    if code.contains("TIMEOUT") {
        "request_timeout"
    } else if code.contains("AUTH") || code.contains("TOKEN") || code.contains("TICKET") {
        "auth_rejected"
    } else if code.contains("REDIRECT") {
        "redirect_failed"
    } else if code.contains("KICK") || code.contains("CONCURRENT") {
        "session_kicked"
    } else {
        "server_error"
    }
}

fn network_event_error_category(event: &NetworkEvent) -> &'static str {
    match event {
        NetworkEvent::ConnectionFailed { .. } => "connection_failed",
        NetworkEvent::Disconnected { .. } => "disconnected",
        NetworkEvent::HttpError { .. } => "http_error",
        NetworkEvent::SendFailed { .. } => "send_failed",
        _ => "network_error",
    }
}

fn myserver_timeline_event(event: &MyServerEvent) -> Option<&'static str> {
    match event {
        MyServerEvent::LoginSucceeded(_) => Some("network login succeeded"),
        MyServerEvent::LoginFailed { .. } => Some("network login failed"),
        MyServerEvent::RegistrationPendingReview { .. } => Some("network registration pending"),
        MyServerEvent::CharacterSelected { .. } => Some("network character selected"),
        MyServerEvent::Connecting { .. } => Some("network connecting"),
        MyServerEvent::Connected { .. } => Some("network connected"),
        MyServerEvent::Authenticated { .. } => Some("network authenticated"),
        MyServerEvent::ReauthenticatedForReconnect { .. } => {
            Some("network reconnect authenticated")
        }
        MyServerEvent::RoomJoined(_) | MyServerEvent::RoomJoinedScoped { .. } => {
            Some("network room joined")
        }
        MyServerEvent::RoomLeft(_) | MyServerEvent::RoomLeftScoped { .. } => {
            Some("network room left")
        }
        MyServerEvent::RoomReconnected(_) | MyServerEvent::RoomReconnectedScoped { .. } => {
            Some("network room reconnected")
        }
        MyServerEvent::Disconnected { .. } => Some("network disconnected"),
        MyServerEvent::ServerRedirectReconnectStarted { .. } => Some("network redirect reconnect"),
        MyServerEvent::SessionKicked { .. } => Some("network session kicked"),
        MyServerEvent::AuthFailed { .. } | MyServerEvent::GameAuthRejected { .. } => {
            Some("network authentication failed")
        }
        MyServerEvent::ProtocolError { .. } => Some("network protocol error"),
        _ => None,
    }
}

fn authority_timeline_event(event: &AuthorityEvent) -> Option<&'static str> {
    match event {
        AuthorityEvent::Connecting { .. } => Some("authority connecting"),
        AuthorityEvent::Connected { .. } => Some("authority connected"),
        AuthorityEvent::ConnectionFailed { .. } => Some("authority connection failed"),
        AuthorityEvent::Disconnected { .. } => Some("authority disconnected"),
        AuthorityEvent::MigrationStarted { .. } => Some("authority migration started"),
        AuthorityEvent::MigrationCompleted { .. } => Some("authority migration completed"),
        AuthorityEvent::ProtocolError { .. } => Some("authority protocol error"),
        _ => None,
    }
}

fn push_network_timeline(
    timeline: &mut LivePreviewTimeline,
    timestamp_ms: u64,
    snapshot_sequence: u64,
    summary: &str,
) {
    timeline.push(LivePreviewTimelineEvent::new(
        LivePreviewTimelineType::Network,
        LivePreviewTimelineSeverity::Info,
        timestamp_ms,
        snapshot_sequence,
        summary,
        None,
    ));
}

fn next_publish_sequence(current_sequence: u64) -> u64 {
    current_sequence.saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::authority::AuthorityEndpoint;
    use crate::game::myserver::ReconnectPlan;

    #[test]
    fn offline_state_is_explicit_and_endpoint_is_redacted() {
        let session = MyServerSession::default();
        let profiles = MyServerProfiles::default();
        let state = collect_network_preview_state(&MyServerNetworkPreviewAdapter {
            session: &session,
            profiles: Some(&profiles),
            authority: None,
            entry: None,
            snapshot_bus: None,
            last_successful_receive_ms: None,
            last_error_category: None,
            authority_last_activity_age_ms: None,
            session_status: None,
        });
        assert_eq!(state.login_status.as_deref(), Some("not_logged_in"));
        assert_eq!(state.connection_state.as_deref(), Some("not_connected"));
        assert_eq!(state.pending_request_count, Some(0));
        assert_eq!(state.endpoint_environment.as_deref(), Some("local"));
        assert_eq!(state.endpoint_detail, None);
        let json = serde_json::to_string(&state).unwrap();
        assert!(!json.contains("127.0.0.1"));
        assert!(!json.contains("4000"));
    }

    #[test]
    fn authority_endpoint_and_health_use_safe_labels() {
        let session = MyServerSession::default();
        let profiles = MyServerProfiles::default();
        let authority = AuthoritySession {
            role: Some(AuthorityRole::Client),
            endpoint: Some(AuthorityEndpoint::Remote {
                host: "10.0.0.4".to_owned(),
                port: 15000,
                transport: NetworkTransport::Tcp,
            }),
            authority_epoch: 7,
            frame_id: 42,
            ..Default::default()
        };
        let state = collect_network_preview_state(&MyServerNetworkPreviewAdapter {
            session: &session,
            profiles: Some(&profiles),
            authority: Some(&authority),
            entry: None,
            snapshot_bus: None,
            last_successful_receive_ms: None,
            last_error_category: None,
            authority_last_activity_age_ms: Some(2501),
            session_status: None,
        });
        assert_eq!(state.authority_endpoint_kind.as_deref(), Some("lan"));
        assert_eq!(state.authority_role.as_deref(), Some("client"));
        assert_eq!(state.authority_epoch, Some(7));
        assert_eq!(state.authority_frame, Some(42));
        assert_eq!(state.authority_sync_health.as_deref(), Some("stale"));
        let json = serde_json::to_string(&state).unwrap();
        assert!(!json.contains("10.0.0.4"));
        assert!(!json.contains("15000"));
    }

    #[test]
    fn error_mapping_is_stable_and_does_not_copy_error_text() {
        assert_eq!(
            classify_error_code("AUTH_TICKET_EXPIRED_secret"),
            "auth_rejected"
        );
        assert_eq!(classify_error_code("REQUEST_TIMEOUT"), "request_timeout");
        assert_eq!(classify_error_code("SERVER_REDIRECT"), "redirect_failed");
        assert_eq!(classify_error_code("CONCURRENT_LOGIN"), "session_kicked");
    }

    #[test]
    fn connection_and_reconnect_states_are_explicit() {
        let mut session = MyServerSession::default();
        session.account_login_state = AccountLoginState::LoggedIn;
        session.registration_state = RegistrationState::PendingReview;
        session.character_selection_state = CharacterSelectionState::Selecting;
        session.game_connection_state = GameConnectionState::Reconnecting;
        session.reconnect_after_auth = Some(ReconnectPlan {
            cause: ReconnectCause::TransportRecovery,
            correlation: None,
        });
        let state = collect_network_preview_state(&MyServerNetworkPreviewAdapter {
            session: &session,
            profiles: None,
            authority: None,
            entry: None,
            snapshot_bus: None,
            last_successful_receive_ms: None,
            last_error_category: None,
            authority_last_activity_age_ms: None,
            session_status: Some("session_kicked"),
        });
        assert_eq!(state.login_status.as_deref(), Some("logged_in"));
        assert_eq!(state.registration_status.as_deref(), Some("pending_review"));
        assert_eq!(
            state.character_selection_status.as_deref(),
            Some("selecting")
        );
        assert_eq!(state.connection_state.as_deref(), Some("reconnecting"));
        assert_eq!(state.reconnect_phase.as_deref(), Some("transport_recovery"));
        assert_eq!(state.session_status.as_deref(), Some("session_kicked"));
    }

    #[test]
    fn snapshot_bus_activity_age_drives_health() {
        let mut bus = MainWorldSnapshotBusState::default();
        bus.authority_activity_serial = 1;
        let session = MyServerSession::default();
        let state = collect_network_preview_state(&MyServerNetworkPreviewAdapter {
            session: &session,
            profiles: None,
            authority: None,
            entry: None,
            snapshot_bus: Some(&bus),
            last_successful_receive_ms: Some(12),
            last_error_category: Some("disconnected"),
            authority_last_activity_age_ms: Some(10),
            session_status: None,
        });
        assert_eq!(state.authority_sync_health.as_deref(), Some("healthy"));
        assert_eq!(state.last_error_category.as_deref(), Some("disconnected"));
    }
}
