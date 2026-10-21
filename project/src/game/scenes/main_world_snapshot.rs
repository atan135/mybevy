//! Main-world authority snapshot normalization.
//!
//! MyServer exposes room state, frame bundles, recovery snapshots, and live
//! movement snapshots as different wire messages.  This module gives the
//! main-world consumers one typed stream and one authority-frame epoch.

use bevy::prelude::*;

use crate::game::myserver::{MyServerEvent, MyServerUpdateSet, protocol::pb};

use super::{
    main_world_contract::{
        MAIN_WORLD_PUBLIC_ROOM_ID, MAIN_WORLD_ROOM_SNAPSHOT_REASON,
        main_world_movement_snapshot_from_room,
    },
    main_world_entry::MainWorldEntryUpdateSet,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum MainWorldSnapshotSource {
    RoomState,
    FrameBundle,
    Movement,
    Recovery,
}

#[derive(Clone, Debug, Message)]
pub(in crate::game) struct MainWorldSnapshotEvent {
    /// Monotonically increasing authority stream generation for this room.
    /// A server restart or a fresh room membership starts a new epoch, so a
    /// lower frame in a newer epoch is valid while an old epoch is not.
    pub epoch: u64,
    pub entry_generation: Option<u64>,
    pub source: MainWorldSnapshotSource,
    pub complete_room_entities: bool,
    pub push: pb::MovementSnapshotPush,
}

#[derive(Clone, Debug, Message)]
pub(in crate::game) struct MainWorldMovementRejectEvent {
    pub epoch: u64,
    pub entry_generation: Option<u64>,
    pub reject: pb::MovementRejectPush,
}

#[derive(Resource, Debug)]
pub(in crate::game) struct MainWorldSnapshotBusState {
    pub epoch: u64,
    /// Monotonic activity counter for every accepted main-world authority
    /// frame or snapshot. Consumers can detect a silent transport without
    /// reading the raw MyServer event stream again.
    pub authority_activity_serial: u64,
    room_id: Option<String>,
    entry_generation: Option<u64>,
    latest_frame: Option<u32>,
}

impl Default for MainWorldSnapshotBusState {
    fn default() -> Self {
        Self {
            epoch: 0,
            authority_activity_serial: 0,
            room_id: None,
            entry_generation: None,
            latest_frame: None,
        }
    }
}

impl MainWorldSnapshotBusState {
    fn ensure_room(&mut self, room_id: &str) {
        if self.room_id.as_deref() != Some(room_id) || self.epoch == 0 {
            self.start_new_epoch(room_id);
        }
    }

    fn start_new_epoch(&mut self, room_id: &str) {
        self.epoch = self.epoch.wrapping_add(1).max(1);
        self.room_id = Some(room_id.to_owned());
        self.latest_frame = None;
    }

    fn observe_snapshot(&mut self, room_id: &str, frame_id: u32, resets_epoch: bool) {
        self.ensure_room(room_id);
        if resets_epoch && self.latest_frame.is_some_and(|latest| frame_id < latest) {
            self.start_new_epoch(room_id);
        }
        self.authority_activity_serial = self.authority_activity_serial.wrapping_add(1).max(1);
        self.latest_frame = Some(
            self.latest_frame
                .map_or(frame_id, |latest| latest.max(frame_id)),
        );
    }
}

/// Installs the bus once.  The three main-world plugins call this helper so
/// their focused unit-test apps receive the same normalization path as the
/// full game plugin.
pub(in crate::game) fn install_main_world_snapshot_bus(app: &mut App) {
    if app.world().contains_resource::<MainWorldSnapshotBusState>() {
        return;
    }
    app.init_resource::<MainWorldSnapshotBusState>()
        .add_message::<MainWorldSnapshotEvent>()
        .add_message::<MainWorldMovementRejectEvent>()
        .configure_sets(
            Update,
            MainWorldSnapshotBusUpdateSet::Publish
                .after(MyServerUpdateSet::NetworkEvents)
                .before(MainWorldEntryUpdateSet::Coordinator),
        )
        .add_systems(
            Update,
            publish_main_world_snapshot_events.in_set(MainWorldSnapshotBusUpdateSet::Publish),
        );
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, SystemSet)]
pub(in crate::game) enum MainWorldSnapshotBusUpdateSet {
    Publish,
}

fn publish_main_world_snapshot_events(
    mut events: MessageReader<MyServerEvent>,
    mut state: ResMut<MainWorldSnapshotBusState>,
    mut snapshots: MessageWriter<MainWorldSnapshotEvent>,
    mut rejects: MessageWriter<MainWorldMovementRejectEvent>,
    entry: Option<Res<super::main_world_entry::MainWorldEntryState>>,
    session: Option<Res<crate::game::myserver::MyServerSession>>,
) {
    let generation = entry.as_ref().map(|entry| entry.generation);
    if state.entry_generation != generation {
        state.entry_generation = generation;
        state.start_new_epoch(MAIN_WORLD_PUBLIC_ROOM_ID);
    }
    let mut admission_open = entry.as_ref().is_none_or(|entry| {
        use super::main_world_entry::MainWorldEntryPhase;
        matches!(
            entry.phase,
            MainWorldEntryPhase::Active
                | MainWorldEntryPhase::LoadingScene
                | MainWorldEntryPhase::WaitingSceneReady
        ) || (entry.phase == MainWorldEntryPhase::JoiningRoom && entry.join_acknowledged)
            || (entry.phase == MainWorldEntryPhase::Recovering && entry.reconnect_room_acknowledged)
    });
    for event in events.read() {
        // Scoped request responses may outlive an entry attempt even on the same connection.
        if let Some(entry) = entry.as_ref() {
            let scoped = match event {
                MyServerEvent::RoomJoinedScoped {
                    correlation,
                    connection_id,
                    ..
                } => Some((entry.join_attempt, *correlation, *connection_id)),
                MyServerEvent::RoomReconnectedScoped {
                    correlation,
                    connection_id,
                    ..
                } => Some((entry.reconnect_attempt, *correlation, *connection_id)),
                _ => None,
            };
            if let Some((expected, correlation, connection_id)) = scoped {
                if expected != Some(correlation)
                    || session.as_ref().is_none_or(|session| {
                        !entry.accepts_scoped_authority_event(session, connection_id)
                    })
                {
                    continue;
                }
            }
        }

        if !admission_open
            && matches!(
                event,
                MyServerEvent::RoomStatePush(_)
                    | MyServerEvent::FrameBundlePush(_)
                    | MyServerEvent::MovementSnapshotPush(_)
                    | MyServerEvent::MovementRejectPush(_)
            )
        {
            continue;
        }
        match event {
            MyServerEvent::RoomJoinedScoped { response, .. }
                if response.ok && response.room_id == MAIN_WORLD_PUBLIC_ROOM_ID =>
            {
                admission_open = true;
                state.start_new_epoch(&response.room_id);
            }
            MyServerEvent::RoomReconnectedScoped { response, .. }
                if response.ok && response.room_id == MAIN_WORLD_PUBLIC_ROOM_ID =>
            {
                admission_open = true;
                state.start_new_epoch(&response.room_id);
                if let Some(recovery) = response.movement_recovery.as_ref() {
                    publish_snapshot(
                        &mut state,
                        &mut snapshots,
                        MainWorldSnapshotSource::Recovery,
                        pb::MovementSnapshotPush {
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
                    );
                }
            }
            MyServerEvent::RoomStatePush(push) => {
                let Some(room_snapshot) = push.snapshot.as_ref() else {
                    continue;
                };
                if room_snapshot.room_id != MAIN_WORLD_PUBLIC_ROOM_ID
                    || room_snapshot.state != "in_game"
                {
                    continue;
                }
                let Some(snapshot) = main_world_movement_snapshot_from_room(
                    room_snapshot,
                    room_snapshot.current_frame_id,
                ) else {
                    continue;
                };
                publish_snapshot(
                    &mut state,
                    &mut snapshots,
                    MainWorldSnapshotSource::RoomState,
                    snapshot,
                );
            }
            MyServerEvent::FrameBundlePush(push) => {
                if push.room_id == MAIN_WORLD_PUBLIC_ROOM_ID && push.snapshot.is_none() {
                    // Frame-only bundles still advance the room watermark so
                    // a later complete snapshot can identify a restart.
                    state.observe_snapshot(&push.room_id, push.frame_id, false);
                    continue;
                }
                let Some(room_snapshot) = push.snapshot.as_ref() else {
                    continue;
                };
                if push.room_id != MAIN_WORLD_PUBLIC_ROOM_ID
                    || room_snapshot.room_id != MAIN_WORLD_PUBLIC_ROOM_ID
                {
                    continue;
                }
                let Some(snapshot) =
                    main_world_movement_snapshot_from_room(room_snapshot, push.frame_id)
                else {
                    continue;
                };
                publish_snapshot(
                    &mut state,
                    &mut snapshots,
                    MainWorldSnapshotSource::FrameBundle,
                    snapshot,
                );
            }
            MyServerEvent::MovementRejectPush(reject)
                if reject.room_id == MAIN_WORLD_PUBLIC_ROOM_ID =>
            {
                rejects.write(MainWorldMovementRejectEvent {
                    epoch: state.epoch,
                    entry_generation: state.entry_generation,
                    reject: reject.clone(),
                });
            }
            MyServerEvent::MovementSnapshotPush(push)
                if push.room_id == MAIN_WORLD_PUBLIC_ROOM_ID =>
            {
                publish_snapshot(
                    &mut state,
                    &mut snapshots,
                    MainWorldSnapshotSource::Movement,
                    push.clone(),
                );
            }
            _ => {}
        }
    }
}

fn publish_snapshot(
    state: &mut MainWorldSnapshotBusState,
    snapshots: &mut MessageWriter<MainWorldSnapshotEvent>,
    source: MainWorldSnapshotSource,
    push: pb::MovementSnapshotPush,
) {
    let complete_room_entities = matches!(
        source,
        MainWorldSnapshotSource::RoomState | MainWorldSnapshotSource::FrameBundle
    ) || (push.full_sync && push.target_character_ids.is_empty())
        || push.reason == MAIN_WORLD_ROOM_SNAPSHOT_REASON;
    // Interpret the legacy wire marker only at this adapter boundary. The
    // consumers use the typed completeness flag instead of reason text.
    let epoch_reset_candidate = matches!(
        source,
        MainWorldSnapshotSource::RoomState
            | MainWorldSnapshotSource::FrameBundle
            | MainWorldSnapshotSource::Recovery
    ) || push.reason == MAIN_WORLD_ROOM_SNAPSHOT_REASON;
    let recovery = matches!(source, MainWorldSnapshotSource::Recovery)
        || matches!(
            pb::MovementCorrectionKind::try_from(push.correction_kind).ok(),
            Some(pb::MovementCorrectionKind::Recovery)
        );
    // Recovery is also allowed to establish a new stream, but only when its
    // frame falls behind the current watermark. A same-window recovery is a
    // normal correction and must remain in the current epoch.
    state.observe_snapshot(
        &push.room_id,
        push.frame_id,
        epoch_reset_candidate || recovery,
    );
    snapshots.write(MainWorldSnapshotEvent {
        epoch: state.epoch,
        entry_generation: state.entry_generation,
        source,
        complete_room_entities,
        push,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delayed_scoped_recovery_cannot_publish_into_current_entry() {
        use crate::framework::network::ConnectionId;
        use crate::game::myserver::MyServerSession;
        use crate::game::scenes::main_world_entry::MainWorldEntryState;
        let connection = ConnectionId::from_raw(12);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<MyServerEvent>()
            .insert_resource(MainWorldEntryState {
                generation: 8,
                character_id: Some("current".into()),
                authority_character_id: Some("current".into()),
                authority_connection_id: Some(connection),
                reconnect_attempt: Some(42),
                ..Default::default()
            })
            .insert_resource(MyServerSession {
                character_id: Some("current".into()),
                connection_id: Some(connection),
                ..Default::default()
            });
        install_main_world_snapshot_bus(&mut app);
        app.update();
        let epoch = app.world().resource::<MainWorldSnapshotBusState>().epoch;
        for (correlation, connection_id) in [(41, connection), (42, ConnectionId::from_raw(11))] {
            app.world_mut()
                .write_message(MyServerEvent::RoomReconnectedScoped {
                    correlation,
                    seq: 1,
                    connection_id,
                    response: pb::RoomReconnectRes {
                        ok: true,
                        room_id: MAIN_WORLD_PUBLIC_ROOM_ID.into(),
                        movement_recovery: Some(pb::MovementRecoveryState {
                            frame_id: 90,
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                });
        }
        app.update();
        let state = app.world().resource::<MainWorldSnapshotBusState>();
        assert_eq!(state.epoch, epoch);
        assert_eq!(state.authority_activity_serial, 0);
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(
                pb::MovementSnapshotPush {
                    room_id: MAIN_WORLD_PUBLIC_ROOM_ID.into(),
                    frame_id: 100,
                    full_sync: true,
                    ..Default::default()
                },
            ));
        app.world_mut()
            .write_message(MyServerEvent::RoomReconnectedScoped {
                correlation: 42,
                seq: 2,
                connection_id: connection,
                response: pb::RoomReconnectRes {
                    ok: true,
                    room_id: MAIN_WORLD_PUBLIC_ROOM_ID.into(),
                    movement_recovery: Some(pb::MovementRecoveryState {
                        frame_id: 91,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            });
        app.update();
        assert_eq!(
            app.world()
                .resource::<MainWorldSnapshotBusState>()
                .authority_activity_serial,
            1
        );
    }

    #[test]
    fn complete_lower_frame_starts_a_new_epoch() {
        let mut state = MainWorldSnapshotBusState::default();
        state.observe_snapshot(MAIN_WORLD_PUBLIC_ROOM_ID, 100, true);
        let first = state.epoch;
        state.observe_snapshot(MAIN_WORLD_PUBLIC_ROOM_ID, 3, true);
        assert_eq!(state.epoch, first + 1);
        assert_eq!(state.latest_frame, Some(3));
    }

    #[test]
    fn incremental_lower_frame_stays_in_the_same_epoch() {
        let mut state = MainWorldSnapshotBusState::default();
        state.observe_snapshot(MAIN_WORLD_PUBLIC_ROOM_ID, 100, true);
        let epoch = state.epoch;
        state.observe_snapshot(MAIN_WORLD_PUBLIC_ROOM_ID, 99, false);
        assert_eq!(state.epoch, epoch);
        assert_eq!(state.latest_frame, Some(100));
    }
}
