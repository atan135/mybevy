//! Stable client-to-MyServer identifiers for the first main-world entry flow.
//!
//! The scene framework owns the client scene session and its presentation
//! lifetime. MyServer owns the ticket-bound character, its authoritative scene
//! id, and movement state. This module is deliberately limited to that mapping;
//! it does not implement room entry or scene loading.

use crate::framework::scene::prelude::SceneId;
use serde_json::Value;
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

/// Out-of-scope systems for the first main-world authority loop.
pub(in crate::game) const MAIN_WORLD_NON_GOALS: &[&str] = &[
    "matchmaking",
    "dynamic_sharding",
    "cross_region_transfer",
    "production_player_model",
    "combat",
];

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
}
