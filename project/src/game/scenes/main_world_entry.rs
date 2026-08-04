//! Generation-bound entry intent coordinator for the fixed public main world.
//!
//! This coordinator owns request generations and authority admission. Scene
//! loading and RoomReady remain separate later-stage responsibilities.

use bevy::prelude::*;

use crate::{
    framework::scene::prelude::{SceneEvent, SceneRegistry},
    game::{
        myserver::{
            AccountLoginState, CharacterSelectionState, GameConnectionState, MyServerCommand,
            MyServerEnvironment, MyServerEvent, MyServerProfiles, MyServerSession,
        },
        scenes::main_world_contract::MAIN_WORLD_AUTHORITY_CONTRACT,
    },
};

pub(in crate::game) struct MainWorldEntryPlugin;

impl Plugin for MainWorldEntryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MainWorldEntryState>()
            .add_message::<MainWorldEntryIntent>()
            .add_message::<MainWorldEntrySignal>()
            .add_message::<MainWorldEntryEvent>()
            .add_systems(
                Update,
                (
                    abort_invalidated_entry,
                    handle_entry_intents,
                    dispatch_main_world_join_requests,
                    consume_main_world_authority_events,
                    ignore_unbound_scene_events,
                    handle_entry_signals,
                )
                    .chain(),
            );
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
    Failed,
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
        )
    }
}

#[derive(Clone, Debug, Resource)]
pub(in crate::game) struct MainWorldEntryState {
    pub generation: u64,
    pub phase: MainWorldEntryPhase,
    pub environment: Option<MyServerEnvironment>,
    pub character_id: Option<String>,
    pub room_id: Option<String>,
    pub policy_id: Option<String>,
    pub authoritative_scene_id: Option<i32>,
    pub position: Option<Vec3>,
    pub snapshot_generation: u32,
    pub join_acknowledged: bool,
    pub failure: Option<MainWorldEntryFailure>,
}

impl Default for MainWorldEntryState {
    fn default() -> Self {
        Self {
            generation: 0,
            phase: MainWorldEntryPhase::LobbyIdle,
            environment: None,
            character_id: None,
            room_id: None,
            policy_id: None,
            authoritative_scene_id: None,
            position: None,
            snapshot_generation: 0,
            join_acknowledged: false,
            failure: None,
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

    fn begin(&mut self, environment: MyServerEnvironment, character_id: String) {
        self.generation = self.generation.wrapping_add(1).max(1);
        self.phase = MainWorldEntryPhase::Validating;
        self.environment = Some(environment);
        self.character_id = Some(character_id);
        self.failure = None;
        self.room_id = None;
        self.policy_id = None;
        self.authoritative_scene_id = None;
        self.position = None;
        self.snapshot_generation = 0;
        self.join_acknowledged = false;
    }

    fn reset(&mut self) {
        self.phase = MainWorldEntryPhase::LobbyIdle;
        self.environment = None;
        self.character_id = None;
        self.failure = None;
        self.room_id = None;
        self.policy_id = None;
        self.authoritative_scene_id = None;
        self.position = None;
        self.snapshot_generation = 0;
        self.join_acknowledged = false;
    }

    fn fail(&mut self, failure: MainWorldEntryFailure) {
        self.phase = MainWorldEntryPhase::Failed;
        self.environment = None;
        self.character_id = None;
        self.failure = Some(failure);
    }
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
}

fn handle_entry_intents(
    mut intents: MessageReader<MainWorldEntryIntent>,
    profiles: Res<MyServerProfiles>,
    session: Res<MyServerSession>,
    registry: Res<SceneRegistry>,
    mut state: ResMut<MainWorldEntryState>,
    mut events: MessageWriter<MainWorldEntryEvent>,
) {
    let mut accepted_enter = false;
    for intent in intents.read() {
        if matches!(intent, MainWorldEntryIntent::Enter) {
            if accepted_enter || state.is_in_flight() {
                continue;
            }
            accepted_enter = true;
            state.begin(
                profiles.selected(),
                session.character_id.clone().unwrap_or_default(),
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
            events.write(MainWorldEntryEvent::JoinRequested {
                generation: state.generation,
                room_id: MAIN_WORLD_AUTHORITY_CONTRACT.room_id,
                policy_id: MAIN_WORLD_AUTHORITY_CONTRACT.policy_id,
                character_id: state.character_id.clone().unwrap_or_default(),
            });
            continue;
        }
        abort_for_intent(intent, &mut state, &mut events);
    }
}

fn abort_invalidated_entry(
    profiles: Res<MyServerProfiles>,
    session: Res<MyServerSession>,
    mut state: ResMut<MainWorldEntryState>,
    mut events: MessageWriter<MainWorldEntryEvent>,
) {
    if state.is_in_flight()
        && (state.environment != Some(profiles.selected())
            || state.character_id.as_deref() != session.character_id.as_deref()
            || !matches!(session.account_login_state, AccountLoginState::LoggedIn))
    {
        abort_entry(
            &mut state,
            &mut events,
            MainWorldEntryAbortReason::PreconditionsInvalidated,
        );
    }
}

fn dispatch_main_world_join_requests(
    mut entry_events: MessageReader<MainWorldEntryEvent>,
    mut commands: MessageWriter<MyServerCommand>,
) {
    for event in entry_events.read() {
        let MainWorldEntryEvent::JoinRequested {
            room_id, policy_id, ..
        } = event
        else {
            continue;
        };
        commands.write(MyServerCommand::JoinRoom {
            room_id: (*room_id).to_owned(),
            policy_id: (*policy_id).to_owned(),
        });
    }
}

fn consume_main_world_authority_events(
    mut myserver_events: MessageReader<MyServerEvent>,
    mut state: ResMut<MainWorldEntryState>,
    mut entry_events: MessageWriter<MainWorldEntryEvent>,
) {
    if !state.is_in_flight() {
        return;
    }
    let character_id = state.character_id.clone().unwrap_or_default();
    for event in myserver_events.read() {
        match event {
            MyServerEvent::RoomJoined(response)
                if response.room_id == MAIN_WORLD_AUTHORITY_CONTRACT.room_id =>
            {
                if response.ok {
                    state.join_acknowledged = true;
                    state.room_id = Some(response.room_id.clone());
                    state.policy_id = Some(MAIN_WORLD_AUTHORITY_CONTRACT.policy_id.to_owned());
                } else {
                    fail_authority_entry(
                        &mut state,
                        &mut entry_events,
                        room_join_failure(&response.error_code),
                    );
                }
            }
            MyServerEvent::RoomStatePush(push) => {
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
                        &mut entry_events,
                        MainWorldEntryFailure::AuthoritativeSceneMismatch,
                    );
                } else {
                    state.room_id = Some(snapshot.room_id.clone());
                    state.policy_id = Some(MAIN_WORLD_AUTHORITY_CONTRACT.policy_id.to_owned());
                    state.snapshot_generation =
                        state.snapshot_generation.max(snapshot.current_frame_id);
                }
            }
            MyServerEvent::MovementSnapshotPush(push)
                if push.room_id == MAIN_WORLD_AUTHORITY_CONTRACT.room_id =>
            {
                if push.frame_id < state.snapshot_generation {
                    continue;
                }
                let Some(entity) = push
                    .entities
                    .iter()
                    .find(|entity| entity.character_id == character_id)
                else {
                    continue;
                };
                if !MAIN_WORLD_AUTHORITY_CONTRACT.is_authoritative_entity_scene(entity.scene_id) {
                    fail_authority_entry(
                        &mut state,
                        &mut entry_events,
                        MainWorldEntryFailure::AuthoritativeSceneMismatch,
                    );
                    continue;
                }
                let Ok(position) = main_world_bevy_position(entity.x, entity.y) else {
                    fail_authority_entry(
                        &mut state,
                        &mut entry_events,
                        MainWorldEntryFailure::InvalidAuthoritativePosition,
                    );
                    continue;
                };
                state.room_id = Some(push.room_id.clone());
                state.policy_id = Some(MAIN_WORLD_AUTHORITY_CONTRACT.policy_id.to_owned());
                state.authoritative_scene_id = Some(entity.scene_id);
                state.position = Some(position);
                state.snapshot_generation = push.frame_id;
                state.phase = MainWorldEntryPhase::LoadingScene;
            }
            _ => {}
        }
    }
}

fn fail_authority_entry(
    state: &mut MainWorldEntryState,
    events: &mut MessageWriter<MainWorldEntryEvent>,
    failure: MainWorldEntryFailure,
) {
    let generation = state.generation;
    state.fail(failure);
    events.write(MainWorldEntryEvent::Failed {
        generation,
        failure,
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

pub(in crate::game) fn main_world_bevy_position(
    x: f32,
    y: f32,
) -> Result<Vec3, MainWorldEntryFailure> {
    const MIN: f32 = 0.0;
    const MAX: f32 = 16.0;
    if !x.is_finite() || !y.is_finite() || !(MIN..=MAX).contains(&x) || !(MIN..=MAX).contains(&y) {
        return Err(MainWorldEntryFailure::InvalidAuthoritativePosition);
    }
    Ok(Vec3::new(x, 0.0, y))
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

// No SceneCommand is issued in this stage. Reading the stream makes the rule
// explicit: unbound/old SceneEvent values cannot transition this coordinator.
fn ignore_unbound_scene_events(mut scene_events: MessageReader<SceneEvent>) {
    for _ in scene_events.read() {}
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
        MainWorldEntryIntent::Enter | MainWorldEntryIntent::Recover => return,
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
    if !session.authenticated || session.game_connection_state != GameConnectionState::Authenticated
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
    use crate::framework::scene::prelude::{SceneDefinition, SceneKind};
    use bevy::ecs::message::{MessageCursor, Messages};

    fn ready_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<MyServerProfiles>()
            .init_resource::<MyServerSession>()
            .init_resource::<SceneRegistry>()
            .add_message::<SceneEvent>()
            .add_message::<MyServerCommand>()
            .add_message::<MyServerEvent>()
            .add_plugins(MainWorldEntryPlugin);
        let session = &mut *app.world_mut().resource_mut::<MyServerSession>();
        session.account_login_state = AccountLoginState::LoggedIn;
        session.character_selection_state = CharacterSelectionState::Selected;
        session.character_id = Some("chr_1".to_owned());
        session.ticket = Some("character-bound-ticket".to_owned());
        session.authenticated = true;
        session.game_connection_state = GameConnectionState::Authenticated;
        app.world_mut()
            .resource_mut::<SceneRegistry>()
            .register(SceneDefinition::new(
                MAIN_WORLD_AUTHORITY_CONTRACT.client_scene(),
                SceneKind::World,
            ))
            .unwrap();
        app
    }

    fn events(app: &App) -> Vec<MainWorldEntryEvent> {
        let messages = app.world().resource::<Messages<MainWorldEntryEvent>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
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
                room_id: "main-world-public",
                policy_id: "movement_demo",
                character_id,
            }] if character_id == "chr_1"
        ));
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
            main_world_bevy_position(2.0, 3.0),
            Ok(Vec3::new(2.0, 0.0, 3.0))
        );
        assert_eq!(
            main_world_bevy_position(f32::NAN, 1.0),
            Err(MainWorldEntryFailure::InvalidAuthoritativePosition)
        );
        assert_eq!(
            main_world_bevy_position(16.1, 1.0),
            Err(MainWorldEntryFailure::InvalidAuthoritativePosition)
        );
    }
}
