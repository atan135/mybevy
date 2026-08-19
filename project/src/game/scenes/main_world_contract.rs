//! Stable client-to-MyServer identifiers for the first main-world entry flow.
//!
//! The scene framework owns the client scene session and its presentation
//! lifetime. MyServer owns the ticket-bound character, its authoritative scene
//! id, and movement state. This module is deliberately limited to that mapping;
//! it does not implement room entry or scene loading.

use crate::framework::scene::prelude::SceneId;
use bevy::prelude::{Vec2, Vec3};
use serde::Deserialize;
use serde_json::Value;
use std::borrow::Cow;
use std::fmt;

/// Product-facing logical identifier for the fixed public main world.
pub(in crate::game) const MAIN_WORLD_LOGICAL_ID: &str = "main_world";

/// Client scene-framework ID rendered for the fixed public main world.
pub(in crate::game) const MAIN_WORLD_CLIENT_SCENE_ID: &str = "world.main";

/// MyServer `SceneTable.Code` for the first main-world authority scene.
pub(in crate::game) const MAIN_WORLD_SERVER_SCENE_CODE: &str = "grassland_01";

/// MyServer `SceneTable.Id` for [`MAIN_WORLD_SERVER_SCENE_CODE`].
pub(in crate::game) const MAIN_WORLD_SERVER_SCENE_ID: i32 = 1;

/// MyServer `SceneSpawnPoint.Id` used by `movement_demo` for a first entry.
pub(in crate::game) const MAIN_WORLD_SERVER_DEFAULT_SPAWN_ID: i32 = 1001;

/// Fixed MyServer room that represents the public main city.
pub(in crate::game) const MAIN_WORLD_PUBLIC_ROOM_ID: &str = "main-world-public";

/// Existing MyServer room policy used by the public main city.
pub(in crate::game) const MAIN_WORLD_ROOM_POLICY_ID: &str = "movement_demo";

/// The authoritative world is a non-negative, 4000 metre square. Its upper
/// edge is exclusive so the client and server share exactly one boundary rule.
pub(in crate::game) const MAIN_WORLD_SERVER_COORDINATE_MIN_METRES: f32 = 0.0;
pub(in crate::game) const MAIN_WORLD_SERVER_COORDINATE_MAX_EXCLUSIVE_METRES: f32 = 4000.0;
pub(in crate::game) const MAIN_WORLD_WORLD_CENTRE_OFFSET_METRES: f32 = 2000.0;

/// Shared movement simulation contract. Future movement systems must advance
/// prediction only at this cadence, rather than at the render cadence.
pub(in crate::game) const MAIN_WORLD_AUTHORITY_TICKS_PER_SECOND: u32 = 20;
pub(in crate::game) const MAIN_WORLD_AUTHORITY_TICK_SECONDS: f32 =
    1.0 / MAIN_WORLD_AUTHORITY_TICKS_PER_SECOND as f32;
pub(in crate::game) const MAIN_WORLD_MOVE_SPEED_METRES_PER_SECOND: f32 = 4.0;

/// `MOVE_DIR` is sent once per authority frame while movement is held. This
/// keeps the existing three-frame server-side stop watchdog alive. A
/// transition to idle emits exactly one `MOVE_STOP`; it is not a render-frame
/// keepalive message.
pub(in crate::game) const MAIN_WORLD_MOVE_DIR_KEEPALIVE_FRAMES: u32 = 1;
pub(in crate::game) const MAIN_WORLD_SERVER_CONTROL_STOP_FRAMES: u32 = 3;
pub(in crate::game) const MAIN_WORLD_MOVE_STOP_MESSAGES_PER_TRANSITION: u32 = 1;

/// Out-of-scope systems for the first main-world authority loop.
pub(in crate::game) const MAIN_WORLD_NON_GOALS: &[&str] = &[
    "matchmaking",
    "dynamic_sharding",
    "cross_region_transfer",
    "production_player_model",
    "combat",
];

/// A frame produced by the 20 Hz MyServer movement authority. This is the
/// only frame kind accepted from `MovementSnapshotPush` and used by
/// `MoveInputReq.frame_id`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(in crate::game) struct MainWorldAuthorityFrame(pub u32);

/// A local fixed-step simulation frame. It can run ahead of the latest
/// authority frame only through unconfirmed local input replay.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(in crate::game) struct MainWorldPredictedFrame(pub u32);

/// The newest local input frame explicitly acknowledged by an authoritative
/// entity snapshot (`EntityTransform.last_input_frame`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(in crate::game) struct MainWorldConfirmedFrame(pub u32);

/// A presentation-only frame. It must never be sent to the server or used as
/// a prediction-history key, because rendering can run at any cadence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(in crate::game) struct MainWorldRenderFrame(pub u64);

/// The two move input semantics shared by all later input adapters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum MainWorldMoveInputKind {
    /// A finite, normalized direction is sent every authority frame while held.
    MoveDirection,
    /// Exactly one message is sent on a moving-to-idle transition.
    MoveStop,
}

/// Errors returned while translating values at the client/server coordinate
/// boundary. Bevy Y is deliberately absent: server positions are planar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum MainWorldCoordinateError {
    UnexpectedScene { expected: i32, actual: i32 },
    NonFiniteServerPosition,
    ServerPositionOutOfBounds,
    NonFiniteBevyPosition,
    BevyPositionOutOfBounds,
    NonFiniteDirection,
    ZeroDirection,
}

impl fmt::Display for MainWorldCoordinateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedScene { expected, actual } => {
                write!(
                    formatter,
                    "main-world scene mismatch: expected {expected}, got {actual}"
                )
            }
            Self::NonFiniteServerPosition => {
                formatter.write_str("main-world server position is not finite")
            }
            Self::ServerPositionOutOfBounds => {
                formatter.write_str("main-world server position is outside [0, 4000) metres")
            }
            Self::NonFiniteBevyPosition => {
                formatter.write_str("main-world Bevy X/Z position is not finite")
            }
            Self::BevyPositionOutOfBounds => {
                formatter.write_str("main-world Bevy X/Z position maps outside [0, 4000) metres")
            }
            Self::NonFiniteDirection => formatter.write_str("main-world direction is not finite"),
            Self::ZeroDirection => formatter.write_str("main-world direction must be non-zero"),
        }
    }
}

impl std::error::Error for MainWorldCoordinateError {}

/// Returns whether a server coordinate is finite and inside its exclusive
/// 4000-metre world boundary.
pub(in crate::game) fn is_main_world_server_coordinate(value: f32) -> bool {
    value.is_finite()
        && (MAIN_WORLD_SERVER_COORDINATE_MIN_METRES
            ..MAIN_WORLD_SERVER_COORDINATE_MAX_EXCLUSIVE_METRES)
            .contains(&value)
}

/// Converts an authoritative MyServer ground-plane position to the centred
/// Bevy X/Z plane. This is the sole position mapping for the main world.
pub(in crate::game) fn main_world_bevy_position(
    server_x: f32,
    server_y: f32,
) -> Result<Vec3, MainWorldCoordinateError> {
    if !server_x.is_finite() || !server_y.is_finite() {
        return Err(MainWorldCoordinateError::NonFiniteServerPosition);
    }
    if !is_main_world_server_coordinate(server_x) || !is_main_world_server_coordinate(server_y) {
        return Err(MainWorldCoordinateError::ServerPositionOutOfBounds);
    }
    Ok(Vec3::new(
        server_x - MAIN_WORLD_WORLD_CENTRE_OFFSET_METRES,
        0.0,
        server_y - MAIN_WORLD_WORLD_CENTRE_OFFSET_METRES,
    ))
}

/// Converts a centred Bevy X/Z ground-plane position back to the server's
/// non-negative coordinate system. Bevy Y is a visual height and is ignored.
pub(in crate::game) fn main_world_server_position(
    bevy_position: Vec3,
) -> Result<Vec2, MainWorldCoordinateError> {
    if !bevy_position.x.is_finite() || !bevy_position.z.is_finite() {
        return Err(MainWorldCoordinateError::NonFiniteBevyPosition);
    }
    let server = Vec2::new(
        bevy_position.x + MAIN_WORLD_WORLD_CENTRE_OFFSET_METRES,
        bevy_position.z + MAIN_WORLD_WORLD_CENTRE_OFFSET_METRES,
    );
    if !is_main_world_server_coordinate(server.x) || !is_main_world_server_coordinate(server.y) {
        return Err(MainWorldCoordinateError::BevyPositionOutOfBounds);
    }
    Ok(server)
}

/// Validates a movement snapshot's scene before accepting its position.
pub(in crate::game) fn main_world_bevy_position_from_authority(
    scene_id: i32,
    server_x: f32,
    server_y: f32,
) -> Result<Vec3, MainWorldCoordinateError> {
    if !MAIN_WORLD_AUTHORITY_CONTRACT.is_authoritative_entity_scene(scene_id) {
        return Err(MainWorldCoordinateError::UnexpectedScene {
            expected: MAIN_WORLD_SERVER_SCENE_ID,
            actual: scene_id,
        });
    }
    main_world_bevy_position(server_x, server_y)
}

/// Maps a finite server XY direction to Bevy XZ without applying the world
/// centre offset. A zero vector is valid for an authoritative stopped state.
pub(in crate::game) fn main_world_bevy_direction(
    server_dir_x: f32,
    server_dir_y: f32,
) -> Result<Vec3, MainWorldCoordinateError> {
    if !server_dir_x.is_finite() || !server_dir_y.is_finite() {
        return Err(MainWorldCoordinateError::NonFiniteDirection);
    }
    Ok(Vec3::new(server_dir_x, 0.0, server_dir_y))
}

/// Maps a finite Bevy XZ direction to server XY without applying the world
/// centre offset. Bevy Y is not part of planar movement direction.
pub(in crate::game) fn main_world_server_direction(
    bevy_direction: Vec3,
) -> Result<Vec2, MainWorldCoordinateError> {
    if !bevy_direction.x.is_finite() || !bevy_direction.z.is_finite() {
        return Err(MainWorldCoordinateError::NonFiniteDirection);
    }
    Ok(Vec2::new(bevy_direction.x, bevy_direction.z))
}

/// Produces the only client move-direction representation accepted by later
/// prediction and send systems. The result has unit length or is rejected.
pub(in crate::game) fn main_world_normalized_direction(
    direction: Vec2,
) -> Result<Vec2, MainWorldCoordinateError> {
    if !direction.is_finite() {
        return Err(MainWorldCoordinateError::NonFiniteDirection);
    }
    direction
        .try_normalize()
        .ok_or(MainWorldCoordinateError::ZeroDirection)
}

/// The single client/server mapping for the initial public main world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) struct MainWorldAuthorityContract {
    pub logical_id: &'static str,
    pub client_scene_id: &'static str,
    pub server_scene_code: &'static str,
    pub server_scene_id: i32,
    pub default_spawn_id: i32,
    pub room_id: &'static str,
    pub policy_id: &'static str,
}

pub(in crate::game) const MAIN_WORLD_AUTHORITY_CONTRACT: MainWorldAuthorityContract =
    MainWorldAuthorityContract {
        logical_id: MAIN_WORLD_LOGICAL_ID,
        client_scene_id: MAIN_WORLD_CLIENT_SCENE_ID,
        server_scene_code: MAIN_WORLD_SERVER_SCENE_CODE,
        server_scene_id: MAIN_WORLD_SERVER_SCENE_ID,
        default_spawn_id: MAIN_WORLD_SERVER_DEFAULT_SPAWN_ID,
        room_id: MAIN_WORLD_PUBLIC_ROOM_ID,
        policy_id: MAIN_WORLD_ROOM_POLICY_ID,
    };

#[derive(Debug, Deserialize)]
struct MainWorldRoomMovementState {
    room_id: String,
    entities: Vec<MainWorldRoomMovementEntity>,
}

#[derive(Debug, Deserialize)]
struct MainWorldRoomMovementEntity {
    entity_id: u64,
    character_id: String,
    scene_id: i32,
    x: f32,
    y: f32,
    dir_x: f32,
    dir_y: f32,
    moving: bool,
    last_input_frame: u32,
}

/// Converts the complete `movement_demo` state embedded in a room/frame
/// snapshot into the same shape consumed by the live movement pipeline.
pub(in crate::game) fn main_world_movement_snapshot_from_room(
    snapshot: &crate::game::myserver::protocol::pb::RoomSnapshot,
    frame_id: u32,
) -> Option<crate::game::myserver::protocol::pb::MovementSnapshotPush> {
    if snapshot.room_id != MAIN_WORLD_PUBLIC_ROOM_ID {
        return None;
    }
    let state: MainWorldRoomMovementState = serde_json::from_str(&snapshot.game_state).ok()?;
    if state.room_id != snapshot.room_id {
        return None;
    }
    let entities = state
        .entities
        .into_iter()
        .map(
            |entity| crate::game::myserver::protocol::pb::EntityTransform {
                entity_id: entity.entity_id,
                character_id: entity.character_id,
                scene_id: entity.scene_id,
                x: entity.x,
                y: entity.y,
                dir_x: entity.dir_x,
                dir_y: entity.dir_y,
                moving: entity.moving,
                last_input_frame: entity.last_input_frame,
            },
        )
        .collect();
    Some(crate::game::myserver::protocol::pb::MovementSnapshotPush {
        room_id: snapshot.room_id.clone(),
        frame_id,
        entities,
        // The embedded state contains every entity, but it is a periodic sample
        // rather than a correction that should reset interpolation/prediction.
        full_sync: false,
        reason: "frame_bundle_snapshot".to_owned(),
        correction_kind: crate::game::myserver::protocol::pb::MovementCorrectionKind::Incremental
            as i32,
        reason_code: crate::game::myserver::protocol::pb::MovementCorrectionReason::Periodic as i32,
        target_character_ids: Vec::new(),
        reference_frame_id: frame_id,
    })
}

pub(in crate::game) fn main_world_movement_snapshot_from_event(
    event: &crate::game::myserver::MyServerEvent,
) -> Option<Cow<'_, crate::game::myserver::protocol::pb::MovementSnapshotPush>> {
    match event {
        crate::game::myserver::MyServerEvent::MovementSnapshotPush(push) => {
            Some(Cow::Borrowed(push))
        }
        crate::game::myserver::MyServerEvent::FrameBundlePush(push) => push
            .snapshot
            .as_ref()
            .and_then(|snapshot| main_world_movement_snapshot_from_room(snapshot, push.frame_id))
            .map(Cow::Owned),
        crate::game::myserver::MyServerEvent::RoomStatePush(push) => push
            .snapshot
            .as_ref()
            .and_then(|snapshot| {
                main_world_movement_snapshot_from_room(snapshot, snapshot.current_frame_id)
            })
            .map(Cow::Owned),
        _ => None,
    }
}

impl MainWorldAuthorityContract {
    pub fn client_scene(self) -> SceneId {
        SceneId::from(self.client_scene_id)
    }

    /// `MovementSnapshotPush.entities[].scene_id` is the final authority for a
    /// character's active scene. A client session may only render this mapping
    /// when that value matches.
    pub fn is_authoritative_entity_scene(self, scene_id: i32) -> bool {
        scene_id == self.server_scene_id
    }

    /// Validates the serialized `RoomStatePush.snapshot.game_state` emitted by
    /// `movement_demo`. It is a room-level compatibility assertion, not the
    /// source of a character's active scene; entity snapshot scene IDs remain
    /// authoritative for that decision.
    pub fn validate_room_game_state(self, game_state: &str) -> Result<(), MainWorldContractError> {
        let scene_id = room_game_state_scene_id(game_state)?;
        if scene_id == self.server_scene_id {
            Ok(())
        } else {
            Err(MainWorldContractError::UnexpectedRoomGameStateScene {
                expected: self.server_scene_id,
                actual: scene_id,
            })
        }
    }
}

/// Extracts the room-level scene id from the `movement_demo` serialized state.
///
/// A missing or malformed field is rejected rather than letting a later entry
/// coordinator infer a scene from unrelated room metadata.
pub(in crate::game) fn room_game_state_scene_id(
    game_state: &str,
) -> Result<i32, MainWorldContractError> {
    let value: Value = serde_json::from_str(game_state)
        .map_err(|error| MainWorldContractError::InvalidRoomGameState(error.to_string()))?;
    let Some(scene_id) = value.get("scene_id").and_then(Value::as_i64) else {
        return Err(MainWorldContractError::MissingRoomGameStateScene);
    };

    i32::try_from(scene_id)
        .map_err(|_| MainWorldContractError::InvalidRoomGameStateSceneId(scene_id))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game) enum MainWorldContractError {
    InvalidRoomGameState(String),
    MissingRoomGameStateScene,
    InvalidRoomGameStateSceneId(i64),
    UnexpectedRoomGameStateScene { expected: i32, actual: i32 },
}

impl fmt::Display for MainWorldContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRoomGameState(error) => {
                write!(
                    formatter,
                    "main-world room game state is not valid JSON: {error}"
                )
            }
            Self::MissingRoomGameStateScene => {
                formatter.write_str("main-world room game state is missing scene_id")
            }
            Self::InvalidRoomGameStateSceneId(scene_id) => write!(
                formatter,
                "main-world room game state scene_id is outside i32 range: {scene_id}"
            ),
            Self::UnexpectedRoomGameStateScene { expected, actual } => write!(
                formatter,
                "main-world room game state scene_id mismatch: expected {expected}, got {actual}"
            ),
        }
    }
}

impl std::error::Error for MainWorldContractError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_centralizes_public_main_world_identifiers() {
        let contract = MAIN_WORLD_AUTHORITY_CONTRACT;

        assert_eq!(contract.logical_id, "main_world");
        assert_eq!(contract.client_scene(), SceneId::from("world.main"));
        assert_eq!(contract.server_scene_code, "grassland_01");
        assert_eq!(contract.server_scene_id, 1);
        assert_eq!(contract.default_spawn_id, 1001);
        assert_eq!(contract.room_id, "main-world-public");
        assert_eq!(contract.policy_id, "movement_demo");
    }

    #[test]
    fn room_game_state_is_a_compatibility_check_for_the_public_main_world() {
        let game_state = r#"{"room_id":"main-world-public","scene_id":1,"entities":[]}"#;

        assert_eq!(room_game_state_scene_id(game_state), Ok(1));
        assert_eq!(
            MAIN_WORLD_AUTHORITY_CONTRACT.validate_room_game_state(game_state),
            Ok(())
        );
    }

    #[test]
    fn room_game_state_mismatch_is_not_allowed_to_select_another_client_scene() {
        assert_eq!(
            MAIN_WORLD_AUTHORITY_CONTRACT.validate_room_game_state(r#"{"scene_id":2}"#),
            Err(MainWorldContractError::UnexpectedRoomGameStateScene {
                expected: MAIN_WORLD_SERVER_SCENE_ID,
                actual: 2,
            })
        );
    }

    #[test]
    fn entity_snapshot_scene_id_is_the_active_scene_authority() {
        let contract = MAIN_WORLD_AUTHORITY_CONTRACT;

        assert!(contract.is_authoritative_entity_scene(MAIN_WORLD_SERVER_SCENE_ID));
        assert!(!contract.is_authoritative_entity_scene(2));
    }

    #[test]
    fn malformed_or_incomplete_game_state_is_rejected() {
        assert!(matches!(
            room_game_state_scene_id("not-json"),
            Err(MainWorldContractError::InvalidRoomGameState(_))
        ));
        assert_eq!(
            room_game_state_scene_id(r#"{"entities":[]}"#),
            Err(MainWorldContractError::MissingRoomGameStateScene)
        );
    }

    #[test]
    fn frame_bundle_room_state_normalizes_to_movement_snapshot() {
        let snapshot = crate::game::myserver::protocol::pb::RoomSnapshot {
            room_id: MAIN_WORLD_PUBLIC_ROOM_ID.to_owned(),
            game_state: r#"{"room_id":"main-world-public","scene_id":1,"entities":[{"entity_id":7,"character_id":"chr-a","scene_id":1,"x":2001.0,"y":1999.0,"dir_x":1.0,"dir_y":0.0,"moving":true,"last_input_frame":9}]}"#.to_owned(),
            ..Default::default()
        };
        let push = main_world_movement_snapshot_from_room(&snapshot, 12).unwrap();
        assert_eq!(push.frame_id, 12);
        assert_eq!(push.entities[0].character_id, "chr-a");
        assert!(push.entities[0].moving);
        assert!(!push.full_sync);
    }

    #[test]
    fn first_main_world_scope_excludes_later_gameplay_systems() {
        assert_eq!(
            MAIN_WORLD_NON_GOALS,
            [
                "matchmaking",
                "dynamic_sharding",
                "cross_region_transfer",
                "production_player_model",
                "combat",
            ]
        );
    }

    #[test]
    fn centred_coordinate_mapping_covers_world_centre_and_all_edges() {
        assert_eq!(main_world_bevy_position(2000.0, 2000.0), Ok(Vec3::ZERO));
        assert_eq!(
            main_world_bevy_position(0.0, 0.0),
            Ok(Vec3::new(-2000.0, 0.0, -2000.0))
        );
        assert_eq!(
            main_world_bevy_position(3999.999, 0.0),
            Ok(Vec3::new(1999.999, 0.0, -2000.0))
        );
        assert_eq!(
            main_world_bevy_position(0.0, 3999.999),
            Ok(Vec3::new(-2000.0, 0.0, 1999.999))
        );
        assert_eq!(
            main_world_bevy_position(3999.999, 3999.999),
            Ok(Vec3::new(1999.999, 0.0, 1999.999))
        );
    }

    #[test]
    fn coordinate_mapping_rejects_exclusive_upper_bound_and_non_finite_values() {
        for invalid in [
            (4000.0, 2000.0),
            (2000.0, 4000.0),
            (-0.001, 2000.0),
            (2000.0, -0.001),
            (f32::NAN, 2000.0),
            (2000.0, f32::INFINITY),
        ] {
            assert!(main_world_bevy_position(invalid.0, invalid.1).is_err());
        }
    }

    #[test]
    fn server_and_bevy_position_mapping_round_trip_for_representative_values() {
        let samples = [
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 3998.5),
            Vec2::new(2000.0, 2000.0),
            Vec2::new(3999.999, 3999.999),
        ];

        for server_position in samples {
            let bevy_position = main_world_bevy_position(server_position.x, server_position.y)
                .expect("sample must be in bounds");
            assert_eq!(
                main_world_server_position(bevy_position),
                Ok(server_position)
            );
        }
    }

    #[test]
    fn server_and_bevy_position_mapping_round_trips_every_finite_grid_sample() {
        for server_x in (0..4000).step_by(37) {
            for server_y in (0..4000).step_by(41) {
                let server_position = Vec2::new(server_x as f32 + 0.25, server_y as f32 + 0.5);
                let bevy_position =
                    main_world_bevy_position(server_position.x, server_position.y).unwrap();
                assert_eq!(
                    main_world_server_position(bevy_position),
                    Ok(server_position),
                    "round trip failed for {server_position:?}"
                );
            }
        }
    }

    #[test]
    fn authority_position_checks_scene_before_applying_mapping() {
        assert_eq!(
            main_world_bevy_position_from_authority(MAIN_WORLD_SERVER_SCENE_ID, 2002.0, 1998.0),
            Ok(Vec3::new(2.0, 0.0, -2.0))
        );
        assert_eq!(
            main_world_bevy_position_from_authority(2, 2000.0, 2000.0),
            Err(MainWorldCoordinateError::UnexpectedScene {
                expected: MAIN_WORLD_SERVER_SCENE_ID,
                actual: 2,
            })
        );
    }

    #[test]
    fn directions_use_axis_mapping_without_position_offset() {
        assert_eq!(
            main_world_bevy_direction(0.6, -0.8),
            Ok(Vec3::new(0.6, 0.0, -0.8))
        );
        assert_eq!(
            main_world_server_direction(Vec3::new(0.6, 12.0, -0.8)),
            Ok(Vec2::new(0.6, -0.8))
        );
        assert_eq!(
            main_world_bevy_direction(f32::NAN, 0.0),
            Err(MainWorldCoordinateError::NonFiniteDirection)
        );
        assert_eq!(
            main_world_server_direction(Vec3::new(0.0, 0.0, f32::NEG_INFINITY)),
            Err(MainWorldCoordinateError::NonFiniteDirection)
        );
    }

    #[test]
    fn movement_contract_freezes_cadence_speed_normalization_and_stop_semantics() {
        assert_eq!(MAIN_WORLD_AUTHORITY_TICKS_PER_SECOND, 20);
        assert_eq!(MAIN_WORLD_AUTHORITY_TICK_SECONDS, 0.05);
        assert_eq!(MAIN_WORLD_MOVE_SPEED_METRES_PER_SECOND, 4.0);
        assert_eq!(MAIN_WORLD_MOVE_DIR_KEEPALIVE_FRAMES, 1);
        assert_eq!(MAIN_WORLD_SERVER_CONTROL_STOP_FRAMES, 3);
        assert_eq!(MAIN_WORLD_MOVE_STOP_MESSAGES_PER_TRANSITION, 1);
        assert_eq!(
            main_world_normalized_direction(Vec2::new(3.0, 4.0)),
            Ok(Vec2::new(0.6, 0.8))
        );
        assert_eq!(
            main_world_normalized_direction(Vec2::ZERO),
            Err(MainWorldCoordinateError::ZeroDirection)
        );
        assert_eq!(
            main_world_normalized_direction(Vec2::new(f32::NAN, 0.0)),
            Err(MainWorldCoordinateError::NonFiniteDirection)
        );
    }

    #[test]
    fn frame_kinds_keep_authority_prediction_confirmation_and_render_boundaries_distinct() {
        let authority = MainWorldAuthorityFrame(42);
        let predicted = MainWorldPredictedFrame(43);
        let confirmed = MainWorldConfirmedFrame(41);
        let render = MainWorldRenderFrame(9_001);

        assert_eq!(authority.0, 42);
        assert_eq!(predicted.0, 43);
        assert_eq!(confirmed.0, 41);
        assert_eq!(render.0, 9_001);
        assert_eq!(
            MainWorldMoveInputKind::MoveDirection,
            MainWorldMoveInputKind::MoveDirection
        );
        assert_ne!(
            MainWorldMoveInputKind::MoveDirection,
            MainWorldMoveInputKind::MoveStop
        );
    }
}
