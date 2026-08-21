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
    pub source: MainWorldSnapshotSource,
    pub complete_room_entities: bool,
    pub push: pb::MovementSnapshotPush,
}

#[derive(Resource, Debug)]
pub(in crate::game) struct MainWorldSnapshotBusState {
    pub epoch: u64,
    room_id: Option<String>,
    latest_frame: Option<u32>,
}

impl Default for MainWorldSnapshotBusState {
    fn default() -> Self {
        Self {
            epoch: 0,
            room_id: None,
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
) {
    for event in events.read() {
        match event {
            MyServerEvent::RoomJoinedScoped { response, .. }
                if response.ok && response.room_id == MAIN_WORLD_PUBLIC_ROOM_ID =>
            {
                state.start_new_epoch(&response.room_id);
            }
            MyServerEvent::RoomReconnectedScoped { response, .. }
                if response.ok && response.room_id == MAIN_WORLD_PUBLIC_ROOM_ID =>
            {
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
        source,
        complete_room_entities,
        push,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

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
