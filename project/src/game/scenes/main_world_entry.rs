//! Generation-bound entry intent coordinator for the fixed public main world.
//!
//! This coordinator owns request generations and authority admission. Scene
//! loading and RoomReady remain separate later-stage responsibilities.

use bevy::{input::keyboard::Key, prelude::*};
use std::time::Duration;

#[cfg(all(debug_assertions, not(target_os = "android")))]
use crate::framework::ui::audit::UiAuditConfig;

use crate::{
    framework::{
        network::{ConnectionId, NetworkTransport},
        scene::prelude::{
            SceneCommand, SceneEnterRequest, SceneEvent, SceneExitRequest, SceneOwned,
            SceneRegistry, SceneSessionId,
        },
        ui::{
            core::{UiPanelCommand, binding::UiBindingValues},
            document::{UiDocumentPanel, UiDocumentRuntimeCommand, UiDocumentRuntimeRoot},
        },
    },
    game::{
        declarative_screen::DeclarativeScreenHostCommand,
        myserver::{
            AccountLoginState, CharacterSelectionState, GameConnectionState, MyServerCommand,
            MyServerEnvironment, MyServerErrorKind, MyServerEvent, MyServerProfiles,
            MyServerSession, MyServerUpdateSet,
        },
        navigation::{AppUiMode, GameRouteCommand},
        scenes::{
            FANGYUAN_HOME_SCENE_ID,
            main_world::MainWorldContentEvent,
            main_world_contract::{
                MAIN_WORLD_AUTHORITY_CONTRACT,
                main_world_bevy_position as contract_main_world_bevy_position,
                main_world_movement_snapshot_from_event,
            },
        },
        screens::gameplay::host::{MainWorldUiTeardownCause, request_main_world_ui_teardown},
    },
};

pub(in crate::game) struct MainWorldEntryPlugin;

/// Marks the point at which the entry coordinator has applied current-frame
/// authority events. Main-world movement consumes that settled lifecycle state
/// before it considers local input, prediction, or presentation work.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, SystemSet)]
pub(in crate::game) enum MainWorldEntryUpdateSet {
    Coordinator,
}

impl Plugin for MainWorldEntryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MainWorldEntryState>()
            .init_resource::<MainWorldEntryWatchdog>()
            .init_resource::<MainWorldEntryDebugConfig>()
            .add_message::<MainWorldEntryIntent>()
            .add_message::<MainWorldEntrySignal>()
            .add_message::<MainWorldEntryEvent>()
            .add_message::<MainWorldContentEvent>()
            .add_message::<DeclarativeScreenHostCommand>()
            .add_message::<UiPanelCommand>()
            .add_message::<UiDocumentRuntimeCommand>()
            .init_resource::<UiBindingValues>()
            .configure_sets(
                Update,
                MainWorldEntryUpdateSet::Coordinator
                    .after(MyServerUpdateSet::NetworkEvents)
                    .before(MyServerUpdateSet::CommandDispatch),
            )
            .add_systems(
                Update,
                (
                    abort_invalidated_entry,
                    trigger_debug_auto_enter,
                    adapt_main_world_return_input,
                    handle_entry_intents,
                    dispatch_main_world_join_requests,
                    consume_main_world_authority_events,
                    dispatch_authority_confirmed_scene_enter,
                    consume_main_world_scene_ready,
                    consume_main_world_content_events,
                    watchdog_main_world_entry_progress,
                    route_failed_authority_entry,
                    handle_entry_signals,
                    trigger_debug_auto_exit_after_recovery,
                )
                    .chain()
                    .in_set(MainWorldEntryUpdateSet::Coordinator)
                    .run_if(resource_exists::<MyServerProfiles>)
                    .run_if(resource_exists::<MyServerSession>),
            );

        #[cfg(all(debug_assertions, not(target_os = "android")))]
        app.add_systems(Startup, prepare_main_world_hud_audit_fixture);
    }
}

const ENV_MAIN_WORLD_AUTO_ENTER: &str = "MYBEVY_MAIN_WORLD_AUTO_ENTER";
const ENV_MAIN_WORLD_AUTO_EXIT: &str = "MYBEVY_MAIN_WORLD_AUTO_EXIT";
const ENV_MAIN_WORLD_AUTO_EXIT_AFTER_RECOVERY: &str = "MYBEVY_MAIN_WORLD_AUTO_EXIT_AFTER_RECOVERY";
const ENV_MAIN_WORLD_ACCEPTANCE_METRICS: &str = "MYBEVY_MAIN_WORLD_ACCEPTANCE_METRICS";
const MAIN_WORLD_ACCEPTANCE_SAMPLE_SECONDS: f64 = 5.0;
const MAIN_WORLD_ENTRY_PROGRESS_TIMEOUT: Duration = Duration::from_secs(20);
const MAIN_WORLD_EXIT_PROGRESS_TIMEOUT: Duration = Duration::from_secs(12);

#[cfg(all(debug_assertions, not(target_os = "android")))]
const MAIN_WORLD_HUD_AUDIT_FIXTURE_ID: &str = "stage16_main_world_hud";
#[cfg(all(debug_assertions, not(target_os = "android")))]
const MAIN_WORLD_MAIL_AUDIT_FIXTURE_ID: &str = "stage18_main_world_mail";

/// Prepares the fixed HUD route for a local deterministic visual capture. This
/// intentionally supplies no account, ticket, authority connection, or scene.
#[cfg(all(debug_assertions, not(target_os = "android")))]
fn prepare_main_world_hud_audit_fixture(
    audit: Option<Res<UiAuditConfig>>,
    mut state: ResMut<MainWorldEntryState>,
    next_mode: Option<ResMut<NextState<AppUiMode>>>,
) {
    let Some(audit) = audit else {
        return;
    };
    let Some(mut next_mode) = next_mode else {
        return;
    };
    apply_main_world_hud_audit_fixture(
        audit.targets_screen("main_world"),
        audit.stable_fixture_id(),
        &mut state,
        &mut next_mode,
    );
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
pub(in crate::game) fn apply_main_world_hud_audit_fixture(
    targets_main_world: bool,
    stable_fixture_id: Option<&str>,
    state: &mut MainWorldEntryState,
    next_mode: &mut NextState<AppUiMode>,
) {
    if targets_main_world
        && matches!(
            stable_fixture_id,
            Some(MAIN_WORLD_HUD_AUDIT_FIXTURE_ID | MAIN_WORLD_MAIL_AUDIT_FIXTURE_ID)
        )
    {
        activate_main_world_hud_audit_fixture(state);
        next_mode.set(AppUiMode::MainWorld);
    }
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
fn activate_main_world_hud_audit_fixture(state: &mut MainWorldEntryState) {
    *state = MainWorldEntryState {
        generation: 1,
        phase: MainWorldEntryPhase::Active,
        ..default()
    };
}

/// Explicit desktop Debug-only hook for exercising the authority entry flow
/// without relying on UI coordinate automation.
#[derive(Clone, Copy, Debug, Resource)]
struct MainWorldEntryDebugConfig {
    auto_enter: bool,
    auto_enter_sent: bool,
    auto_exit: bool,
    auto_exit_after_recovery: bool,
    auto_exit_sent: bool,
    acceptance_metrics: bool,
    metrics_elapsed_seconds: f64,
    metrics_frames: u64,
    metrics_reported: bool,
}

impl MainWorldEntryDebugConfig {
    fn from_env() -> Self {
        Self::from_env_reader(|key| std::env::var(key).ok())
    }

    fn from_env_reader(mut read: impl FnMut(&str) -> Option<String>) -> Self {
        Self {
            auto_enter: Self::enabled_from_env(&mut read, ENV_MAIN_WORLD_AUTO_ENTER),
            auto_enter_sent: false,
            auto_exit: Self::enabled_from_env(&mut read, ENV_MAIN_WORLD_AUTO_EXIT),
            auto_exit_after_recovery: Self::enabled_from_env(
                &mut read,
                ENV_MAIN_WORLD_AUTO_EXIT_AFTER_RECOVERY,
            ),
            auto_exit_sent: false,
            acceptance_metrics: Self::enabled_from_env(
                &mut read,
                ENV_MAIN_WORLD_ACCEPTANCE_METRICS,
            ),
            metrics_elapsed_seconds: 0.0,
            metrics_frames: 0,
            metrics_reported: false,
        }
    }

    fn enabled_from_env(read: &mut impl FnMut(&str) -> Option<String>, key: &str) -> bool {
        cfg!(debug_assertions)
            && read(key).is_some_and(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "on" | "yes" | "enabled"
                )
            })
    }
}

impl Default for MainWorldEntryDebugConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::game) enum MainWorldEntryPhase {
    #[default]
    LobbyIdle,
    Validating,
    JoiningRoom,
    LoadingScene,
    WaitingSceneReady,
    Active,
    Exiting,
    Recovering,
    HomeLoading,
    HomeActive,
    ReturningFromHome,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MainWorldEntryWatchdogKey {
    generation: u64,
    phase: MainWorldEntryPhase,
    join_attempt: Option<u64>,
    join_acknowledged: bool,
    has_authoritative_scene: bool,
    has_scene_session: bool,
    scene_ready: bool,
    scene_content_ready: bool,
    room_ready_requested: bool,
    reconnect_attempt: Option<u64>,
    reconnect_room_acknowledged: bool,
    recovery_snapshot_received: bool,
    exit_scene_settled: bool,
    exit_authority_settled: bool,
}

impl MainWorldEntryWatchdogKey {
    fn from_state(state: &MainWorldEntryState) -> Self {
        Self {
            generation: state.generation,
            phase: state.phase,
            join_attempt: state.join_attempt,
            join_acknowledged: state.join_acknowledged,
            has_authoritative_scene: state.authoritative_scene_id.is_some(),
            has_scene_session: state.scene_session_id.is_some(),
            scene_ready: state.scene_ready,
            scene_content_ready: state.scene_content_ready,
            room_ready_requested: state.room_ready_requested,
            reconnect_attempt: state.reconnect_attempt,
            reconnect_room_acknowledged: state.reconnect_room_acknowledged,
            recovery_snapshot_received: state.recovery_snapshot_received,
            exit_scene_settled: state.exit_scene_settled,
            exit_authority_settled: state.exit_authority_settled,
        }
    }
}

#[derive(Debug, Default, Resource)]
struct MainWorldEntryWatchdog {
    key: Option<MainWorldEntryWatchdogKey>,
    elapsed: Duration,
}

impl MainWorldEntryPhase {
    pub const fn blocks_lobby_input(self) -> bool {
        matches!(
            self,
            Self::Validating
                | Self::JoiningRoom
                | Self::LoadingScene
                | Self::WaitingSceneReady
                | Self::Exiting
                | Self::Recovering
                | Self::HomeLoading
                | Self::HomeActive
                | Self::ReturningFromHome
        )
    }
}

#[derive(Clone, Debug, Resource)]
pub(in crate::game) struct MainWorldEntryState {
    pub generation: u64,
    pub phase: MainWorldEntryPhase,
    pub environment: Option<MyServerEnvironment>,
    pub character_id: Option<String>,
    pub authority_transport: Option<NetworkTransport>,
    /// Identity boundary for every authority-side action in this entry run.
    /// A response must belong to this character and transport before it can
    /// advance the state machine, even when its correlation matches.
    pub authority_character_id: Option<String>,
    pub authority_connection_id: Option<ConnectionId>,
    pub room_id: Option<String>,
    pub policy_id: Option<String>,
    pub authoritative_scene_id: Option<i32>,
    pub position: Option<Vec3>,
    pub snapshot_generation: u32,
    pub scene_content_ready: bool,
    pub join_acknowledged: bool,
    pub authority_attempt: u64,
    pub join_attempt: Option<u64>,
    pub join_dispatched: bool,
    pub room_start_requested: bool,
    pub scene_session_id: Option<SceneSessionId>,
    pub scene_ready: bool,
    pub room_ready_requested: bool,
    pub room_ready_acknowledged: bool,
    pub ready_attempt: Option<u64>,
    pub input_frozen: bool,
    pub room_membership: MainWorldRoomMembership,
    pub last_departure: MainWorldRoomDeparture,
    pub exit_destination: Option<MainWorldExitDestination>,
    pub exit_scene_settled: bool,
    pub exit_authority_settled: bool,
    pub leave_attempt: Option<u64>,
    pub reconnect_requested: bool,
    pub reconnect_room_acknowledged: bool,
    pub reconnect_attempt: Option<u64>,
    pub recovery_snapshot_received: bool,
    pub home_session_id: Option<SceneSessionId>,
    pub failure: Option<MainWorldEntryFailure>,
    pub failure_routed: bool,
}

impl Default for MainWorldEntryState {
    fn default() -> Self {
        Self {
            generation: 0,
            phase: MainWorldEntryPhase::LobbyIdle,
            environment: None,
            character_id: None,
            authority_transport: None,
            authority_character_id: None,
            authority_connection_id: None,
            room_id: None,
            policy_id: None,
            authoritative_scene_id: None,
            position: None,
            snapshot_generation: 0,
            scene_content_ready: false,
            join_acknowledged: false,
            authority_attempt: 0,
            join_attempt: None,
            join_dispatched: false,
            room_start_requested: false,
            scene_session_id: None,
            scene_ready: false,
            room_ready_requested: false,
            room_ready_acknowledged: false,
            ready_attempt: None,
            input_frozen: true,
            room_membership: MainWorldRoomMembership::None,
            last_departure: MainWorldRoomDeparture::None,
            exit_destination: None,
            exit_scene_settled: false,
            exit_authority_settled: false,
            leave_attempt: None,
            reconnect_requested: false,
            reconnect_room_acknowledged: false,
            reconnect_attempt: None,
            recovery_snapshot_received: false,
            home_session_id: None,
            failure: None,
            failure_routed: false,
        }
    }
}

impl MainWorldEntryState {
    pub fn is_in_flight(&self) -> bool {
        self.phase.blocks_lobby_input()
    }

    pub fn accepts_generation(&self, generation: u64) -> bool {
        self.generation == generation && self.is_in_flight()
    }

    pub fn allows_gameplay_input(&self) -> bool {
        self.phase == MainWorldEntryPhase::Active && !self.input_frozen
    }

    fn owns_authority_session(&self) -> bool {
        matches!(
            self.phase,
            MainWorldEntryPhase::JoiningRoom
                | MainWorldEntryPhase::LoadingScene
                | MainWorldEntryPhase::WaitingSceneReady
                | MainWorldEntryPhase::Active
                | MainWorldEntryPhase::Exiting
                | MainWorldEntryPhase::Recovering
        )
    }

    fn accepts_scoped_authority_event(
        &self,
        session: &MyServerSession,
        connection_id: ConnectionId,
    ) -> bool {
        self.authority_character_id == self.character_id
            && self.authority_character_id.as_deref() == session.character_id.as_deref()
            && self.authority_connection_id == Some(connection_id)
            && session.connection_id == Some(connection_id)
    }

    fn begin(
        &mut self,
        environment: MyServerEnvironment,
        character_id: String,
        authority_transport: Option<NetworkTransport>,
        authority_connection_id: Option<ConnectionId>,
    ) {
        self.generation = self.generation.wrapping_add(1).max(1);
        self.phase = MainWorldEntryPhase::Validating;
        self.environment = Some(environment);
        self.character_id = Some(character_id);
        self.authority_transport = authority_transport;
        self.authority_character_id = self.character_id.clone();
        self.authority_connection_id = authority_connection_id;
        self.failure = None;
        self.failure_routed = false;
        self.room_id = None;
        self.policy_id = None;
        self.authoritative_scene_id = None;
        self.position = None;
        self.snapshot_generation = 0;
        self.scene_content_ready = false;
        self.join_acknowledged = false;
        self.join_attempt = None;
        self.join_dispatched = false;
        self.room_start_requested = false;
        self.scene_session_id = None;
        self.scene_ready = false;
        self.room_ready_requested = false;
        self.room_ready_acknowledged = false;
        self.ready_attempt = None;
        self.input_frozen = true;
        self.room_membership = MainWorldRoomMembership::None;
        self.last_departure = MainWorldRoomDeparture::None;
        self.exit_destination = None;
        self.exit_scene_settled = false;
        self.exit_authority_settled = false;
        self.leave_attempt = None;
        self.reconnect_requested = false;
        self.reconnect_room_acknowledged = false;
        self.reconnect_attempt = None;
        self.recovery_snapshot_received = false;
        self.home_session_id = None;
    }

    fn reset(&mut self) {
        self.phase = MainWorldEntryPhase::LobbyIdle;
        self.environment = None;
        self.character_id = None;
        self.authority_transport = None;
        self.authority_character_id = None;
        self.authority_connection_id = None;
        self.failure = None;
        self.failure_routed = false;
        self.room_id = None;
        self.policy_id = None;
        self.authoritative_scene_id = None;
        self.position = None;
        self.snapshot_generation = 0;
        self.scene_content_ready = false;
        self.join_acknowledged = false;
        self.join_attempt = None;
        self.join_dispatched = false;
        self.room_start_requested = false;
        self.scene_session_id = None;
        self.scene_ready = false;
        self.room_ready_requested = false;
        self.room_ready_acknowledged = false;
        self.ready_attempt = None;
        self.input_frozen = true;
        self.room_membership = MainWorldRoomMembership::None;
        self.exit_destination = None;
        self.exit_scene_settled = false;
        self.exit_authority_settled = false;
        self.leave_attempt = None;
        self.reconnect_requested = false;
        self.reconnect_room_acknowledged = false;
        self.reconnect_attempt = None;
        self.recovery_snapshot_received = false;
        self.home_session_id = None;
    }

    fn fail(&mut self, failure: MainWorldEntryFailure) {
        self.phase = MainWorldEntryPhase::Failed;
        self.input_frozen = true;
        self.environment = None;
        self.character_id = None;
        self.authority_transport = None;
        self.failure = Some(failure);
        self.failure_routed = false;
    }

    fn begin_authority_attempt(&mut self) -> u64 {
        self.authority_attempt = self.authority_attempt.wrapping_add(1).max(1);
        self.authority_attempt
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::game) enum MainWorldRoomMembership {
    #[default]
    None,
    Joined,
    Unknown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::game) enum MainWorldRoomDeparture {
    #[default]
    None,
    Confirmed,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum MainWorldExitDestination {
    Lobby,
    Login,
    Home,
}

#[derive(Clone, Debug, Message, PartialEq, Eq)]
pub(in crate::game) enum MainWorldEntryIntent {
    Enter,
    Cancel,
    ExitToLobby,
    Recover,
    EnvironmentChanged,
    CharacterChanged,
    LoggedOut,
    ApplicationExit,
    EnterHome,
    ReturnFromHome,
}

#[derive(Clone, Debug, Message, PartialEq, Eq)]
pub(in crate::game) enum MainWorldEntrySignal {
    /// Reserved for the stage 5 JoinRoom response adapter.
    JoinAccepted { generation: u64 },
    /// Reserved for the stage 6 SceneEvent::Ready adapter.
    SceneReady { generation: u64 },
}

#[derive(Clone, Debug, Message, PartialEq, Eq)]
pub(in crate::game) enum MainWorldEntryEvent {
    JoinRequested {
        generation: u64,
        attempt: u64,
        connection_id: ConnectionId,
        room_id: &'static str,
        policy_id: &'static str,
        character_id: String,
    },
    Aborted {
        generation: u64,
        reason: MainWorldEntryAbortReason,
    },
    Failed {
        generation: u64,
        failure: MainWorldEntryFailure,
    },
    IgnoredStaleSignal {
        generation: u64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum MainWorldEntryAbortReason {
    Cancelled,
    ExitToLobby,
    EnvironmentChanged,
    CharacterChanged,
    LoggedOut,
    ApplicationExit,
    PreconditionsInvalidated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum MainWorldEntryFailure {
    EnvironmentUnavailable,
    AccountSessionUnavailable,
    CharacterUnavailable,
    TicketUnavailable,
    GameAuthUnavailable,
    SceneMappingUnavailable,
    RoomFull,
    RoomPolicyRejected,
    RoomUnavailable,
    JoinRejected,
    AuthoritativeSceneMismatch,
    InvalidAuthoritativePosition,
    JoinTimedOut,
    ReadyTimedOut,
    SceneLoadFailed,
    ReconnectUnavailable,
}

fn trigger_debug_auto_enter(
    profiles: Res<MyServerProfiles>,
    session: Res<MyServerSession>,
    registry: Res<SceneRegistry>,
    state: Res<MainWorldEntryState>,
    mut debug: ResMut<MainWorldEntryDebugConfig>,
    mut intents: MessageWriter<MainWorldEntryIntent>,
) {
    if !debug.auto_enter
        || debug.auto_enter_sent
        || state.phase != MainWorldEntryPhase::LobbyIdle
        || validate_entry(&profiles, &session, &registry).is_err()
    {
        return;
    }

    debug.auto_enter_sent = true;
    info!("main world debug auto-enter requested after authority authentication");
    intents.write(MainWorldEntryIntent::Enter);
}

fn trigger_debug_auto_exit_after_recovery(
    time: Res<Time<Real>>,
    state: Res<MainWorldEntryState>,
    entities: Query<(Entity, Option<&SceneOwned>)>,
    mut debug_config: ResMut<MainWorldEntryDebugConfig>,
    mut intents: MessageWriter<MainWorldEntryIntent>,
) {
    let recovered_active = state.reconnect_requested
        && state.reconnect_room_acknowledged
        && state.room_ready_acknowledged;
    let exit_enabled =
        debug_config.auto_exit || (debug_config.auto_exit_after_recovery && recovered_active);
    if !exit_enabled || debug_config.auto_exit_sent || state.phase != MainWorldEntryPhase::Active {
        return;
    }

    if debug_config.acceptance_metrics && !debug_config.metrics_reported {
        debug_config.metrics_frames += 1;
        debug_config.metrics_elapsed_seconds += time.delta_secs_f64();
        if debug_config.metrics_elapsed_seconds < MAIN_WORLD_ACCEPTANCE_SAMPLE_SECONDS {
            return;
        }

        let session_id = state.scene_session_id.as_ref();
        let mut ecs_entity_count = 0usize;
        let mut scene_owned_entity_count = 0usize;
        for (_, owned) in &entities {
            ecs_entity_count += 1;
            if session_id.is_some_and(|session_id| {
                owned.is_some_and(|owned| owned.session_id == *session_id)
            }) {
                scene_owned_entity_count += 1;
            }
        }
        let sample_seconds = debug_config.metrics_elapsed_seconds;
        let application_update_fps = debug_config.metrics_frames as f64 / sample_seconds;
        info!(
            sample_seconds,
            application_update_fps,
            ecs_entity_count,
            scene_owned_entity_count,
            "main world debug acceptance metrics (application updates, not GPU presents)"
        );
        debug_config.metrics_reported = true;
    }

    debug_config.auto_exit_sent = true;
    info!(
        recovered = recovered_active,
        "main world debug automatic lobby exit requested"
    );
    intents.write(MainWorldEntryIntent::ExitToLobby);
}

fn adapt_main_world_return_input(
    key_codes: Option<Res<ButtonInput<KeyCode>>>,
    keys: Option<Res<ButtonInput<Key>>>,
    state: Res<MainWorldEntryState>,
    document_roots: Query<&UiDocumentRuntimeRoot>,
    mut intents: MessageWriter<MainWorldEntryIntent>,
) {
    if state.phase != MainWorldEntryPhase::Active {
        return;
    }

    if document_roots.iter().any(|root| {
        matches!(
            root.panel,
            UiDocumentPanel::Floating | UiDocumentPanel::Modal | UiDocumentPanel::BlockingOverlay
        )
    }) {
        return;
    }

    let return_requested = key_codes
        .as_deref()
        .is_some_and(|input| input.just_pressed(KeyCode::Escape))
        || keys
            .as_deref()
            .is_some_and(|input| input.just_pressed(Key::BrowserBack));
    if return_requested {
        intents.write(MainWorldEntryIntent::ExitToLobby);
    }
}

fn handle_entry_intents(
    mut intents: MessageReader<MainWorldEntryIntent>,
    profiles: Res<MyServerProfiles>,
    session: Res<MyServerSession>,
    registry: Res<SceneRegistry>,
    mut state: ResMut<MainWorldEntryState>,
    mut events: MessageWriter<MainWorldEntryEvent>,
    mut myserver_commands: MessageWriter<MyServerCommand>,
    mut scene_commands: MessageWriter<SceneCommand>,
    mut route_commands: MessageWriter<GameRouteCommand>,
    mut bindings: ResMut<UiBindingValues>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
    mut runtime_commands: MessageWriter<UiDocumentRuntimeCommand>,
    mut screen_commands: MessageWriter<DeclarativeScreenHostCommand>,
) {
    let mut accepted_enter = false;
    for intent in intents.read() {
        if matches!(intent, MainWorldEntryIntent::Enter) {
            if accepted_enter
                || !matches!(
                    state.phase,
                    MainWorldEntryPhase::LobbyIdle | MainWorldEntryPhase::Failed
                )
            {
                continue;
            }
            accepted_enter = true;
            state.begin(
                profiles.selected(),
                session.character_id.clone().unwrap_or_default(),
                session.transport,
                session.connection_id,
            );
            if let Err(failure) = validate_entry(&profiles, &session, &registry) {
                let generation = state.generation;
                state.fail(failure);
                events.write(MainWorldEntryEvent::Failed {
                    generation,
                    failure,
                });
                continue;
            }
            state.phase = MainWorldEntryPhase::JoiningRoom;
            let attempt = state.begin_authority_attempt();
            state.join_attempt = Some(attempt);
            warn!(
                generation = state.generation,
                character_id = %state.character_id.as_deref().unwrap_or_default(),
                "main world entry accepted; joining room"
            );
            events.write(MainWorldEntryEvent::JoinRequested {
                generation: state.generation,
                attempt,
                connection_id: session
                    .connection_id
                    .expect("validated game transport must have a connection id"),
                room_id: MAIN_WORLD_AUTHORITY_CONTRACT.room_id,
                policy_id: MAIN_WORLD_AUTHORITY_CONTRACT.policy_id,
                character_id: state.character_id.clone().unwrap_or_default(),
            });
            continue;
        }
        match intent {
            MainWorldEntryIntent::EnterHome if state.owns_authority_session() => {
                if state.phase != MainWorldEntryPhase::Exiting {
                    request_main_world_ui_teardown(
                        MainWorldUiTeardownCause::SwitchToHome,
                        &mut bindings,
                        &mut panel_commands,
                        &mut runtime_commands,
                        &mut screen_commands,
                    );
                }
                begin_exit(
                    &mut state,
                    MainWorldExitDestination::Home,
                    &session,
                    &mut myserver_commands,
                    &mut scene_commands,
                    &mut route_commands,
                );
            }
            MainWorldEntryIntent::EnterHome
                if matches!(
                    state.phase,
                    MainWorldEntryPhase::LobbyIdle | MainWorldEntryPhase::Failed
                ) =>
            {
                begin_home_enter(&mut state, &mut scene_commands);
            }
            MainWorldEntryIntent::ReturnFromHome
                if matches!(
                    state.phase,
                    MainWorldEntryPhase::HomeLoading | MainWorldEntryPhase::HomeActive
                ) =>
            {
                begin_home_return(&mut state, &mut scene_commands);
            }
            MainWorldEntryIntent::ExitToLobby | MainWorldEntryIntent::Cancel
                if state.owns_authority_session() =>
            {
                if state.phase != MainWorldEntryPhase::Exiting {
                    request_main_world_ui_teardown(
                        MainWorldUiTeardownCause::LeaveToLobby,
                        &mut bindings,
                        &mut panel_commands,
                        &mut runtime_commands,
                        &mut screen_commands,
                    );
                }
                begin_exit(
                    &mut state,
                    MainWorldExitDestination::Lobby,
                    &session,
                    &mut myserver_commands,
                    &mut scene_commands,
                    &mut route_commands,
                );
            }
            MainWorldEntryIntent::EnvironmentChanged | MainWorldEntryIntent::CharacterChanged
                if state.owns_authority_session() =>
            {
                begin_exit(
                    &mut state,
                    MainWorldExitDestination::Lobby,
                    &session,
                    &mut myserver_commands,
                    &mut scene_commands,
                    &mut route_commands,
                );
            }
            MainWorldEntryIntent::LoggedOut | MainWorldEntryIntent::ApplicationExit
                if state.owns_authority_session() =>
            {
                begin_exit(
                    &mut state,
                    MainWorldExitDestination::Login,
                    &session,
                    &mut myserver_commands,
                    &mut scene_commands,
                    &mut route_commands,
                );
            }
            _ => abort_for_intent(intent, &mut state, &mut events),
        }
    }
}

fn abort_invalidated_entry(
    profiles: Res<MyServerProfiles>,
    session: Res<MyServerSession>,
    mut state: ResMut<MainWorldEntryState>,
    mut events: MessageWriter<MainWorldEntryEvent>,
    mut myserver_commands: MessageWriter<MyServerCommand>,
    mut scene_commands: MessageWriter<SceneCommand>,
    mut route_commands: MessageWriter<GameRouteCommand>,
) {
    if matches!(
        state.phase,
        MainWorldEntryPhase::Validating
            | MainWorldEntryPhase::JoiningRoom
            | MainWorldEntryPhase::LoadingScene
            | MainWorldEntryPhase::WaitingSceneReady
            | MainWorldEntryPhase::Recovering
    ) && (state.environment != Some(profiles.selected())
        || state.character_id.as_deref() != session.character_id.as_deref()
        || !matches!(session.account_login_state, AccountLoginState::LoggedIn))
    {
        abort_invalidated_entry_with_cleanup(
            &mut state,
            &session,
            &mut events,
            &mut myserver_commands,
            &mut scene_commands,
            &mut route_commands,
        );
    }
}

fn dispatch_main_world_join_requests(
    mut entry_events: MessageReader<MainWorldEntryEvent>,
    session: Res<MyServerSession>,
    mut state: ResMut<MainWorldEntryState>,
    mut commands: MessageWriter<MyServerCommand>,
) {
    for event in entry_events.read() {
        let MainWorldEntryEvent::JoinRequested {
            generation,
            attempt,
            connection_id,
            room_id,
            policy_id,
            ..
        } = event
        else {
            continue;
        };
        if state.phase != MainWorldEntryPhase::JoiningRoom
            || state.generation != *generation
            || state.join_acknowledged
            || state.join_attempt != Some(*attempt)
            || !state.accepts_scoped_authority_event(&session, *connection_id)
        {
            continue;
        }
        commands.write(MyServerCommand::JoinRoomScoped {
            room_id: (*room_id).to_owned(),
            policy_id: (*policy_id).to_owned(),
            correlation: *attempt,
        });
        state.join_dispatched = true;
    }
}

fn consume_main_world_authority_events(
    mut myserver_events: MessageReader<MyServerEvent>,
    session: Res<MyServerSession>,
    mut state: ResMut<MainWorldEntryState>,
    mut entry_events: MessageWriter<MainWorldEntryEvent>,
    mut commands: MessageWriter<MyServerCommand>,
    mut scene_commands: MessageWriter<SceneCommand>,
    mut route_commands: MessageWriter<GameRouteCommand>,
) {
    if !state.owns_authority_session() {
        return;
    }
    let character_id = state.character_id.clone().unwrap_or_default();
    for event in myserver_events.read() {
        if state.phase == MainWorldEntryPhase::Exiting {
            match event {
                MyServerEvent::RoomJoinedScoped {
                    correlation,
                    connection_id,
                    response,
                    ..
                } if state.join_attempt == Some(*correlation)
                    && state.accepts_scoped_authority_event(&session, *connection_id)
                    && response.room_id == MAIN_WORLD_AUTHORITY_CONTRACT.room_id =>
                {
                    state.join_attempt = None;
                    state.join_dispatched = false;
                    if response.ok {
                        commands.write(MyServerCommand::AdoptRoomMembership {
                            room_id: response.room_id.clone(),
                        });
                        state.room_id = Some(response.room_id.clone());
                        state.room_membership = MainWorldRoomMembership::Joined;
                        request_main_world_leave_for_exit(&mut state, &session, &mut commands);
                    } else {
                        state.room_membership = MainWorldRoomMembership::None;
                        state.exit_authority_settled = true;
                    }
                    complete_exit_if_settled(&mut state, &mut scene_commands, &mut route_commands);
                }
                MyServerEvent::ScopedRequestFailed {
                    message_type: crate::game::myserver::protocol::MessageType::RoomJoinReq,
                    correlation,
                    connection_id: Some(connection_id),
                    ..
                } if state.join_attempt == Some(*correlation)
                    && state.accepts_scoped_authority_event(&session, *connection_id) =>
                {
                    state.join_attempt = None;
                    state.join_dispatched = false;
                    state.room_membership = MainWorldRoomMembership::None;
                    state.exit_authority_settled = true;
                    complete_exit_if_settled(&mut state, &mut scene_commands, &mut route_commands);
                }
                MyServerEvent::RoomLeftScoped {
                    correlation,
                    connection_id,
                    response,
                    ..
                } if state.leave_attempt == Some(*correlation)
                    && state.accepts_scoped_authority_event(&session, *connection_id)
                    && response.room_id == MAIN_WORLD_AUTHORITY_CONTRACT.room_id =>
                {
                    state.leave_attempt = None;
                    state.last_departure = if response.ok {
                        MainWorldRoomDeparture::Confirmed
                    } else {
                        MainWorldRoomDeparture::Unknown
                    };
                    state.room_membership = if response.ok {
                        commands.write(MyServerCommand::ClearRoomMembership {
                            room_id: response.room_id.clone(),
                        });
                        MainWorldRoomMembership::None
                    } else {
                        MainWorldRoomMembership::Unknown
                    };
                    state.exit_authority_settled = true;
                    complete_exit_if_settled(&mut state, &mut scene_commands, &mut route_commands);
                }
                MyServerEvent::ScopedRequestFailed {
                    message_type:
                        crate::game::myserver::protocol::MessageType::RoomLeaveReq
                        | crate::game::myserver::protocol::MessageType::RoomLeaveRes,
                    correlation,
                    connection_id: Some(connection_id),
                    ..
                } if state.leave_attempt == Some(*correlation)
                    && state.accepts_scoped_authority_event(&session, *connection_id) =>
                {
                    state.leave_attempt = None;
                    state.last_departure = MainWorldRoomDeparture::Unknown;
                    state.room_membership = MainWorldRoomMembership::Unknown;
                    state.exit_authority_settled = true;
                    complete_exit_if_settled(&mut state, &mut scene_commands, &mut route_commands);
                }
                MyServerEvent::Disconnected { .. } | MyServerEvent::ConnectionFailed { .. } => {
                    state.join_attempt = None;
                    state.reconnect_attempt = None;
                    state.leave_attempt = None;
                    if state.room_membership != MainWorldRoomMembership::None {
                        state.last_departure = MainWorldRoomDeparture::Unknown;
                        state.room_membership = MainWorldRoomMembership::Unknown;
                    }
                    state.exit_authority_settled = true;
                    complete_exit_if_settled(&mut state, &mut scene_commands, &mut route_commands);
                }
                MyServerEvent::RequestFailed { correlation, .. }
                    if correlation.is_some_and(|value| state.leave_attempt == Some(value)) =>
                {
                    state.leave_attempt = None;
                    state.last_departure = MainWorldRoomDeparture::Unknown;
                    state.room_membership = MainWorldRoomMembership::Unknown;
                    state.exit_authority_settled = true;
                    complete_exit_if_settled(&mut state, &mut scene_commands, &mut route_commands);
                }
                _ => {}
            }
            continue;
        }
        if !matches!(event, MyServerEvent::RoomStatePush(_))
            && let Some(push) = main_world_movement_snapshot_from_event(event)
        {
            consume_main_world_entry_snapshot(
                &push,
                &character_id,
                &mut state,
                &session,
                &mut entry_events,
                &mut commands,
                &mut scene_commands,
            );
            continue;
        }
        match event {
            MyServerEvent::Connected { connection_id, .. }
                if state.phase == MainWorldEntryPhase::Recovering
                    && state.authority_character_id.as_deref()
                        == session.character_id.as_deref()
                    && session.connection_id == Some(*connection_id) =>
            {
                state.authority_connection_id = Some(*connection_id);
            }
            MyServerEvent::RoomJoinedScoped {
                correlation,
                connection_id,
                response,
                ..
            } if state.phase == MainWorldEntryPhase::JoiningRoom
                && state.join_attempt.is_some()
                && Some(*correlation) == state.join_attempt
                && state.accepts_scoped_authority_event(&session, *connection_id)
                && response.room_id == MAIN_WORLD_AUTHORITY_CONTRACT.room_id =>
            {
                if response.ok {
                    warn!(
                        generation = state.generation,
                        character_id = %character_id,
                        "main world room join acknowledged"
                    );
                    state.join_acknowledged = true;
                    state.join_attempt = None;
                    state.join_dispatched = false;
                    state.room_membership = MainWorldRoomMembership::Joined;
                    commands.write(MyServerCommand::AdoptRoomMembership {
                        room_id: response.room_id.clone(),
                    });
                    state.room_id = Some(response.room_id.clone());
                    state.policy_id = Some(MAIN_WORLD_AUTHORITY_CONTRACT.policy_id.to_owned());
                } else {
                    fail_authority_entry(
                        &mut state,
                        &session,
                        &mut entry_events,
                        &mut commands,
                        &mut scene_commands,
                        room_join_failure(&response.error_code),
                    );
                }
            }
            MyServerEvent::RoomStatePush(push) => {
                if (state.phase == MainWorldEntryPhase::JoiningRoom && !state.join_acknowledged)
                    || (state.phase == MainWorldEntryPhase::Recovering
                        && (!state.join_acknowledged || !state.reconnect_room_acknowledged))
                {
                    continue;
                }
                let Some(snapshot) = push.snapshot.as_ref() else {
                    continue;
                };
                if snapshot.room_id != MAIN_WORLD_AUTHORITY_CONTRACT.room_id {
                    continue;
                }
                if MAIN_WORLD_AUTHORITY_CONTRACT
                    .validate_room_game_state(&snapshot.game_state)
                    .is_err()
                {
                    fail_authority_entry(
                        &mut state,
                        &session,
                        &mut entry_events,
                        &mut commands,
                        &mut scene_commands,
                        MainWorldEntryFailure::AuthoritativeSceneMismatch,
                    );
                } else {
                    state.room_membership = MainWorldRoomMembership::Joined;
                    state.room_id = Some(snapshot.room_id.clone());
                    state.policy_id = Some(MAIN_WORLD_AUTHORITY_CONTRACT.policy_id.to_owned());
                    if snapshot.state == "in_game" {
                        if let Some(movement) = main_world_movement_snapshot_from_event(event) {
                            consume_main_world_entry_snapshot(
                                &movement,
                                &character_id,
                                &mut state,
                                &session,
                                &mut entry_events,
                                &mut commands,
                                &mut scene_commands,
                            );
                        }
                    } else if !state.room_start_requested {
                        state.room_start_requested = true;
                        info!(
                            room_id = MAIN_WORLD_AUTHORITY_CONTRACT.room_id,
                            room_state = %snapshot.state,
                            "main world public room start requested"
                        );
                        commands.write(MyServerCommand::StartRoom);
                    }
                }
            }
            MyServerEvent::ReadyChangedScoped {
                correlation,
                connection_id,
                response,
                ..
            } if response.room_id == MAIN_WORLD_AUTHORITY_CONTRACT.room_id
                && Some(*correlation) == state.ready_attempt
                && state.accepts_scoped_authority_event(&session, *connection_id)
                && response.ok
                && response.ready
                && state.phase == MainWorldEntryPhase::WaitingSceneReady
                && state.scene_ready
                && state.scene_content_ready
                && state.room_ready_requested
                && state.ready_attempt.is_some() =>
            {
                state.room_ready_acknowledged = true;
                state.ready_attempt = None;
                info!(
                    room_id = MAIN_WORLD_AUTHORITY_CONTRACT.room_id,
                    "main world room ready acknowledged"
                );
                activate_when_ready(&mut state, &mut route_commands);
            }
            MyServerEvent::ReadyChangedScoped {
                correlation,
                connection_id,
                response,
                ..
            } if response.room_id == MAIN_WORLD_AUTHORITY_CONTRACT.room_id
                && Some(*correlation) == state.ready_attempt
                && state.accepts_scoped_authority_event(&session, *connection_id)
                && !response.ok
                && state.phase == MainWorldEntryPhase::WaitingSceneReady
                && state.room_ready_requested
                && state.ready_attempt.is_some() =>
            {
                fail_authority_entry(
                    &mut state,
                    &session,
                    &mut entry_events,
                    &mut commands,
                    &mut scene_commands,
                    room_ready_failure(&response.error_code),
                );
            }
            MyServerEvent::ScopedRequestFailed {
                message_type:
                    crate::game::myserver::protocol::MessageType::RoomReadyReq
                    | crate::game::myserver::protocol::MessageType::RoomReadyRes,
                error,
                correlation,
                connection_id: Some(connection_id),
                ..
            } if state.phase == MainWorldEntryPhase::WaitingSceneReady
                && state.room_ready_requested
                && state.ready_attempt.is_some()
                && Some(*correlation) == state.ready_attempt
                && state.accepts_scoped_authority_event(&session, *connection_id) =>
            {
                let failure = if error.to_ascii_lowercase().contains("timeout") {
                    MainWorldEntryFailure::ReadyTimedOut
                } else {
                    MainWorldEntryFailure::JoinRejected
                };
                fail_authority_entry(
                    &mut state,
                    &session,
                    &mut entry_events,
                    &mut commands,
                    &mut scene_commands,
                    failure,
                );
            }
            MyServerEvent::ScopedRequestFailed {
                message_type:
                    crate::game::myserver::protocol::MessageType::RoomJoinReq
                    | crate::game::myserver::protocol::MessageType::RoomJoinRes,
                error,
                correlation,
                connection_id: Some(connection_id),
                ..
            } if state.phase == MainWorldEntryPhase::JoiningRoom
                && state.join_attempt.is_some()
                && Some(*correlation) == state.join_attempt
                && state.accepts_scoped_authority_event(&session, *connection_id) =>
            {
                state.join_dispatched = false;
                fail_authority_entry(
                    &mut state,
                    &session,
                    &mut entry_events,
                    &mut commands,
                    &mut scene_commands,
                    join_request_failure(error),
                );
            }
            MyServerEvent::ScopedRequestFailed {
                message_type:
                    crate::game::myserver::protocol::MessageType::RoomReconnectReq
                    | crate::game::myserver::protocol::MessageType::RoomReconnectRes,
                correlation,
                connection_id: Some(connection_id),
                ..
            } if state.phase == MainWorldEntryPhase::Recovering
                && state.reconnect_attempt.is_some()
                && Some(*correlation) == state.reconnect_attempt
                && state.accepts_scoped_authority_event(&session, *connection_id) =>
            {
                fail_authority_entry(
                    &mut state,
                    &session,
                    &mut entry_events,
                    &mut commands,
                    &mut scene_commands,
                    MainWorldEntryFailure::ReconnectUnavailable,
                );
            }
            MyServerEvent::Disconnected { .. } | MyServerEvent::ConnectionFailed { .. }
                if matches!(
                    state.phase,
                    MainWorldEntryPhase::Active
                        | MainWorldEntryPhase::JoiningRoom
                        | MainWorldEntryPhase::WaitingSceneReady
                        | MainWorldEntryPhase::LoadingScene
                ) =>
            {
                begin_recovery(
                    &mut state,
                    &session,
                    &mut entry_events,
                    &mut commands,
                    &mut scene_commands,
                    &mut route_commands,
                );
            }
            MyServerEvent::Disconnected { .. } | MyServerEvent::ConnectionFailed { .. }
                if state.phase == MainWorldEntryPhase::Recovering =>
            {
                fail_authority_entry(
                    &mut state,
                    &session,
                    &mut entry_events,
                    &mut commands,
                    &mut scene_commands,
                    MainWorldEntryFailure::ReconnectUnavailable,
                );
            }
            MyServerEvent::ServerRedirectReconnectStarted {
                transport,
                correlation,
                ..
            } => {
                state.phase = MainWorldEntryPhase::Recovering;
                state.input_frozen = true;
                state.room_membership = MainWorldRoomMembership::Unknown;
                state.authoritative_scene_id = None;
                state.position = None;
                state.snapshot_generation = 0;
                state.room_ready_requested = false;
                state.room_ready_acknowledged = false;
                state.reconnect_requested = true;
                state.reconnect_room_acknowledged = false;
                state.authority_transport = Some(*transport);
                state.authority_connection_id = None;
                state.reconnect_attempt = Some(*correlation);
                state.recovery_snapshot_received = false;
            }
            MyServerEvent::RoomReconnectedScoped {
                correlation,
                connection_id,
                response,
                ..
            } if state.phase == MainWorldEntryPhase::Recovering
                && state.reconnect_attempt.is_some()
                && Some(*correlation) == state.reconnect_attempt
                && state.accepts_scoped_authority_event(&session, *connection_id)
                && response.room_id == MAIN_WORLD_AUTHORITY_CONTRACT.room_id =>
            {
                if response.ok {
                    commands.write(MyServerCommand::AdoptRoomMembership {
                        room_id: response.room_id.clone(),
                    });
                    state.room_id = Some(response.room_id.clone());
                    state.policy_id = Some(MAIN_WORLD_AUTHORITY_CONTRACT.policy_id.to_owned());
                    state.room_membership = MainWorldRoomMembership::Joined;
                    state.join_acknowledged = true;
                    state.reconnect_room_acknowledged = true;
                    state.reconnect_attempt = None;
                    info!("main world recovery room reconnect accepted");
                    if let Some(recovery) = response.movement_recovery.as_ref() {
                        consume_main_world_entry_snapshot(
                            &crate::game::myserver::protocol::pb::MovementSnapshotPush {
                                room_id: response.room_id.clone(),
                                frame_id: recovery.frame_id,
                                entities: recovery.entities.clone(),
                                full_sync: true,
                                reason: "room_reconnect".to_owned(),
                                correction_kind: recovery.correction_kind,
                                reason_code: recovery.reason_code,
                                target_character_ids: Vec::new(),
                                reference_frame_id: recovery.reference_frame_id,
                            },
                            &character_id,
                            &mut state,
                            &session,
                            &mut entry_events,
                            &mut commands,
                            &mut scene_commands,
                        );
                    }
                } else {
                    commands.write(MyServerCommand::ClearRoomMembership {
                        room_id: response.room_id.clone(),
                    });
                    state.room_membership = MainWorldRoomMembership::None;
                    state.join_acknowledged = false;
                    state.reconnect_room_acknowledged = false;
                    state.recovery_snapshot_received = false;
                    state.snapshot_generation = 0;
                    state.authoritative_scene_id = None;
                    state.position = None;
                    state.room_start_requested = false;
                    state.room_ready_requested = false;
                    state.room_ready_acknowledged = false;
                    state.ready_attempt = None;
                    state.phase = MainWorldEntryPhase::JoiningRoom;
                    state.room_id = None;
                    state.join_attempt = Some(state.begin_authority_attempt());
                    state.join_dispatched = true;
                    commands.write(MyServerCommand::JoinRoomScoped {
                        room_id: MAIN_WORLD_AUTHORITY_CONTRACT.room_id.to_owned(),
                        policy_id: MAIN_WORLD_AUTHORITY_CONTRACT.policy_id.to_owned(),
                        correlation: state.join_attempt.unwrap_or_default(),
                    });
                }
            }
            MyServerEvent::SessionKicked { .. }
            | MyServerEvent::AccountBanned { .. }
            | MyServerEvent::VersionIncompatible { .. }
            | MyServerEvent::GameAuthRejected { .. }
            | MyServerEvent::DisplayError {
                error:
                    crate::game::myserver::MyServerDisplayError {
                        kind:
                            MyServerErrorKind::Maintenance
                            | MyServerErrorKind::VersionIncompatible
                            | MyServerErrorKind::Unauthorized
                            | MyServerErrorKind::SessionKicked
                            | MyServerErrorKind::AccountBanned
                            | MyServerErrorKind::AccountBlocked,
                        ..
                    },
            } => begin_exit(
                &mut state,
                MainWorldExitDestination::Login,
                &session,
                &mut commands,
                &mut scene_commands,
                &mut route_commands,
            ),
            _ => {}
        }
    }
}

fn consume_main_world_entry_snapshot(
    push: &crate::game::myserver::protocol::pb::MovementSnapshotPush,
    character_id: &str,
    state: &mut MainWorldEntryState,
    session: &MyServerSession,
    entry_events: &mut MessageWriter<MainWorldEntryEvent>,
    commands: &mut MessageWriter<MyServerCommand>,
    scene_commands: &mut MessageWriter<SceneCommand>,
) {
    if push.room_id != MAIN_WORLD_AUTHORITY_CONTRACT.room_id
        || (state.phase == MainWorldEntryPhase::JoiningRoom && !state.join_acknowledged)
        || (state.phase == MainWorldEntryPhase::Recovering
            && (!state.join_acknowledged || !state.reconnect_room_acknowledged))
        || push.frame_id < state.snapshot_generation
    {
        return;
    }
    let Some(entity) = push
        .entities
        .iter()
        .find(|entity| entity.character_id == character_id)
    else {
        return;
    };
    if !MAIN_WORLD_AUTHORITY_CONTRACT.is_authoritative_entity_scene(entity.scene_id) {
        fail_authority_entry(
            state,
            session,
            entry_events,
            commands,
            scene_commands,
            MainWorldEntryFailure::AuthoritativeSceneMismatch,
        );
        return;
    }
    let Ok(position) = main_world_bevy_position(entity.x, entity.y) else {
        fail_authority_entry(
            state,
            session,
            entry_events,
            commands,
            scene_commands,
            MainWorldEntryFailure::InvalidAuthoritativePosition,
        );
        return;
    };
    state.room_id = Some(push.room_id.clone());
    state.room_membership = MainWorldRoomMembership::Joined;
    state.policy_id = Some(MAIN_WORLD_AUTHORITY_CONTRACT.policy_id.to_owned());
    state.authoritative_scene_id = Some(entity.scene_id);
    state.position = Some(position);
    state.snapshot_generation = push.frame_id;
    if state.phase == MainWorldEntryPhase::Recovering {
        state.recovery_snapshot_received = true;
    }
    match state.phase {
        MainWorldEntryPhase::JoiningRoom => {
            info!(
                room_id = MAIN_WORLD_AUTHORITY_CONTRACT.room_id,
                scene_id = entity.scene_id,
                frame_id = push.frame_id,
                "main world authoritative snapshot accepted"
            );
            begin_ready_or_scene_load_after_snapshot(state, commands);
        }
        MainWorldEntryPhase::Recovering => resume_after_reconnect_snapshot(state, commands),
        _ => {}
    }
}

fn fail_authority_entry(
    state: &mut MainWorldEntryState,
    session: &MyServerSession,
    events: &mut MessageWriter<MainWorldEntryEvent>,
    commands: &mut MessageWriter<MyServerCommand>,
    scene_commands: &mut MessageWriter<SceneCommand>,
    failure: MainWorldEntryFailure,
) {
    if matches!(
        state.phase,
        MainWorldEntryPhase::Failed | MainWorldEntryPhase::Exiting
    ) {
        return;
    }
    let generation = state.generation;
    if (state.room_membership == MainWorldRoomMembership::Joined || state.join_attempt.is_some())
        && session.authenticated
        && session.game_connection_state == GameConnectionState::Authenticated
    {
        commands.write(MyServerCommand::LeaveRoom);
        state.last_departure = MainWorldRoomDeparture::Unknown;
    }
    if let Some(session_id) = state.scene_session_id.clone() {
        scene_commands.write(SceneCommand::Exit(SceneExitRequest {
            scene_id: Some(MAIN_WORLD_AUTHORITY_CONTRACT.client_scene()),
            session_id: Some(session_id),
            ..SceneExitRequest::default()
        }));
    }
    state.fail(failure);
    events.write(MainWorldEntryEvent::Failed {
        generation,
        failure,
    });
}

fn begin_exit(
    state: &mut MainWorldEntryState,
    destination: MainWorldExitDestination,
    session: &MyServerSession,
    myserver_commands: &mut MessageWriter<MyServerCommand>,
    scene_commands: &mut MessageWriter<SceneCommand>,
    route_commands: &mut MessageWriter<GameRouteCommand>,
) {
    if state.phase == MainWorldEntryPhase::Exiting {
        return;
    }
    state.phase = MainWorldEntryPhase::Exiting;
    state.input_frozen = true;
    state.exit_destination = Some(destination);
    state.room_ready_requested = false;
    state.room_ready_acknowledged = false;
    state.reconnect_requested = false;
    state.reconnect_room_acknowledged = false;
    state.recovery_snapshot_received = false;
    state.leave_attempt = None;
    state.exit_scene_settled = state.scene_session_id.is_none();
    state.exit_authority_settled = false;

    info!(destination = ?destination, "main world exit requested");

    if state.join_attempt.is_some() && state.join_dispatched {
        // A cancelled join can still succeed later in this update stream. Wait
        // for its correlated result, then leave only if membership was created.
        state.exit_authority_settled = false;
    } else if state.join_attempt.take().is_some() {
        state.join_dispatched = false;
        state.exit_authority_settled = true;
    } else if state.room_membership != MainWorldRoomMembership::None {
        request_main_world_leave_for_exit(state, session, myserver_commands);
    } else {
        state.exit_authority_settled = true;
    }

    if destination == MainWorldExitDestination::Login {
        myserver_commands.write(MyServerCommand::Logout);
    }

    if let Some(session_id) = state.scene_session_id.clone() {
        scene_commands.write(SceneCommand::Exit(SceneExitRequest {
            scene_id: Some(MAIN_WORLD_AUTHORITY_CONTRACT.client_scene()),
            session_id: Some(session_id),
            ..SceneExitRequest::default()
        }));
    }
    complete_exit_if_settled(state, scene_commands, route_commands);
}

fn request_main_world_leave_for_exit(
    state: &mut MainWorldEntryState,
    session: &MyServerSession,
    commands: &mut MessageWriter<MyServerCommand>,
) {
    if state.leave_attempt.is_some() || state.exit_authority_settled {
        return;
    }
    if session.authenticated && session.game_connection_state == GameConnectionState::Authenticated
    {
        let attempt = state.begin_authority_attempt();
        state.leave_attempt = Some(attempt);
        info!(
            attempt,
            "main world dispatching correlated LeaveRoom before exit"
        );
        commands.write(MyServerCommand::LeaveRoomScoped {
            correlation: attempt,
        });
    } else {
        state.last_departure = MainWorldRoomDeparture::Unknown;
        state.room_membership = MainWorldRoomMembership::Unknown;
        state.exit_authority_settled = true;
    }
}

fn complete_exit_if_settled(
    state: &mut MainWorldEntryState,
    scene_commands: &mut MessageWriter<SceneCommand>,
    route_commands: &mut MessageWriter<GameRouteCommand>,
) {
    if state.phase != MainWorldEntryPhase::Exiting
        || !state.exit_scene_settled
        || !state.exit_authority_settled
    {
        return;
    }
    if state.exit_destination == Some(MainWorldExitDestination::Home) {
        state.reset();
        begin_home_enter(state, scene_commands);
    } else {
        complete_exit(state, route_commands);
    }
}

fn complete_exit(
    state: &mut MainWorldEntryState,
    route_commands: &mut MessageWriter<GameRouteCommand>,
) {
    let destination = state
        .exit_destination
        .take()
        .unwrap_or(MainWorldExitDestination::Lobby);
    if state.last_departure == MainWorldRoomDeparture::None
        && state.room_membership != MainWorldRoomMembership::None
    {
        state.last_departure = MainWorldRoomDeparture::Unknown;
    }
    state.reset();
    info!(destination = ?destination, "main world exit completed");
    route_commands.write(GameRouteCommand::ChangeMode(match destination {
        MainWorldExitDestination::Lobby => AppUiMode::Lobby,
        MainWorldExitDestination::Login => AppUiMode::Login,
        MainWorldExitDestination::Home => AppUiMode::Lobby,
    }));
}

fn begin_home_enter(
    state: &mut MainWorldEntryState,
    scene_commands: &mut MessageWriter<SceneCommand>,
) {
    if matches!(
        state.phase,
        MainWorldEntryPhase::HomeLoading | MainWorldEntryPhase::HomeActive
    ) {
        return;
    }
    state.generation = state.generation.wrapping_add(1).max(1);
    let session_id = SceneSessionId::from(format!("fangyuan-home-{}", state.generation));
    let mut request = SceneEnterRequest::new(FANGYUAN_HOME_SCENE_ID);
    request.session_id = Some(session_id.clone());
    scene_commands.write(SceneCommand::Enter(request));
    state.home_session_id = Some(session_id);
    state.phase = MainWorldEntryPhase::HomeLoading;
    state.input_frozen = true;
}

fn begin_home_return(
    state: &mut MainWorldEntryState,
    scene_commands: &mut MessageWriter<SceneCommand>,
) {
    let Some(session_id) = state.home_session_id.clone() else {
        return;
    };
    state.phase = MainWorldEntryPhase::ReturningFromHome;
    state.input_frozen = true;
    scene_commands.write(SceneCommand::Exit(SceneExitRequest {
        scene_id: Some(FANGYUAN_HOME_SCENE_ID.into()),
        session_id: Some(session_id),
        ..SceneExitRequest::default()
    }));
}

fn begin_recovery(
    state: &mut MainWorldEntryState,
    session: &MyServerSession,
    entry_events: &mut MessageWriter<MainWorldEntryEvent>,
    commands: &mut MessageWriter<MyServerCommand>,
    scene_commands: &mut MessageWriter<SceneCommand>,
    route_commands: &mut MessageWriter<GameRouteCommand>,
) {
    if state.phase == MainWorldEntryPhase::Recovering {
        return;
    }
    let Some(ticket) = session.ticket.clone().filter(|ticket| !ticket.is_empty()) else {
        begin_exit(
            state,
            MainWorldExitDestination::Login,
            session,
            commands,
            scene_commands,
            route_commands,
        );
        return;
    };
    let Some(transport) = state.authority_transport else {
        fail_authority_entry(
            state,
            session,
            entry_events,
            commands,
            scene_commands,
            MainWorldEntryFailure::ReconnectUnavailable,
        );
        error!("main world recovery unavailable because authority transport was not retained");
        return;
    };
    state.phase = MainWorldEntryPhase::Recovering;
    state.input_frozen = true;
    state.room_membership = MainWorldRoomMembership::Unknown;
    state.authoritative_scene_id = None;
    state.position = None;
    state.snapshot_generation = 0;
    state.room_ready_requested = false;
    state.room_ready_acknowledged = false;
    state.reconnect_requested = true;
    state.reconnect_room_acknowledged = false;
    state.reconnect_attempt = Some(state.begin_authority_attempt());
    state.recovery_snapshot_received = false;
    info!("main world recovery requested after connection loss");
    commands.write(MyServerCommand::ReconnectWithTicketScoped {
        ticket,
        transport,
        host: None,
        port: None,
        correlation: state.reconnect_attempt.unwrap_or_default(),
    });
}

fn resume_after_reconnect_snapshot(
    state: &mut MainWorldEntryState,
    commands: &mut MessageWriter<MyServerCommand>,
) {
    if state.phase != MainWorldEntryPhase::Recovering
        || !state.reconnect_room_acknowledged
        || !state.recovery_snapshot_received
        || state.authoritative_scene_id.is_none()
    {
        return;
    }
    info!("main world recovery snapshot accepted");
    if state.scene_session_id.is_none() {
        state.phase = MainWorldEntryPhase::LoadingScene;
        return;
    }
    state.phase = MainWorldEntryPhase::WaitingSceneReady;
    request_room_ready_if_admitted(state, commands);
}

fn begin_ready_or_scene_load_after_snapshot(
    state: &mut MainWorldEntryState,
    commands: &mut MessageWriter<MyServerCommand>,
) {
    if state.scene_session_id.is_none() {
        state.phase = MainWorldEntryPhase::LoadingScene;
        return;
    }
    state.phase = MainWorldEntryPhase::WaitingSceneReady;
    request_room_ready_if_admitted(state, commands);
}

fn request_room_ready_if_admitted(
    state: &mut MainWorldEntryState,
    commands: &mut MessageWriter<MyServerCommand>,
) {
    if state.phase != MainWorldEntryPhase::WaitingSceneReady
        || !state.scene_ready
        || !state.scene_content_ready
        || state.authoritative_scene_id.is_none()
        || state.room_ready_requested
    {
        return;
    }
    state.room_ready_requested = true;
    state.room_ready_acknowledged = false;
    state.ready_attempt = Some(state.begin_authority_attempt());
    warn!("main world scene and content ready; requesting room ready");
    commands.write(MyServerCommand::SetReadyScoped {
        ready: true,
        correlation: state.ready_attempt.unwrap_or_default(),
    });
}

fn room_join_failure(error_code: &str) -> MainWorldEntryFailure {
    match error_code.trim() {
        "ROOM_FULL" => MainWorldEntryFailure::RoomFull,
        "ROOM_POLICY_MISMATCH" | "ROOM_POLICY_UNSUPPORTED" => {
            MainWorldEntryFailure::RoomPolicyRejected
        }
        "ROOM_UNAVAILABLE" | "ROOM_NOT_FOUND" | "SERVER_DRAINING_REJECT_NEW_ROOM" => {
            MainWorldEntryFailure::RoomUnavailable
        }
        "JOIN_TIMEOUT" => MainWorldEntryFailure::JoinTimedOut,
        _ => MainWorldEntryFailure::JoinRejected,
    }
}

fn join_request_failure(error: &str) -> MainWorldEntryFailure {
    if error.to_ascii_lowercase().contains("timeout") {
        MainWorldEntryFailure::JoinTimedOut
    } else {
        MainWorldEntryFailure::JoinRejected
    }
}

fn room_ready_failure(error_code: &str) -> MainWorldEntryFailure {
    match error_code.trim() {
        "READY_TIMEOUT" => MainWorldEntryFailure::ReadyTimedOut,
        _ => MainWorldEntryFailure::JoinRejected,
    }
}

pub(in crate::game) fn main_world_bevy_position(
    x: f32,
    y: f32,
) -> Result<Vec3, MainWorldEntryFailure> {
    contract_main_world_bevy_position(x, y).map_err(|error| {
        debug!(
            ?error,
            "main world authority position rejected by coordinate contract"
        );
        MainWorldEntryFailure::InvalidAuthoritativePosition
    })
}

fn handle_entry_signals(
    mut signals: MessageReader<MainWorldEntrySignal>,
    state: Res<MainWorldEntryState>,
    mut events: MessageWriter<MainWorldEntryEvent>,
) {
    for signal in signals.read() {
        let generation = match signal {
            MainWorldEntrySignal::JoinAccepted { generation }
            | MainWorldEntrySignal::SceneReady { generation } => *generation,
        };
        if !state.accepts_generation(generation) {
            events.write(MainWorldEntryEvent::IgnoredStaleSignal { generation });
        }
    }
}

fn dispatch_authority_confirmed_scene_enter(
    mut state: ResMut<MainWorldEntryState>,
    mut scene_commands: MessageWriter<SceneCommand>,
) {
    if state.phase != MainWorldEntryPhase::LoadingScene || state.scene_session_id.is_some() {
        return;
    }
    let session_id = SceneSessionId::from(format!("main-world-{}", state.generation));
    let mut request = SceneEnterRequest::new(MAIN_WORLD_AUTHORITY_CONTRACT.client_scene());
    request.session_id = Some(session_id.clone());
    scene_commands.write(SceneCommand::Enter(request));
    warn!(
        scene_id = MAIN_WORLD_AUTHORITY_CONTRACT.client_scene().as_str(),
        session_id = %session_id,
        "main world SceneCommand::Enter dispatched"
    );
    state.scene_session_id = Some(session_id);
    state.phase = MainWorldEntryPhase::WaitingSceneReady;
}

fn consume_main_world_scene_ready(
    mut scene_events: MessageReader<SceneEvent>,
    mut state: ResMut<MainWorldEntryState>,
    session: Res<MyServerSession>,
    mut commands: MessageWriter<MyServerCommand>,
    mut entry_events: MessageWriter<MainWorldEntryEvent>,
    mut route_commands: MessageWriter<GameRouteCommand>,
    mut scene_commands: MessageWriter<SceneCommand>,
    mut intents: MessageWriter<MainWorldEntryIntent>,
) {
    for event in scene_events.read() {
        match event {
            SceneEvent::Ready(ready)
                if matches!(
                    state.phase,
                    MainWorldEntryPhase::WaitingSceneReady | MainWorldEntryPhase::Recovering
                ) && ready.scene_id == MAIN_WORLD_AUTHORITY_CONTRACT.client_scene()
                    && state.scene_session_id.as_ref() == Some(&ready.session_id)
                    && !state.scene_ready =>
            {
                state.scene_ready = true;
                if state.phase == MainWorldEntryPhase::WaitingSceneReady {
                    request_room_ready_if_admitted(&mut state, &mut commands);
                    activate_when_ready(&mut state, &mut route_commands);
                }
            }
            SceneEvent::Failed(failure)
                if matches!(
                    state.phase,
                    MainWorldEntryPhase::WaitingSceneReady | MainWorldEntryPhase::Recovering
                ) && failure.scene_id.as_ref()
                    == Some(&MAIN_WORLD_AUTHORITY_CONTRACT.client_scene())
                    && failure.session_id.as_ref() == state.scene_session_id.as_ref() =>
            {
                fail_authority_entry(
                    &mut state,
                    &session,
                    &mut entry_events,
                    &mut commands,
                    &mut scene_commands,
                    MainWorldEntryFailure::SceneLoadFailed,
                );
            }
            SceneEvent::Exited(exited)
                if state.phase == MainWorldEntryPhase::Exiting
                    && exited.scene_id == MAIN_WORLD_AUTHORITY_CONTRACT.client_scene()
                    && state.scene_session_id.as_ref() == Some(&exited.session_id) =>
            {
                info!(
                    scene_id = exited.scene_id.as_str(),
                    session_id = %exited.session_id,
                    "main world SceneEvent::Exited received during exit"
                );
                state.exit_scene_settled = true;
                complete_exit_if_settled(&mut state, &mut scene_commands, &mut route_commands);
            }
            SceneEvent::Exited(exited)
                if state.phase == MainWorldEntryPhase::Recovering
                    && exited.scene_id == MAIN_WORLD_AUTHORITY_CONTRACT.client_scene()
                    && state.scene_session_id.as_ref() == Some(&exited.session_id) =>
            {
                fail_authority_entry(
                    &mut state,
                    &session,
                    &mut entry_events,
                    &mut commands,
                    &mut scene_commands,
                    MainWorldEntryFailure::SceneLoadFailed,
                );
            }
            SceneEvent::Failed(failure)
                if state.phase == MainWorldEntryPhase::Exiting
                    && failure.scene_id.as_ref()
                        == Some(&MAIN_WORLD_AUTHORITY_CONTRACT.client_scene())
                    && failure.session_id.as_ref() == state.scene_session_id.as_ref() =>
            {
                state.exit_scene_settled = true;
                complete_exit_if_settled(&mut state, &mut scene_commands, &mut route_commands);
            }
            SceneEvent::Ready(ready)
                if state.phase == MainWorldEntryPhase::HomeLoading
                    && ready.scene_id.as_str() == FANGYUAN_HOME_SCENE_ID
                    && state.home_session_id.as_ref() == Some(&ready.session_id) =>
            {
                state.phase = MainWorldEntryPhase::HomeActive;
                route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::FangyuanHome));
            }
            SceneEvent::Failed(failure)
                if matches!(
                    state.phase,
                    MainWorldEntryPhase::HomeLoading | MainWorldEntryPhase::ReturningFromHome
                ) && failure
                    .scene_id
                    .as_ref()
                    .is_some_and(|scene_id| scene_id.as_str() == FANGYUAN_HOME_SCENE_ID)
                    && failure.session_id.as_ref() == state.home_session_id.as_ref() =>
            {
                state.reset();
                route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::Lobby));
            }
            SceneEvent::Exited(exited)
                if state.phase == MainWorldEntryPhase::ReturningFromHome
                    && exited.scene_id.as_str() == FANGYUAN_HOME_SCENE_ID
                    && state.home_session_id.as_ref() == Some(&exited.session_id) =>
            {
                state.reset();
                intents.write(MainWorldEntryIntent::Enter);
            }
            _ => {}
        }
    }
}

fn activate_when_ready(
    state: &mut MainWorldEntryState,
    route_commands: &mut MessageWriter<GameRouteCommand>,
) {
    if state.scene_ready
        && state.scene_content_ready
        && state.room_ready_acknowledged
        && state.authoritative_scene_id.is_some()
    {
        let recovered = state.reconnect_requested;
        state.phase = MainWorldEntryPhase::Active;
        state.input_frozen = false;
        state.reconnect_requested = false;
        state.reconnect_room_acknowledged = false;
        state.recovery_snapshot_received = false;
        route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::MainWorld));
        if recovered {
            info!("main world entry recovered active");
        } else {
            info!("main world entry active");
        }
    }
}

fn consume_main_world_content_events(
    mut content_events: MessageReader<MainWorldContentEvent>,
    mut state: ResMut<MainWorldEntryState>,
    session: Res<MyServerSession>,
    mut commands: MessageWriter<MyServerCommand>,
    mut entry_events: MessageWriter<MainWorldEntryEvent>,
    mut scene_commands: MessageWriter<SceneCommand>,
    mut route_commands: MessageWriter<GameRouteCommand>,
) {
    for event in content_events.read() {
        match event {
            MainWorldContentEvent::Ready {
                scene_id,
                session_id,
            } if matches!(
                state.phase,
                MainWorldEntryPhase::WaitingSceneReady | MainWorldEntryPhase::Recovering
            ) && *scene_id == MAIN_WORLD_AUTHORITY_CONTRACT.client_scene()
                && state.scene_session_id.as_ref() == Some(session_id) =>
            {
                state.scene_content_ready = true;
                if state.phase == MainWorldEntryPhase::WaitingSceneReady {
                    request_room_ready_if_admitted(&mut state, &mut commands);
                    activate_when_ready(&mut state, &mut route_commands);
                }
            }
            MainWorldContentEvent::Failed {
                scene_id,
                session_id,
                ..
            } if matches!(
                state.phase,
                MainWorldEntryPhase::LoadingScene
                    | MainWorldEntryPhase::WaitingSceneReady
                    | MainWorldEntryPhase::Recovering
            ) && *scene_id == MAIN_WORLD_AUTHORITY_CONTRACT.client_scene()
                && state.scene_session_id.as_ref() == Some(session_id) =>
            {
                fail_authority_entry(
                    &mut state,
                    &session,
                    &mut entry_events,
                    &mut commands,
                    &mut scene_commands,
                    MainWorldEntryFailure::SceneLoadFailed,
                );
            }
            _ => {}
        }
    }
}

fn watchdog_main_world_entry_progress(
    time: Res<Time<Real>>,
    mut watchdog: ResMut<MainWorldEntryWatchdog>,
    mut state: ResMut<MainWorldEntryState>,
    session: Res<MyServerSession>,
    mut entry_events: MessageWriter<MainWorldEntryEvent>,
    mut commands: MessageWriter<MyServerCommand>,
    mut scene_commands: MessageWriter<SceneCommand>,
    mut route_commands: MessageWriter<GameRouteCommand>,
) {
    let key = MainWorldEntryWatchdogKey::from_state(&state);
    if watchdog.key.as_ref() != Some(&key) {
        watchdog.key = Some(key);
        watchdog.elapsed = Duration::ZERO;
        return;
    }

    let timeout = match state.phase {
        MainWorldEntryPhase::JoiningRoom
        | MainWorldEntryPhase::LoadingScene
        | MainWorldEntryPhase::WaitingSceneReady
        | MainWorldEntryPhase::Recovering => MAIN_WORLD_ENTRY_PROGRESS_TIMEOUT,
        MainWorldEntryPhase::Exiting => MAIN_WORLD_EXIT_PROGRESS_TIMEOUT,
        _ => {
            watchdog.elapsed = Duration::ZERO;
            return;
        }
    };
    watchdog.elapsed = watchdog.elapsed.saturating_add(time.delta());
    if watchdog.elapsed < timeout {
        return;
    }
    watchdog.elapsed = Duration::ZERO;

    match state.phase {
        MainWorldEntryPhase::JoiningRoom => fail_authority_entry(
            &mut state,
            &session,
            &mut entry_events,
            &mut commands,
            &mut scene_commands,
            MainWorldEntryFailure::JoinTimedOut,
        ),
        MainWorldEntryPhase::LoadingScene | MainWorldEntryPhase::WaitingSceneReady => {
            let ready_was_requested = state.room_ready_requested;
            fail_authority_entry(
                &mut state,
                &session,
                &mut entry_events,
                &mut commands,
                &mut scene_commands,
                if ready_was_requested {
                    MainWorldEntryFailure::ReadyTimedOut
                } else {
                    MainWorldEntryFailure::SceneLoadFailed
                },
            )
        }
        MainWorldEntryPhase::Recovering => fail_authority_entry(
            &mut state,
            &session,
            &mut entry_events,
            &mut commands,
            &mut scene_commands,
            MainWorldEntryFailure::ReconnectUnavailable,
        ),
        MainWorldEntryPhase::Exiting => {
            state.last_departure = MainWorldRoomDeparture::Unknown;
            state.room_membership = MainWorldRoomMembership::Unknown;
            state.leave_attempt = None;
            state.exit_scene_settled = true;
            state.exit_authority_settled = true;
            complete_exit_if_settled(&mut state, &mut scene_commands, &mut route_commands);
        }
        _ => {}
    }
}

fn route_failed_authority_entry(
    mut state: ResMut<MainWorldEntryState>,
    session: Res<MyServerSession>,
    mut route_commands: MessageWriter<GameRouteCommand>,
) {
    if state.phase != MainWorldEntryPhase::Failed || state.failure_routed {
        return;
    }
    state.failure_routed = true;
    let destination = match state.failure {
        Some(
            MainWorldEntryFailure::AccountSessionUnavailable
            | MainWorldEntryFailure::TicketUnavailable
            | MainWorldEntryFailure::GameAuthUnavailable,
        ) => AppUiMode::Login,
        Some(MainWorldEntryFailure::ReconnectUnavailable) if !session.authenticated => {
            AppUiMode::Login
        }
        _ => AppUiMode::Lobby,
    };
    route_commands.write(GameRouteCommand::ChangeMode(destination));
}

fn abort_for_intent(
    intent: &MainWorldEntryIntent,
    state: &mut MainWorldEntryState,
    events: &mut MessageWriter<MainWorldEntryEvent>,
) {
    let reason = match intent {
        MainWorldEntryIntent::Cancel => MainWorldEntryAbortReason::Cancelled,
        MainWorldEntryIntent::ExitToLobby => MainWorldEntryAbortReason::ExitToLobby,
        MainWorldEntryIntent::EnvironmentChanged => MainWorldEntryAbortReason::EnvironmentChanged,
        MainWorldEntryIntent::CharacterChanged => MainWorldEntryAbortReason::CharacterChanged,
        MainWorldEntryIntent::LoggedOut => MainWorldEntryAbortReason::LoggedOut,
        MainWorldEntryIntent::ApplicationExit => MainWorldEntryAbortReason::ApplicationExit,
        MainWorldEntryIntent::Enter
        | MainWorldEntryIntent::Recover
        | MainWorldEntryIntent::EnterHome
        | MainWorldEntryIntent::ReturnFromHome => return,
    };
    abort_entry(state, events, reason);
}

fn abort_entry(
    state: &mut MainWorldEntryState,
    events: &mut MessageWriter<MainWorldEntryEvent>,
    reason: MainWorldEntryAbortReason,
) {
    if !state.is_in_flight() {
        return;
    }
    let generation = state.generation;
    state.reset();
    events.write(MainWorldEntryEvent::Aborted { generation, reason });
}

fn abort_invalidated_entry_with_cleanup(
    state: &mut MainWorldEntryState,
    session: &MyServerSession,
    events: &mut MessageWriter<MainWorldEntryEvent>,
    myserver_commands: &mut MessageWriter<MyServerCommand>,
    scene_commands: &mut MessageWriter<SceneCommand>,
    route_commands: &mut MessageWriter<GameRouteCommand>,
) {
    if !state.is_in_flight() {
        return;
    }
    let generation = state.generation;
    if (state.room_membership == MainWorldRoomMembership::Joined || state.join_attempt.is_some())
        && state.character_id.as_deref() == session.character_id.as_deref()
        && session.authenticated
        && session.game_connection_state == GameConnectionState::Authenticated
    {
        myserver_commands.write(MyServerCommand::LeaveRoom);
        state.last_departure = MainWorldRoomDeparture::Unknown;
    }
    if let Some(session_id) = state.scene_session_id.clone() {
        scene_commands.write(SceneCommand::Exit(SceneExitRequest {
            scene_id: Some(MAIN_WORLD_AUTHORITY_CONTRACT.client_scene()),
            session_id: Some(session_id),
            ..SceneExitRequest::default()
        }));
    }
    state.reset();
    events.write(MainWorldEntryEvent::Aborted {
        generation,
        reason: MainWorldEntryAbortReason::PreconditionsInvalidated,
    });
    route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::Lobby));
}

fn validate_entry(
    profiles: &MyServerProfiles,
    session: &MyServerSession,
    registry: &SceneRegistry,
) -> Result<(), MainWorldEntryFailure> {
    let config = profiles.config(profiles.selected());
    if config.http_base_url.trim().is_empty() || config.game_host.trim().is_empty() {
        return Err(MainWorldEntryFailure::EnvironmentUnavailable);
    }
    if !matches!(session.account_login_state, AccountLoginState::LoggedIn) {
        return Err(MainWorldEntryFailure::AccountSessionUnavailable);
    }
    if !matches!(
        session.character_selection_state,
        CharacterSelectionState::Selected
    ) || session.character_id.as_deref().is_none_or(str::is_empty)
    {
        return Err(MainWorldEntryFailure::CharacterUnavailable);
    }
    if session.ticket.as_deref().is_none_or(str::is_empty) {
        return Err(MainWorldEntryFailure::TicketUnavailable);
    }
    if !session.authenticated
        || session.game_connection_state != GameConnectionState::Authenticated
        || session.connection_id.is_none()
    {
        return Err(MainWorldEntryFailure::GameAuthUnavailable);
    }
    registry
        .contains(&MAIN_WORLD_AUTHORITY_CONTRACT.client_scene())
        .then_some(())
        .ok_or(MainWorldEntryFailure::SceneMappingUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::{
        scene::prelude::{SceneDefinition, SceneKind},
        ui::document::{
            UiDocumentId, UiDocumentInstanceId, UiDocumentLayer, UiDocumentRequestId,
            UiDocumentSourceOrigin,
        },
    };
    use crate::game::myserver::protocol::pb;
    use bevy::ecs::message::{MessageCursor, Messages};
    use std::str::FromStr;

    fn ready_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<MyServerProfiles>()
            .init_resource::<MyServerSession>()
            .init_resource::<SceneRegistry>()
            .add_message::<SceneEvent>()
            .add_message::<SceneCommand>()
            .add_message::<MyServerCommand>()
            .add_message::<MyServerEvent>()
            .add_message::<GameRouteCommand>()
            .add_plugins(MainWorldEntryPlugin);
        let session = &mut *app.world_mut().resource_mut::<MyServerSession>();
        session.account_login_state = AccountLoginState::LoggedIn;
        session.character_selection_state = CharacterSelectionState::Selected;
        session.character_id = Some("chr_1".to_owned());
        session.ticket = Some("character-bound-ticket".to_owned());
        session.authenticated = true;
        session.game_connection_state = GameConnectionState::Authenticated;
        session.connection_id = Some(test_connection_id());
        session.transport = Some(NetworkTransport::Tcp);
        app.world_mut()
            .resource_mut::<SceneRegistry>()
            .register(SceneDefinition::new(
                MAIN_WORLD_AUTHORITY_CONTRACT.client_scene(),
                SceneKind::World,
            ))
            .unwrap();
        app
    }

    fn test_connection_id() -> ConnectionId {
        ConnectionId::from_raw(1)
    }

    fn events(app: &App) -> Vec<MainWorldEntryEvent> {
        let messages = app.world().resource::<Messages<MainWorldEntryEvent>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    fn messages<M: Message + Clone>(app: &App) -> Vec<M> {
        let mut cursor = MessageCursor::default();
        cursor
            .read(app.world().resource::<Messages<M>>())
            .cloned()
            .collect()
    }

    fn app_waiting_scene_ready() -> (App, SceneSessionId) {
        let mut app = ready_app();
        app.world_mut().write_message(MainWorldEntryIntent::Enter);
        app.update();
        app.world_mut().write_message(room_join_ack());
        app.world_mut()
            .write_message(MyServerEvent::RoomStatePush(pb::RoomStatePush {
                event: "joined".to_owned(),
                snapshot: Some(pb::RoomSnapshot {
                    room_id: "main-world-public".to_owned(),
                    owner_character_id: "chr_1".to_owned(),
                    state: "in_game".to_owned(),
                    members: Vec::new(),
                    current_frame_id: 7,
                    game_state: r#"{"scene_id":1}"#.to_owned(),
                }),
            }));
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(
                pb::MovementSnapshotPush {
                    room_id: "main-world-public".to_owned(),
                    frame_id: 8,
                    entities: vec![pb::EntityTransform {
                        entity_id: 1,
                        character_id: "chr_1".to_owned(),
                        scene_id: 1,
                        x: 2002.0,
                        y: 2003.0,
                        dir_x: 0.0,
                        dir_y: 1.0,
                        moving: false,
                        last_input_frame: 8,
                    }],
                    full_sync: true,
                    reason: String::new(),
                    correction_kind: 0,
                    reason_code: 0,
                    target_character_ids: Vec::new(),
                    reference_frame_id: 8,
                },
            ));
        app.update();
        let session_id = app
            .world()
            .resource::<MainWorldEntryState>()
            .scene_session_id
            .clone()
            .unwrap();
        (app, session_id)
    }

    fn scene_ready(session_id: SceneSessionId) -> SceneEvent {
        SceneEvent::Ready(crate::framework::scene::prelude::SceneReady {
            scene_id: MAIN_WORLD_AUTHORITY_CONTRACT.client_scene(),
            session_id,
            content_version: None,
            authority_mode: Default::default(),
            seed: None,
        })
    }

    fn room_ready_response(app: &App, ok: bool, error_code: &str) -> MyServerEvent {
        let state = app.world().resource::<MainWorldEntryState>();
        let correlation = state
            .ready_attempt
            .expect("room ready request must be in flight");
        let connection_id = state
            .authority_connection_id
            .expect("room ready request must retain its authority connection");
        MyServerEvent::ReadyChangedScoped {
            correlation,
            seq: 1,
            connection_id,
            response: pb::RoomReadyRes {
                ok,
                room_id: "main-world-public".to_owned(),
                ready: true,
                error_code: error_code.to_owned(),
            },
        }
    }

    fn room_ready_ack(app: &App) -> MyServerEvent {
        room_ready_response(app, true, "")
    }

    fn room_leave_ack(app: &App) -> MyServerEvent {
        let correlation = app
            .world()
            .resource::<MainWorldEntryState>()
            .leave_attempt
            .expect("room leave request must be in flight");
        MyServerEvent::RoomLeftScoped {
            correlation,
            seq: 1,
            connection_id: test_connection_id(),
            response: pb::RoomLeaveRes {
                ok: true,
                room_id: "main-world-public".to_owned(),
                error_code: String::new(),
            },
        }
    }

    fn room_join_ack() -> MyServerEvent {
        MyServerEvent::RoomJoinedScoped {
            correlation: 1,
            seq: 1,
            connection_id: test_connection_id(),
            response: pb::RoomJoinRes {
                ok: true,
                room_id: "main-world-public".to_owned(),
                error_code: String::new(),
            },
        }
    }

    fn bind_reconnected_transport(app: &mut App, connection_id: ConnectionId) {
        {
            let mut session = app.world_mut().resource_mut::<MyServerSession>();
            session.connection_id = Some(connection_id);
            session.transport = Some(NetworkTransport::Tcp);
            session.connected = true;
            session.authenticated = true;
            session.game_connection_state = GameConnectionState::Authenticated;
        }
        app.world_mut().write_message(MyServerEvent::Connected {
            connection_id,
            transport: NetworkTransport::Tcp,
            remote_addr: "127.0.0.1:4000".to_owned(),
        });
        app.update();
    }

    fn content_ready(session_id: SceneSessionId) -> MainWorldContentEvent {
        MainWorldContentEvent::Ready {
            scene_id: MAIN_WORLD_AUTHORITY_CONTRACT.client_scene(),
            session_id,
        }
    }

    fn active_app() -> (App, SceneSessionId) {
        let (mut app, session_id) = app_waiting_scene_ready();
        app.world_mut()
            .write_message(scene_ready(session_id.clone()));
        app.world_mut()
            .write_message(content_ready(session_id.clone()));
        app.update();
        let ready_ack = room_ready_ack(&app);
        app.world_mut().write_message(ready_ack);
        app.update();
        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::Active
        );
        (app, session_id)
    }

    #[test]
    fn debug_auto_enter_only_enables_for_explicit_truthy_values() {
        assert!(
            MainWorldEntryDebugConfig::from_env_reader(|key| {
                (key == ENV_MAIN_WORLD_AUTO_ENTER).then_some("enabled".to_owned())
            })
            .auto_enter
        );
        assert!(
            !MainWorldEntryDebugConfig::from_env_reader(|key| {
                (key == ENV_MAIN_WORLD_AUTO_ENTER).then_some("no".to_owned())
            })
            .auto_enter
        );
        assert!(
            MainWorldEntryDebugConfig::from_env_reader(|key| {
                (key == ENV_MAIN_WORLD_AUTO_EXIT_AFTER_RECOVERY).then_some("on".to_owned())
            })
            .auto_exit_after_recovery
        );
        assert!(
            MainWorldEntryDebugConfig::from_env_reader(|key| {
                (key == ENV_MAIN_WORLD_AUTO_EXIT).then_some("true".to_owned())
            })
            .auto_exit
        );
        assert!(
            MainWorldEntryDebugConfig::from_env_reader(|key| {
                (key == ENV_MAIN_WORLD_ACCEPTANCE_METRICS).then_some("yes".to_owned())
            })
            .acceptance_metrics
        );
    }

    #[test]
    fn debug_auto_enter_waits_for_authority_preconditions_then_joins_once() {
        let mut app = ready_app();
        app.world_mut().insert_resource(MainWorldEntryDebugConfig {
            auto_enter: true,
            auto_enter_sent: false,
            auto_exit: false,
            auto_exit_after_recovery: false,
            auto_exit_sent: false,
            acceptance_metrics: false,
            metrics_elapsed_seconds: 0.0,
            metrics_frames: 0,
            metrics_reported: false,
        });

        app.update();
        app.update();

        assert!(
            app.world()
                .resource::<MainWorldEntryDebugConfig>()
                .auto_enter_sent
        );
        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::JoiningRoom
        );
        assert_eq!(
            messages::<MyServerCommand>(&app)
                .iter()
                .filter(|command| matches!(command, MyServerCommand::JoinRoomScoped { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn debug_auto_exit_only_runs_once_after_a_recovered_active_state() {
        let (mut app, session_id) = active_app();
        {
            let mut state = app.world_mut().resource_mut::<MainWorldEntryState>();
            state.reconnect_requested = true;
            state.reconnect_room_acknowledged = true;
        }
        app.world_mut().insert_resource(MainWorldEntryDebugConfig {
            auto_enter: false,
            auto_enter_sent: false,
            auto_exit: false,
            auto_exit_after_recovery: true,
            auto_exit_sent: false,
            acceptance_metrics: false,
            metrics_elapsed_seconds: 0.0,
            metrics_frames: 0,
            metrics_reported: false,
        });

        app.update();
        app.update();

        assert!(
            app.world()
                .resource::<MainWorldEntryDebugConfig>()
                .auto_exit_sent
        );
        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::Exiting
        );
        assert_eq!(
            messages::<MyServerCommand>(&app)
                .iter()
                .filter(|command| matches!(command, MyServerCommand::LeaveRoomScoped { .. }))
                .count(),
            1
        );
        assert_eq!(
            messages::<SceneCommand>(&app)
                .iter()
                .filter(|command| matches!(
                    command,
                    SceneCommand::Exit(request)
                        if request.scene_id == Some(MAIN_WORLD_AUTHORITY_CONTRACT.client_scene())
                            && request.session_id.as_ref() == Some(&session_id)
                ))
                .count(),
            1
        );
    }

    #[test]
    fn room_state_frame_does_not_discard_a_late_join_recovery_snapshot() {
        let mut app = ready_app();
        app.world_mut().write_message(MainWorldEntryIntent::Enter);
        app.update();
        app.world_mut().write_message(room_join_ack());
        app.world_mut()
            .write_message(MyServerEvent::RoomStatePush(pb::RoomStatePush {
                event: "joined".to_owned(),
                snapshot: Some(pb::RoomSnapshot {
                    room_id: "main-world-public".to_owned(),
                    owner_character_id: "chr_other".to_owned(),
                    state: "in_game".to_owned(),
                    members: Vec::new(),
                    current_frame_id: 120,
                    game_state: r#"{"scene_id":1}"#.to_owned(),
                }),
            }));
        app.world_mut()
            .write_message(movement_snapshot(0, 2002.0, 2003.0));
        app.update();

        let state = app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.phase, MainWorldEntryPhase::WaitingSceneReady);
        assert_eq!(state.snapshot_generation, 0);
        assert_eq!(state.position, Some(Vec3::new(2.0, 0.0, 3.0)));
        assert!(state.scene_session_id.is_some());
    }

    #[test]
    fn running_room_state_embedded_movement_completes_first_entry() {
        let mut app = ready_app();
        app.world_mut().write_message(MainWorldEntryIntent::Enter);
        app.update();
        app.world_mut().write_message(room_join_ack());
        app.world_mut()
            .write_message(MyServerEvent::RoomStatePush(pb::RoomStatePush {
                event: "joined".to_owned(),
                snapshot: Some(pb::RoomSnapshot {
                    room_id: "main-world-public".to_owned(),
                    state: "in_game".to_owned(),
                    current_frame_id: 120,
                    game_state: r#"{"room_id":"main-world-public","scene_id":1,"entities":[{"entity_id":1,"character_id":"chr_1","scene_id":1,"x":2002.0,"y":2003.0,"dir_x":0.0,"dir_y":1.0,"moving":false,"last_input_frame":119}]}"#.to_owned(),
                    ..Default::default()
                }),
            }));
        app.update();

        let state = app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.phase, MainWorldEntryPhase::WaitingSceneReady);
        assert_eq!(state.snapshot_generation, 120);
        assert_eq!(state.position, Some(Vec3::new(2.0, 0.0, 3.0)));
        assert!(!state.room_start_requested);
    }

    #[test]
    fn running_public_room_join_does_not_restart_the_movement_policy() {
        let mut app = ready_app();
        app.world_mut().write_message(MainWorldEntryIntent::Enter);
        app.update();
        app.world_mut().write_message(room_join_ack());
        app.world_mut()
            .write_message(MyServerEvent::RoomStatePush(pb::RoomStatePush {
                event: "joined".to_owned(),
                snapshot: Some(pb::RoomSnapshot {
                    room_id: "main-world-public".to_owned(),
                    state: "in_game".to_owned(),
                    game_state: r#"{"scene_id":1}"#.to_owned(),
                    ..Default::default()
                }),
            }));
        app.update();

        assert!(
            !app.world()
                .resource::<MainWorldEntryState>()
                .room_start_requested
        );
        assert_eq!(
            messages::<MyServerCommand>(&app)
                .iter()
                .filter(|command| matches!(command, MyServerCommand::StartRoom))
                .count(),
            0
        );
    }

    #[test]
    fn waiting_public_room_starts_the_movement_policy_once() {
        let mut app = ready_app();
        app.world_mut().write_message(MainWorldEntryIntent::Enter);
        app.update();
        app.world_mut().write_message(room_join_ack());
        for state in ["waiting", "ready"] {
            app.world_mut()
                .write_message(MyServerEvent::RoomStatePush(pb::RoomStatePush {
                    event: "joined".to_owned(),
                    snapshot: Some(pb::RoomSnapshot {
                        room_id: "main-world-public".to_owned(),
                        state: state.to_owned(),
                        game_state: r#"{"scene_id":1}"#.to_owned(),
                        ..Default::default()
                    }),
                }));
            app.update();
        }

        assert!(
            app.world()
                .resource::<MainWorldEntryState>()
                .room_start_requested
        );
        assert_eq!(
            messages::<MyServerCommand>(&app)
                .iter()
                .filter(|command| matches!(command, MyServerCommand::StartRoom))
                .count(),
            1
        );
    }

    #[test]
    fn debug_auto_exit_can_leave_an_initial_active_state_once() {
        let (mut app, _) = active_app();
        app.world_mut().insert_resource(MainWorldEntryDebugConfig {
            auto_enter: false,
            auto_enter_sent: false,
            auto_exit: true,
            auto_exit_after_recovery: false,
            auto_exit_sent: false,
            acceptance_metrics: false,
            metrics_elapsed_seconds: 0.0,
            metrics_frames: 0,
            metrics_reported: false,
        });

        app.update();
        app.update();

        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::Exiting
        );
        assert_eq!(
            messages::<MyServerCommand>(&app)
                .iter()
                .filter(|command| matches!(command, MyServerCommand::LeaveRoomScoped { .. }))
                .count(),
            1
        );
    }

    fn movement_snapshot(frame_id: u32, x: f32, y: f32) -> MyServerEvent {
        movement_snapshot_with_scene(frame_id, 1, x, y)
    }

    fn movement_snapshot_with_scene(frame_id: u32, scene_id: i32, x: f32, y: f32) -> MyServerEvent {
        MyServerEvent::MovementSnapshotPush(pb::MovementSnapshotPush {
            room_id: "main-world-public".to_owned(),
            frame_id,
            entities: vec![pb::EntityTransform {
                entity_id: 1,
                character_id: "chr_1".to_owned(),
                scene_id,
                x,
                y,
                dir_x: 0.0,
                dir_y: 1.0,
                moving: false,
                last_input_frame: frame_id,
            }],
            full_sync: true,
            reason: String::new(),
            correction_kind: 0,
            reason_code: 0,
            target_character_ids: Vec::new(),
            reference_frame_id: frame_id,
        })
    }

    #[test]
    fn valid_duplicate_enters_create_one_generation_bound_join_boundary() {
        let mut app = ready_app();
        app.world_mut().write_message(MainWorldEntryIntent::Enter);
        app.world_mut().write_message(MainWorldEntryIntent::Enter);
        app.update();

        let state = app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.generation, 1);
        assert_eq!(state.phase, MainWorldEntryPhase::JoiningRoom);
        assert!(matches!(
            events(&app).as_slice(),
            [MainWorldEntryEvent::JoinRequested {
                generation: 1,
                attempt: 1,
                room_id: "main-world-public",
                policy_id: "movement_demo",
                character_id,
                ..
            }] if character_id == "chr_1"
        ));
    }

    #[test]
    fn old_connection_join_response_cannot_advance_the_current_entry_attempt() {
        let mut app = ready_app();
        app.world_mut().write_message(MainWorldEntryIntent::Enter);
        app.update();
        let attempt = app
            .world()
            .resource::<MainWorldEntryState>()
            .join_attempt
            .expect("join attempt must be active");

        app.world_mut()
            .write_message(MyServerEvent::RoomJoinedScoped {
                correlation: attempt,
                seq: 91,
                connection_id: ConnectionId::from_raw(2),
                response: pb::RoomJoinRes {
                    ok: true,
                    room_id: "main-world-public".to_owned(),
                    error_code: String::new(),
                },
            });
        app.update();

        let state = app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.phase, MainWorldEntryPhase::JoiningRoom);
        assert_eq!(state.join_attempt, Some(attempt));
        assert!(!state.join_acknowledged);
        assert_eq!(state.room_membership, MainWorldRoomMembership::None);

        app.world_mut()
            .write_message(MyServerEvent::RoomJoinedScoped {
                correlation: attempt,
                seq: 92,
                connection_id: test_connection_id(),
                response: pb::RoomJoinRes {
                    ok: true,
                    room_id: "main-world-public".to_owned(),
                    error_code: String::new(),
                },
            });
        app.update();
        assert!(
            app.world()
                .resource::<MainWorldEntryState>()
                .join_acknowledged
        );
    }

    #[test]
    fn same_frame_cancel_prevents_a_stale_join_request_dispatch() {
        let mut app = ready_app();
        app.world_mut().write_message(MainWorldEntryIntent::Enter);
        app.world_mut().write_message(MainWorldEntryIntent::Cancel);
        app.update();

        let state = app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.phase, MainWorldEntryPhase::LobbyIdle);
        assert!(
            messages::<MyServerCommand>(&app)
                .iter()
                .all(|command| !matches!(command, MyServerCommand::JoinRoomScoped { .. }))
        );
    }

    #[test]
    fn snapshot_before_join_acknowledgement_is_not_admitted() {
        let mut app = ready_app();
        app.world_mut().write_message(MainWorldEntryIntent::Enter);
        app.update();
        app.world_mut()
            .write_message(movement_snapshot(1, 2002.0, 2003.0));
        app.update();

        let state = app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.phase, MainWorldEntryPhase::JoiningRoom);
        assert!(!state.join_acknowledged);
        assert!(state.position.is_none());
        assert!(state.scene_session_id.is_none());
    }

    #[test]
    fn active_entry_ignores_a_new_enter_intent() {
        let (mut app, session_id) = active_app();
        let generation = app.world().resource::<MainWorldEntryState>().generation;
        app.world_mut().write_message(MainWorldEntryIntent::Enter);
        app.update();

        let state = app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.phase, MainWorldEntryPhase::Active);
        assert_eq!(state.generation, generation);
        assert_eq!(state.scene_session_id.as_ref(), Some(&session_id));
    }

    #[test]
    fn missing_game_auth_fails_without_a_join_request() {
        let mut app = ready_app();
        let session = &mut *app.world_mut().resource_mut::<MyServerSession>();
        session.authenticated = false;
        session.game_connection_state = GameConnectionState::Connected;
        app.world_mut().write_message(MainWorldEntryIntent::Enter);
        app.update();

        assert_eq!(
            app.world().resource::<MainWorldEntryState>().failure,
            Some(MainWorldEntryFailure::GameAuthUnavailable)
        );
        assert!(matches!(
            events(&app).as_slice(),
            [MainWorldEntryEvent::Failed {
                generation: 1,
                failure: MainWorldEntryFailure::GameAuthUnavailable,
            }]
        ));
    }

    #[test]
    fn room_join_errors_map_to_stable_entry_failures() {
        assert_eq!(
            room_join_failure("ROOM_FULL"),
            MainWorldEntryFailure::RoomFull
        );
        for error_code in ["ROOM_POLICY_MISMATCH", "ROOM_POLICY_UNSUPPORTED"] {
            assert_eq!(
                room_join_failure(error_code),
                MainWorldEntryFailure::RoomPolicyRejected
            );
        }
        assert_eq!(
            room_join_failure("JOIN_TIMEOUT"),
            MainWorldEntryFailure::JoinTimedOut
        );
    }

    #[test]
    fn unknown_authoritative_scene_fails_before_client_scene_enter() {
        let mut app = ready_app();
        app.world_mut().write_message(MainWorldEntryIntent::Enter);
        app.update();
        app.world_mut().write_message(room_join_ack());
        app.world_mut()
            .write_message(movement_snapshot_with_scene(1, 999, 2.0, 3.0));
        app.update();

        let state = app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.phase, MainWorldEntryPhase::Failed);
        assert_eq!(
            state.failure,
            Some(MainWorldEntryFailure::AuthoritativeSceneMismatch)
        );
        assert!(state.scene_session_id.is_none());
    }

    #[test]
    fn stale_signal_does_not_change_the_current_request() {
        let mut app = ready_app();
        app.world_mut().write_message(MainWorldEntryIntent::Enter);
        app.update();
        app.world_mut()
            .write_message(MainWorldEntrySignal::JoinAccepted { generation: 0 });
        app.update();

        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::JoiningRoom
        );
        assert!(events(&app).iter().any(|event| matches!(
            event,
            MainWorldEntryEvent::IgnoredStaleSignal { generation: 0 }
        )));
    }

    #[test]
    fn coordinate_conversion_rejects_non_finite_and_out_of_bounds_values() {
        assert_eq!(
            main_world_bevy_position(2002.0, 2003.0),
            Ok(Vec3::new(2.0, 0.0, 3.0))
        );
        assert_eq!(
            main_world_bevy_position(f32::NAN, 1.0),
            Err(MainWorldEntryFailure::InvalidAuthoritativePosition)
        );
        assert_eq!(
            main_world_bevy_position(4000.0, 1.0),
            Err(MainWorldEntryFailure::InvalidAuthoritativePosition)
        );
    }

    #[cfg(all(debug_assertions, not(target_os = "android")))]
    #[test]
    fn hud_audit_fixture_is_active_without_session_or_authority_data() {
        for fixture_id in [
            MAIN_WORLD_HUD_AUDIT_FIXTURE_ID,
            MAIN_WORLD_MAIL_AUDIT_FIXTURE_ID,
        ] {
            let mut state = MainWorldEntryState::default();
            let mut next_mode = NextState::default();

            apply_main_world_hud_audit_fixture(true, Some(fixture_id), &mut state, &mut next_mode);

            assert_eq!(state.generation, 1);
            assert_eq!(state.phase, MainWorldEntryPhase::Active);
            assert!(state.environment.is_none());
            assert!(state.character_id.is_none());
            assert!(state.authority_transport.is_none());
            assert!(state.scene_session_id.is_none());
            assert_eq!(state.room_membership, MainWorldRoomMembership::None);
            assert!(state.input_frozen);
        }
    }

    #[cfg(all(debug_assertions, not(target_os = "android")))]
    #[test]
    fn hud_audit_fixture_leaves_real_entry_state_unchanged_when_not_selected() {
        for (targets_main_world, fixture_id) in [
            (false, Some(MAIN_WORLD_HUD_AUDIT_FIXTURE_ID)),
            (true, Some("different_fixture")),
        ] {
            let (mut app, _) = active_app();
            let mut next_mode = NextState::default();
            let expected = {
                let state = app.world().resource::<MainWorldEntryState>();
                (
                    state.generation,
                    state.phase,
                    state.environment.clone(),
                    state.character_id.clone(),
                    state.authority_transport.clone(),
                    state.scene_session_id.clone(),
                    state.room_membership,
                )
            };

            apply_main_world_hud_audit_fixture(
                targets_main_world,
                fixture_id,
                &mut app.world_mut().resource_mut::<MainWorldEntryState>(),
                &mut next_mode,
            );

            let state = app.world().resource::<MainWorldEntryState>();
            assert_eq!(state.generation, expected.0);
            assert_eq!(state.phase, expected.1);
            assert_eq!(state.environment, expected.2);
            assert_eq!(state.character_id, expected.3);
            assert_eq!(state.authority_transport, expected.4);
            assert_eq!(state.scene_session_id, expected.5);
            assert_eq!(state.room_membership, expected.6);
        }
    }

    #[test]
    fn authority_snapshot_scene_ready_and_room_ack_form_the_only_active_path() {
        let (mut app, session_id) = app_waiting_scene_ready();
        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::WaitingSceneReady
        );
        assert_eq!(
            messages::<SceneCommand>(&app)
                .iter()
                .filter(|command| matches!(command, SceneCommand::Enter(_)))
                .count(),
            1
        );

        app.world_mut().write_message(SceneEvent::Entered(
            crate::framework::scene::prelude::SceneEntered {
                scene_id: MAIN_WORLD_AUTHORITY_CONTRACT.client_scene(),
                session_id: session_id.clone(),
                content_version: None,
            },
        ));
        app.update();
        assert!(!app.world().resource::<MainWorldEntryState>().scene_ready);

        app.world_mut()
            .write_message(scene_ready(SceneSessionId::from("main-world-0")));
        app.update();
        assert!(!app.world().resource::<MainWorldEntryState>().scene_ready);
        assert_eq!(
            messages::<MyServerCommand>(&app)
                .iter()
                .filter(|command| matches!(
                    command,
                    MyServerCommand::SetReadyScoped { ready: true, .. }
                ))
                .count(),
            0
        );

        app.world_mut()
            .write_message(scene_ready(session_id.clone()));
        app.world_mut()
            .write_message(scene_ready(session_id.clone()));
        app.world_mut()
            .write_message(content_ready(session_id.clone()));
        app.update();
        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::WaitingSceneReady
        );
        assert_eq!(
            messages::<MyServerCommand>(&app)
                .iter()
                .filter(|command| matches!(
                    command,
                    MyServerCommand::SetReadyScoped { ready: true, .. }
                ))
                .count(),
            1
        );

        let ready_ack = room_ready_ack(&app);
        app.world_mut().write_message(ready_ack);
        app.update();
        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::Active
        );
        assert!(messages::<GameRouteCommand>(&app).iter().any(|command| {
            matches!(command, GameRouteCommand::ChangeMode(AppUiMode::MainWorld))
        }));
    }

    #[test]
    fn scene_failures_return_the_entry_coordinator_to_an_operable_lobby_state() {
        use crate::framework::scene::prelude::{
            SceneFailure, SceneFailureKind, SceneLifecycleState,
        };

        for (kind, lifecycle_state) in [
            (
                SceneFailureKind::RequiredAssetMissing,
                SceneLifecycleState::LoadingAssets,
            ),
            (
                SceneFailureKind::ManifestLoadFailed,
                SceneLifecycleState::Resolving,
            ),
            (
                SceneFailureKind::CameraSetupFailed,
                SceneLifecycleState::Activating,
            ),
            (
                SceneFailureKind::SpawnPointMissing,
                SceneLifecycleState::Activating,
            ),
        ] {
            let (mut app, session_id) = app_waiting_scene_ready();
            app.world_mut().write_message(SceneEvent::Failed(
                SceneFailure::new(kind, lifecycle_state)
                    .with_scene(MAIN_WORLD_AUTHORITY_CONTRACT.client_scene())
                    .with_session(session_id),
            ));
            app.update();

            let state = app.world().resource::<MainWorldEntryState>();
            assert_eq!(state.phase, MainWorldEntryPhase::Failed);
            assert_eq!(state.failure, Some(MainWorldEntryFailure::SceneLoadFailed));
            assert!(!state.is_in_flight());
            assert!(events(&app).iter().any(|event| matches!(
                event,
                MainWorldEntryEvent::Failed {
                    generation: 1,
                    failure: MainWorldEntryFailure::SceneLoadFailed,
                }
            )));
        }
    }

    #[test]
    fn room_ready_failure_and_timeout_return_to_a_determinate_entry_state() {
        let (mut response_app, response_session_id) = app_waiting_scene_ready();
        response_app
            .world_mut()
            .write_message(scene_ready(response_session_id.clone()));
        response_app
            .world_mut()
            .write_message(content_ready(response_session_id));
        response_app.update();
        let ready_failure = room_ready_response(&response_app, false, "READY_TIMEOUT");
        response_app.world_mut().write_message(ready_failure);
        response_app.update();
        assert_eq!(
            response_app
                .world()
                .resource::<MainWorldEntryState>()
                .failure,
            Some(MainWorldEntryFailure::ReadyTimedOut)
        );

        let (mut request_app, request_session_id) = app_waiting_scene_ready();
        request_app
            .world_mut()
            .write_message(scene_ready(request_session_id.clone()));
        request_app
            .world_mut()
            .write_message(content_ready(request_session_id));
        request_app.update();
        let ready_attempt = request_app
            .world()
            .resource::<MainWorldEntryState>()
            .ready_attempt
            .unwrap();
        request_app
            .world_mut()
            .write_message(MyServerEvent::ScopedRequestFailed {
                seq: 1,
                message_type: crate::game::myserver::protocol::MessageType::RoomReadyReq,
                correlation: ready_attempt,
                connection_id: Some(test_connection_id()),
                error: "request timeout".to_owned(),
            });
        request_app.update();
        let state = request_app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.phase, MainWorldEntryPhase::Failed);
        assert_eq!(state.failure, Some(MainWorldEntryFailure::ReadyTimedOut));
        assert!(!state.is_in_flight());
    }

    #[test]
    fn cancelled_entry_ignores_a_late_room_ready_ack() {
        let (mut app, session_id) = app_waiting_scene_ready();
        app.world_mut()
            .write_message(scene_ready(session_id.clone()));
        app.world_mut()
            .write_message(content_ready(session_id.clone()));
        app.update();
        let stale_ready_ack = room_ready_ack(&app);
        app.world_mut().write_message(MainWorldEntryIntent::Cancel);
        app.world_mut().write_message(stale_ready_ack);
        app.update();

        let state = app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.phase, MainWorldEntryPhase::Exiting);
        assert!(!state.room_ready_acknowledged);
        assert!(state.input_frozen);

        app.world_mut().write_message(SceneEvent::Exited(
            crate::framework::scene::prelude::SceneExited {
                scene_id: MAIN_WORLD_AUTHORITY_CONTRACT.client_scene(),
                session_id,
            },
        ));
        app.update();
        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::Exiting
        );
        let leave_ack = room_leave_ack(&app);
        app.world_mut().write_message(leave_ack);
        app.update();
        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::LobbyIdle
        );
    }

    #[test]
    fn active_exit_leaves_room_exits_its_scene_and_returns_to_lobby() {
        let (mut app, session_id) = active_app();
        let ticket = app.world().resource::<MyServerSession>().ticket.clone();
        let character_id = app
            .world()
            .resource::<MyServerSession>()
            .character_id
            .clone();
        app.world_mut()
            .write_message(MainWorldEntryIntent::ExitToLobby);
        app.update();

        let state = app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.phase, MainWorldEntryPhase::Exiting);
        assert!(state.input_frozen);
        assert!(matches!(
            messages::<UiPanelCommand>(&app).as_slice(),
            [
                UiPanelCommand::CloseAllForOwner(mail),
                UiPanelCommand::CloseAllForOwner(settings),
                UiPanelCommand::CloseAllForOwner(hud),
            ] if *mail == crate::game::ui_ids::OWNER_MAIN_WORLD_MAIL_PANEL
                && *settings == crate::game::ui_ids::OWNER_MAIN_WORLD_SETTINGS_PANEL
                && *hud == crate::game::ui_ids::OWNER_MAIN_WORLD
        ));
        assert!(matches!(
            messages::<UiDocumentRuntimeCommand>(&app).as_slice(),
            [
                UiDocumentRuntimeCommand::CloseAllForOwner { owner: mail },
                UiDocumentRuntimeCommand::CloseAllForOwner { owner: settings },
                UiDocumentRuntimeCommand::CloseAllForOwner { owner: hud },
            ] if mail == "main_world_mail_panel"
                && settings == "main_world_settings_panel"
                && hud == "main_world"
        ));
        assert!(matches!(
            messages::<DeclarativeScreenHostCommand>(&app).as_slice(),
            [
                DeclarativeScreenHostCommand::CloseRoute { route: mail },
                DeclarativeScreenHostCommand::CloseRoute { route: settings },
                DeclarativeScreenHostCommand::CloseRoute { route: hud },
            ] if mail == "main_world_mail"
                && settings == "main_world_settings"
                && hud == "main_world"
        ));
        assert!(
            messages::<MyServerCommand>(&app)
                .iter()
                .any(|command| matches!(command, MyServerCommand::LeaveRoomScoped { .. }))
        );
        assert!(
            messages::<MyServerCommand>(&app)
                .iter()
                .all(|command| !matches!(command, MyServerCommand::Logout))
        );
        assert_eq!(app.world().resource::<MyServerSession>().ticket, ticket);
        assert_eq!(
            app.world().resource::<MyServerSession>().character_id,
            character_id
        );
        assert!(
            messages::<SceneCommand>(&app)
                .iter()
                .any(|command| matches!(
                    command,
                    SceneCommand::Exit(request)
                        if request.scene_id == Some(MAIN_WORLD_AUTHORITY_CONTRACT.client_scene())
                            && request.session_id.as_ref() == Some(&session_id)
                ))
        );

        app.world_mut().write_message(SceneEvent::Exited(
            crate::framework::scene::prelude::SceneExited {
                scene_id: MAIN_WORLD_AUTHORITY_CONTRACT.client_scene(),
                session_id,
            },
        ));
        app.update();

        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::Exiting
        );
        let leave_ack = room_leave_ack(&app);
        app.world_mut().write_message(leave_ack);
        app.update();

        let state = app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.phase, MainWorldEntryPhase::LobbyIdle);
        assert_eq!(state.last_departure, MainWorldRoomDeparture::Confirmed);
        assert!(!state.allows_gameplay_input());
    }

    #[test]
    fn active_escape_input_requests_a_single_room_leave() {
        let (mut app, _) = active_app();
        app.world_mut()
            .insert_resource(ButtonInput::<KeyCode>::default());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);

        app.update();
        app.update();

        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::Exiting
        );
        assert_eq!(
            messages::<MyServerCommand>(&app)
                .iter()
                .filter(|command| matches!(command, MyServerCommand::LeaveRoomScoped { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn floating_document_intercepts_escape_and_browser_back_before_main_world_exit() {
        enum ReturnInput {
            Escape,
            BrowserBack,
        }

        for input in [ReturnInput::Escape, ReturnInput::BrowserBack] {
            let (mut app, _) = active_app();
            app.world_mut().spawn(UiDocumentRuntimeRoot {
                request_id: UiDocumentRequestId(1),
                instance_id: UiDocumentInstanceId(1),
                generation: 1,
                document_id: UiDocumentId::from_str("game.main_world_mail").unwrap(),
                schema_version: 1,
                owner: "main_world_mail_panel".to_owned(),
                panel: UiDocumentPanel::Floating,
                layer: UiDocumentLayer::Floating,
                origin: UiDocumentSourceOrigin::Runtime {
                    producer: "test".to_owned(),
                },
            });
            match input {
                ReturnInput::Escape => {
                    app.world_mut()
                        .insert_resource(ButtonInput::<KeyCode>::default());
                    app.world_mut()
                        .resource_mut::<ButtonInput<KeyCode>>()
                        .press(KeyCode::Escape);
                }
                ReturnInput::BrowserBack => {
                    app.world_mut()
                        .insert_resource(ButtonInput::<Key>::default());
                    app.world_mut()
                        .resource_mut::<ButtonInput<Key>>()
                        .press(Key::BrowserBack);
                }
            }
            app.update();

            assert_eq!(
                app.world().resource::<MainWorldEntryState>().phase,
                MainWorldEntryPhase::Active
            );
            assert!(
                messages::<MyServerCommand>(&app)
                    .iter()
                    .all(|command| !matches!(command, MyServerCommand::LeaveRoomScoped { .. }))
            );
        }
    }

    #[test]
    fn exit_with_a_lost_leave_response_still_cleans_up_locally() {
        let (mut app, session_id) = active_app();
        app.world_mut()
            .write_message(MainWorldEntryIntent::ExitToLobby);
        app.update();
        app.world_mut().write_message(MyServerEvent::Disconnected {
            reason: Some("leave response lost".to_owned()),
        });
        app.world_mut().write_message(SceneEvent::Exited(
            crate::framework::scene::prelude::SceneExited {
                scene_id: MAIN_WORLD_AUTHORITY_CONTRACT.client_scene(),
                session_id,
            },
        ));
        app.update();

        let state = app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.phase, MainWorldEntryPhase::LobbyIdle);
        assert_eq!(state.last_departure, MainWorldRoomDeparture::Unknown);
    }

    #[test]
    fn disconnect_freezes_input_keeps_the_scene_and_requests_ticket_reconnect() {
        let (mut app, session_id) = active_app();
        app.world_mut().write_message(MyServerEvent::Disconnected {
            reason: Some("temporary transport loss".to_owned()),
        });
        app.update();

        let state = app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.phase, MainWorldEntryPhase::Recovering);
        assert!(state.input_frozen);
        assert_eq!(state.scene_session_id.as_ref(), Some(&session_id));
        assert_eq!(state.room_membership, MainWorldRoomMembership::Unknown);
        assert!(
            messages::<MyServerCommand>(&app)
                .iter()
                .any(|command| matches!(
                    command,
                    MyServerCommand::ReconnectWithTicketScoped { ticket, transport: NetworkTransport::Tcp, .. }
                        if ticket == "character-bound-ticket"
                ))
        );
    }

    #[test]
    fn disconnect_and_scene_failure_in_the_same_frame_fail_recovery() {
        use crate::framework::scene::prelude::{
            SceneFailure, SceneFailureKind, SceneLifecycleState,
        };

        let (mut app, session_id) = active_app();
        app.world_mut().write_message(MyServerEvent::Disconnected {
            reason: Some("transport lost while scene failed".to_owned()),
        });
        app.world_mut().write_message(SceneEvent::Failed(
            SceneFailure::new(
                SceneFailureKind::RequiredAssetMissing,
                SceneLifecycleState::LoadingAssets,
            )
            .with_scene(MAIN_WORLD_AUTHORITY_CONTRACT.client_scene())
            .with_session(session_id),
        ));
        app.update();

        let state = app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.phase, MainWorldEntryPhase::Failed);
        assert_eq!(state.failure, Some(MainWorldEntryFailure::SceneLoadFailed));
    }

    #[test]
    fn disconnect_and_content_failure_in_the_same_frame_fail_recovery() {
        let (mut app, session_id) = active_app();
        app.world_mut().write_message(MyServerEvent::Disconnected {
            reason: Some("transport lost while content failed".to_owned()),
        });
        app.world_mut()
            .write_message(MainWorldContentEvent::Failed {
                scene_id: MAIN_WORLD_AUTHORITY_CONTRACT.client_scene(),
                session_id,
                reason: "main world content instantiation failed".to_owned(),
            });
        app.update();

        let state = app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.phase, MainWorldEntryPhase::Failed);
        assert_eq!(state.failure, Some(MainWorldEntryFailure::SceneLoadFailed));
    }

    #[test]
    fn disconnect_and_scene_exit_in_the_same_frame_fail_recovery() {
        let (mut app, session_id) = active_app();
        app.world_mut().write_message(MyServerEvent::Disconnected {
            reason: Some("transport lost while scene exited".to_owned()),
        });
        app.world_mut().write_message(SceneEvent::Exited(
            crate::framework::scene::prelude::SceneExited {
                scene_id: MAIN_WORLD_AUTHORITY_CONTRACT.client_scene(),
                session_id,
            },
        ));
        app.update();

        let state = app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.phase, MainWorldEntryPhase::Failed);
        assert_eq!(state.failure, Some(MainWorldEntryFailure::SceneLoadFailed));
    }

    #[test]
    fn recovery_reuses_the_authority_kcp_transport_after_session_cleanup() {
        let (mut app, _) = active_app();
        app.world_mut()
            .resource_mut::<MainWorldEntryState>()
            .authority_transport = Some(NetworkTransport::Kcp);
        app.world_mut().write_message(MyServerEvent::Disconnected {
            reason: Some("KCP peer loss".to_owned()),
        });
        app.update();

        assert!(messages::<MyServerCommand>(&app).iter().any(|command| {
            matches!(
                command,
                MyServerCommand::ReconnectWithTicketScoped {
                    transport: NetworkTransport::Kcp,
                    ..
                }
            )
        }));
    }

    #[test]
    fn recovery_without_a_retained_transport_fails_without_tcp_fallback() {
        let (mut app, _) = active_app();
        app.world_mut()
            .resource_mut::<MainWorldEntryState>()
            .authority_transport = None;
        app.world_mut().write_message(MyServerEvent::Disconnected {
            reason: Some("missing transport".to_owned()),
        });
        app.update();

        let state = app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.phase, MainWorldEntryPhase::Failed);
        assert_eq!(
            state.failure,
            Some(MainWorldEntryFailure::ReconnectUnavailable)
        );
        assert!(
            !messages::<MyServerCommand>(&app)
                .iter()
                .any(|command| matches!(
                    command,
                    MyServerCommand::ReconnectWithTicketScoped { .. }
                ))
        );
    }

    #[test]
    fn recovery_keeps_scene_and_content_gates_until_a_fresh_snapshot_restores_active() {
        let (mut app, session_id) = active_app();
        app.world_mut().write_message(MyServerEvent::Disconnected {
            reason: Some("temporary transport loss".to_owned()),
        });
        app.update();
        let reconnect_attempt = app
            .world()
            .resource::<MainWorldEntryState>()
            .reconnect_attempt
            .unwrap();

        app.world_mut()
            .write_message(MyServerEvent::RoomReconnectedScoped {
                correlation: reconnect_attempt,
                seq: 3,
                connection_id: test_connection_id(),
                response: pb::RoomReconnectRes {
                    ok: true,
                    room_id: "main-world-public".to_owned(),
                    error_code: String::new(),
                    ..Default::default()
                },
            });
        app.update();
        let state = app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.phase, MainWorldEntryPhase::Recovering);
        assert!(state.scene_ready);
        assert!(state.scene_content_ready);
        assert_eq!(state.scene_session_id.as_ref(), Some(&session_id));

        app.world_mut()
            .write_message(movement_snapshot(10, 2005.0, 2006.0));
        app.update();
        let ready_attempt = app
            .world()
            .resource::<MainWorldEntryState>()
            .ready_attempt
            .unwrap();
        app.world_mut()
            .write_message(MyServerEvent::ReadyChangedScoped {
                correlation: ready_attempt,
                seq: 4,
                connection_id: test_connection_id(),
                response: pb::RoomReadyRes {
                    ok: true,
                    room_id: "main-world-public".to_owned(),
                    ready: true,
                    error_code: String::new(),
                },
            });
        app.update();
        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::Active
        );
    }

    #[test]
    fn redirect_ignores_old_snapshots_until_reconnect_then_restores_ready() {
        let (mut app, _) = active_app();
        app.world_mut()
            .write_message(MyServerEvent::ServerRedirectReconnectStarted {
                reason: "rollout".to_owned(),
                target_host: "new.game.test".to_owned(),
                target_port: 14400,
                transport: NetworkTransport::Tcp,
                correlation: 3,
            });
        app.world_mut()
            .write_message(movement_snapshot(99, 2009.0, 2009.0));
        app.update();
        assert_eq!(app.world().resource::<MainWorldEntryState>().position, None);

        let reconnected_connection_id = ConnectionId::from_raw(2);
        bind_reconnected_transport(&mut app, reconnected_connection_id);

        app.world_mut()
            .write_message(MyServerEvent::RoomReconnectedScoped {
                correlation: 3,
                seq: 3,
                connection_id: reconnected_connection_id,
                response: pb::RoomReconnectRes {
                    ok: true,
                    room_id: "main-world-public".to_owned(),
                    error_code: String::new(),
                    ..Default::default()
                },
            });
        app.world_mut()
            .write_message(movement_snapshot(10, 2005.0, 2006.0));
        app.update();
        assert_eq!(
            app.world().resource::<MainWorldEntryState>().position,
            Some(Vec3::new(5.0, 0.0, 6.0))
        );
        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::WaitingSceneReady
        );
        assert!(
            messages::<MyServerCommand>(&app)
                .iter()
                .filter(|command| matches!(
                    command,
                    MyServerCommand::SetReadyScoped { ready: true, .. }
                ))
                .count()
                >= 2
        );

        let ready_ack = room_ready_ack(&app);
        app.world_mut().write_message(ready_ack);
        app.update();
        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::Active
        );
        assert!(
            app.world()
                .resource::<MainWorldEntryState>()
                .allows_gameplay_input()
        );
    }

    #[test]
    fn reconnect_acknowledgement_requires_a_fresh_snapshot_before_resuming() {
        let (mut app, _) = active_app();
        app.world_mut()
            .write_message(MyServerEvent::ServerRedirectReconnectStarted {
                reason: "rollout".to_owned(),
                target_host: "new.game.test".to_owned(),
                target_port: 14400,
                transport: NetworkTransport::Tcp,
                correlation: 3,
            });
        app.update();
        let reconnected_connection_id = ConnectionId::from_raw(2);
        bind_reconnected_transport(&mut app, reconnected_connection_id);
        app.world_mut()
            .write_message(MyServerEvent::RoomReconnectedScoped {
                correlation: 3,
                seq: 3,
                connection_id: reconnected_connection_id,
                response: pb::RoomReconnectRes {
                    ok: true,
                    room_id: "main-world-public".to_owned(),
                    error_code: String::new(),
                    ..Default::default()
                },
            });
        app.update();

        let state = app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.phase, MainWorldEntryPhase::Recovering);
        assert!(state.reconnect_room_acknowledged);
        assert!(!state.recovery_snapshot_received);
        assert!(state.position.is_none());
        assert!(state.authoritative_scene_id.is_none());
    }

    #[test]
    fn reconnect_membership_expiry_rejoins_the_fixed_public_room() {
        let (mut app, session_id) = active_app();
        app.world_mut()
            .write_message(MyServerEvent::ServerRedirectReconnectStarted {
                reason: "rollout".to_owned(),
                target_host: "new.game.test".to_owned(),
                target_port: 14400,
                transport: NetworkTransport::Tcp,
                correlation: 3,
            });
        app.update();
        let reconnected_connection_id = ConnectionId::from_raw(2);
        bind_reconnected_transport(&mut app, reconnected_connection_id);
        app.world_mut()
            .write_message(MyServerEvent::RoomReconnectedScoped {
                correlation: 3,
                seq: 3,
                connection_id: reconnected_connection_id,
                response: pb::RoomReconnectRes {
                    ok: false,
                    room_id: "main-world-public".to_owned(),
                    error_code: "ROOM_MEMBER_EXPIRED".to_owned(),
                    ..Default::default()
                },
            });
        app.update();

        let state = app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.phase, MainWorldEntryPhase::JoiningRoom);
        assert_eq!(state.scene_session_id.as_ref(), Some(&session_id));
        assert!(
            messages::<MyServerCommand>(&app)
                .iter()
                .any(|command| matches!(
                    command,
                    MyServerCommand::JoinRoomScoped { room_id, policy_id, .. }
                        if room_id == "main-world-public" && policy_id == "movement_demo"
                ))
        );
    }

    #[test]
    fn fatal_account_event_cleans_up_the_scene_and_routes_to_login() {
        let (mut app, session_id) = active_app();
        app.world_mut().write_message(MyServerEvent::AccountBanned {
            message: "account banned".to_owned(),
            banned_until: None,
        });
        app.update();
        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::Exiting
        );
        assert!(
            messages::<MyServerCommand>(&app)
                .iter()
                .any(|command| matches!(command, MyServerCommand::Logout))
        );

        app.world_mut().write_message(SceneEvent::Exited(
            crate::framework::scene::prelude::SceneExited {
                scene_id: MAIN_WORLD_AUTHORITY_CONTRACT.client_scene(),
                session_id,
            },
        ));
        app.update();
        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::Exiting
        );
        let leave_ack = room_leave_ack(&app);
        app.world_mut().write_message(leave_ack);
        app.update();
        let route_messages = app.world().resource::<Messages<GameRouteCommand>>();
        let mut cursor = MessageCursor::default();
        assert!(
            cursor
                .read(route_messages)
                .any(|command| matches!(command, GameRouteCommand::ChangeMode(AppUiMode::Login)))
        );
    }

    #[test]
    fn home_transition_leaves_the_public_room_and_return_restarts_main_world_entry() {
        let (mut app, main_world_session) = active_app();
        app.world_mut()
            .write_message(MainWorldEntryIntent::EnterHome);
        app.update();
        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::Exiting
        );
        assert!(
            messages::<MyServerCommand>(&app)
                .iter()
                .any(|command| matches!(command, MyServerCommand::LeaveRoomScoped { .. }))
        );

        let leave_ack = room_leave_ack(&app);
        app.world_mut().write_message(leave_ack);
        app.world_mut().write_message(SceneEvent::Exited(
            crate::framework::scene::prelude::SceneExited {
                scene_id: MAIN_WORLD_AUTHORITY_CONTRACT.client_scene(),
                session_id: main_world_session,
            },
        ));
        app.update();
        let home_session = app
            .world()
            .resource::<MainWorldEntryState>()
            .home_session_id
            .clone()
            .unwrap();
        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::HomeLoading
        );

        app.world_mut().write_message(SceneEvent::Ready(
            crate::framework::scene::prelude::SceneReady {
                scene_id: FANGYUAN_HOME_SCENE_ID.into(),
                session_id: home_session.clone(),
                content_version: None,
                authority_mode: Default::default(),
                seed: None,
            },
        ));
        app.update();
        app.world_mut()
            .write_message(MainWorldEntryIntent::ReturnFromHome);
        app.update();
        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::ReturningFromHome
        );

        app.world_mut().write_message(SceneEvent::Exited(
            crate::framework::scene::prelude::SceneExited {
                scene_id: FANGYUAN_HOME_SCENE_ID.into(),
                session_id: home_session,
            },
        ));
        app.update();
        app.update();
        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::JoiningRoom
        );
    }

    #[test]
    fn repeated_home_and_return_intents_emit_one_scene_command_each() {
        let (mut app, main_world_session) = active_app();
        app.world_mut()
            .write_message(MainWorldEntryIntent::EnterHome);
        app.world_mut()
            .write_message(MainWorldEntryIntent::EnterHome);
        app.update();
        assert_eq!(
            messages::<SceneCommand>(&app)
                .iter()
                .filter(|command| matches!(
                    command,
                    SceneCommand::Exit(request)
                        if request.scene_id.as_ref() == Some(&MAIN_WORLD_AUTHORITY_CONTRACT.client_scene())
                ))
                .count(),
            1
        );

        let leave_ack = room_leave_ack(&app);
        app.world_mut().write_message(leave_ack);
        app.world_mut().write_message(SceneEvent::Exited(
            crate::framework::scene::prelude::SceneExited {
                scene_id: MAIN_WORLD_AUTHORITY_CONTRACT.client_scene(),
                session_id: main_world_session,
            },
        ));
        app.update();
        app.world_mut()
            .write_message(MainWorldEntryIntent::EnterHome);
        app.update();
        let home_session = app
            .world()
            .resource::<MainWorldEntryState>()
            .home_session_id
            .clone()
            .unwrap();
        assert_eq!(
            messages::<SceneCommand>(&app)
                .iter()
                .filter(|command| matches!(
                    command,
                    SceneCommand::Enter(request) if request.scene_id.as_str() == FANGYUAN_HOME_SCENE_ID
                ))
                .count(),
            1
        );

        app.world_mut().write_message(SceneEvent::Ready(
            crate::framework::scene::prelude::SceneReady {
                scene_id: FANGYUAN_HOME_SCENE_ID.into(),
                session_id: home_session.clone(),
                content_version: None,
                authority_mode: Default::default(),
                seed: None,
            },
        ));
        app.update();
        app.world_mut()
            .write_message(MainWorldEntryIntent::ReturnFromHome);
        app.world_mut()
            .write_message(MainWorldEntryIntent::ReturnFromHome);
        app.update();
        assert_eq!(
            messages::<SceneCommand>(&app)
                .iter()
                .filter(|command| matches!(
                    command,
                    SceneCommand::Exit(request)
                        if request.scene_id.as_ref().is_some_and(|scene_id| scene_id.as_str() == FANGYUAN_HOME_SCENE_ID)
                ))
                .count(),
            1
        );
    }

    #[test]
    fn disconnect_during_public_room_exit_still_enters_local_home() {
        let (mut app, main_world_session) = active_app();
        app.world_mut()
            .write_message(MainWorldEntryIntent::EnterHome);
        app.update();
        app.world_mut().write_message(MyServerEvent::Disconnected {
            reason: Some("transport closed during room leave".to_owned()),
        });
        app.world_mut().write_message(SceneEvent::Exited(
            crate::framework::scene::prelude::SceneExited {
                scene_id: MAIN_WORLD_AUTHORITY_CONTRACT.client_scene(),
                session_id: main_world_session,
            },
        ));
        app.update();

        let state = app.world().resource::<MainWorldEntryState>();
        assert_eq!(state.phase, MainWorldEntryPhase::HomeLoading);
        assert!(state.home_session_id.is_some());
        assert_eq!(state.last_departure, MainWorldRoomDeparture::Unknown);
        assert!(
            messages::<GameRouteCommand>(&app)
                .iter()
                .all(|command| !matches!(command, GameRouteCommand::ChangeMode(AppUiMode::Lobby)))
        );
    }

    #[test]
    fn home_load_failure_uses_the_lobby_fallback() {
        let mut app = ready_app();
        app.world_mut()
            .write_message(MainWorldEntryIntent::EnterHome);
        app.update();
        let home_session = app
            .world()
            .resource::<MainWorldEntryState>()
            .home_session_id
            .clone()
            .unwrap();
        app.world_mut().write_message(SceneEvent::Failed(
            crate::framework::scene::prelude::SceneFailure::new(
                crate::framework::scene::prelude::SceneFailureKind::RequiredAssetMissing,
                crate::framework::scene::prelude::SceneLifecycleState::LoadingAssets,
            )
            .with_scene(FANGYUAN_HOME_SCENE_ID)
            .with_session(home_session),
        ));
        app.update();
        assert_eq!(
            app.world().resource::<MainWorldEntryState>().phase,
            MainWorldEntryPhase::LobbyIdle
        );
    }
}
