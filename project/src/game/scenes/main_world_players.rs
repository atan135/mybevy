use bevy::prelude::*;
use std::collections::HashMap;

use crate::framework::scene::prelude::{
    SCENE_CAMERA_LOCAL_PLAYER_TARGET_TAG, SceneCameraTarget, SceneOwned, SceneSessionId,
};

use super::main_world_contract::MAIN_WORLD_SERVER_SCENE_ID;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum MainWorldPlayerOwnership {
    Local,
    Remote,
}

#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub(in crate::game) struct MainWorldPlayer {
    pub character_id: String,
    pub server_entity_id: i64,
    pub ownership: MainWorldPlayerOwnership,
    pub scene_session_id: SceneSessionId,
    pub last_authoritative_frame: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::game) struct MainWorldPlayerRegistration {
    pub character_id: String,
    pub server_entity_id: i64,
    pub server_scene_id: i32,
    pub generation: u64,
    pub authoritative_frame: u32,
    pub transform: Transform,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum MainWorldPlayerRegistrationResult {
    Created(Entity),
    Updated(Entity),
    Replaced { stale: Entity, current: Entity },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game) enum MainWorldPlayerRegistrationError {
    EmptyCharacterId,
    UnexpectedScene { actual: i32 },
    NonFiniteTransform,
    StaleGeneration { expected: u64, actual: u64 },
    StaleFrame { current: u32, actual: u32 },
}

#[derive(Resource, Clone, Debug)]
pub(in crate::game) struct MainWorldPlayerRegistry {
    session_id: SceneSessionId,
    generation: u64,
    local_character_id: String,
    players: HashMap<String, MainWorldPlayerRegistryEntry>,
}

#[derive(Clone, Copy, Debug)]
struct MainWorldPlayerRegistryEntry {
    entity: Entity,
    server_entity_id: i64,
    last_authoritative_frame: u32,
}

impl MainWorldPlayerRegistry {
    pub fn new(
        session_id: impl Into<SceneSessionId>,
        generation: u64,
        local_character_id: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            generation,
            local_character_id: local_character_id.into(),
            players: HashMap::new(),
        }
    }

    pub fn session_id(&self) -> &SceneSessionId {
        &self.session_id
    }

    pub fn get(&self, character_id: &str) -> Option<Entity> {
        self.players.get(character_id).map(|entry| entry.entity)
    }

    pub fn len(&self) -> usize {
        self.players.len()
    }

    pub fn clear(&mut self, commands: &mut Commands) {
        for entry in self.players.drain().map(|(_, entry)| entry) {
            commands.entity(entry.entity).despawn();
        }
    }

    pub fn register(
        &mut self,
        commands: &mut Commands,
        registration: MainWorldPlayerRegistration,
    ) -> Result<MainWorldPlayerRegistrationResult, MainWorldPlayerRegistrationError> {
        self.validate(&registration)?;
        let ownership = if registration.character_id == self.local_character_id {
            MainWorldPlayerOwnership::Local
        } else {
            MainWorldPlayerOwnership::Remote
        };
        let player = MainWorldPlayer {
            character_id: registration.character_id.clone(),
            server_entity_id: registration.server_entity_id,
            ownership,
            scene_session_id: self.session_id.clone(),
            last_authoritative_frame: registration.authoritative_frame,
        };

        if let Some(existing) = self.players.get(&registration.character_id).copied() {
            if registration.authoritative_frame < existing.last_authoritative_frame {
                return Err(MainWorldPlayerRegistrationError::StaleFrame {
                    current: existing.last_authoritative_frame,
                    actual: registration.authoritative_frame,
                });
            }
            if existing.server_entity_id == registration.server_entity_id {
                commands
                    .entity(existing.entity)
                    .insert((player, registration.transform));
                self.players.insert(
                    registration.character_id,
                    MainWorldPlayerRegistryEntry {
                        entity: existing.entity,
                        server_entity_id: existing.server_entity_id,
                        last_authoritative_frame: registration.authoritative_frame,
                    },
                );
                return Ok(MainWorldPlayerRegistrationResult::Updated(existing.entity));
            }
            commands.entity(existing.entity).despawn();
            let current =
                spawn_player_root(commands, &self.session_id, player, registration.transform);
            self.players.insert(
                registration.character_id,
                MainWorldPlayerRegistryEntry {
                    entity: current,
                    server_entity_id: registration.server_entity_id,
                    last_authoritative_frame: registration.authoritative_frame,
                },
            );
            return Ok(MainWorldPlayerRegistrationResult::Replaced {
                stale: existing.entity,
                current,
            });
        }

        let character_id = registration.character_id;
        let current = spawn_player_root(commands, &self.session_id, player, registration.transform);
        self.players.insert(
            character_id,
            MainWorldPlayerRegistryEntry {
                entity: current,
                server_entity_id: registration.server_entity_id,
                last_authoritative_frame: registration.authoritative_frame,
            },
        );
        Ok(MainWorldPlayerRegistrationResult::Created(current))
    }

    fn validate(
        &self,
        registration: &MainWorldPlayerRegistration,
    ) -> Result<(), MainWorldPlayerRegistrationError> {
        if registration.character_id.trim().is_empty() {
            return Err(MainWorldPlayerRegistrationError::EmptyCharacterId);
        }
        if registration.server_scene_id != MAIN_WORLD_SERVER_SCENE_ID {
            return Err(MainWorldPlayerRegistrationError::UnexpectedScene {
                actual: registration.server_scene_id,
            });
        }
        if registration.generation != self.generation {
            return Err(MainWorldPlayerRegistrationError::StaleGeneration {
                expected: self.generation,
                actual: registration.generation,
            });
        }
        if !registration.transform.translation.is_finite()
            || !registration.transform.rotation.is_finite()
            || !registration.transform.scale.is_finite()
        {
            return Err(MainWorldPlayerRegistrationError::NonFiniteTransform);
        }
        Ok(())
    }
}

fn spawn_player_root(
    commands: &mut Commands,
    session_id: &SceneSessionId,
    player: MainWorldPlayer,
    transform: Transform,
) -> Entity {
    let local = player.ownership == MainWorldPlayerOwnership::Local;
    let mut entity = commands.spawn((
        player,
        SceneOwned::new(session_id.clone()),
        transform,
        GlobalTransform::default(),
    ));
    if local {
        entity.insert(
            SceneCameraTarget::new(session_id.clone())
                .with_tag(SCENE_CAMERA_LOCAL_PLAYER_TARGET_TAG),
        );
    }
    entity.id()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registration(character: &str, entity_id: i64, frame: u32) -> MainWorldPlayerRegistration {
        MainWorldPlayerRegistration {
            character_id: character.to_owned(),
            server_entity_id: entity_id,
            server_scene_id: MAIN_WORLD_SERVER_SCENE_ID,
            generation: 3,
            authoritative_frame: frame,
            transform: Transform::from_xyz(1.0, 0.0, 2.0),
        }
    }

    fn register(
        world: &mut World,
        registry: &mut MainWorldPlayerRegistry,
        value: MainWorldPlayerRegistration,
    ) -> Result<MainWorldPlayerRegistrationResult, MainWorldPlayerRegistrationError> {
        let result = registry.register(&mut world.commands(), value);
        world.flush();
        result
    }

    #[test]
    fn registry_is_unique_by_character_and_updates_same_entity() {
        let mut world = World::new();
        let mut registry = MainWorldPlayerRegistry::new("session-a", 3, "local");
        let first = register(&mut world, &mut registry, registration("remote", 10, 1)).unwrap();
        let second = register(&mut world, &mut registry, registration("remote", 10, 2)).unwrap();
        let MainWorldPlayerRegistrationResult::Created(first) = first else {
            panic!()
        };
        assert_eq!(second, MainWorldPlayerRegistrationResult::Updated(first));
        assert_eq!(registry.len(), 1);
        assert_eq!(
            world
                .get::<MainWorldPlayer>(first)
                .unwrap()
                .last_authoritative_frame,
            2
        );
    }

    #[test]
    fn changed_server_entity_id_replaces_stale_root() {
        let mut world = World::new();
        let mut registry = MainWorldPlayerRegistry::new("session-a", 3, "local");
        let first = register(&mut world, &mut registry, registration("remote", 10, 1)).unwrap();
        let MainWorldPlayerRegistrationResult::Created(stale) = first else {
            panic!()
        };
        let replaced = register(&mut world, &mut registry, registration("remote", 11, 2)).unwrap();
        let MainWorldPlayerRegistrationResult::Replaced {
            stale: old,
            current,
        } = replaced
        else {
            panic!()
        };
        assert_eq!(old, stale);
        assert!(world.get_entity(stale).is_err());
        assert_eq!(registry.get("remote"), Some(current));
    }

    #[test]
    fn local_ticket_character_gets_camera_target_but_remote_does_not() {
        let mut world = World::new();
        let mut registry = MainWorldPlayerRegistry::new("session-a", 3, "ticket-character");
        let local = register(
            &mut world,
            &mut registry,
            registration("ticket-character", 1, 1),
        )
        .unwrap();
        let remote = register(
            &mut world,
            &mut registry,
            registration("account-player-id", 2, 1),
        )
        .unwrap();
        let MainWorldPlayerRegistrationResult::Created(local) = local else {
            panic!()
        };
        let MainWorldPlayerRegistrationResult::Created(remote) = remote else {
            panic!()
        };
        let target = world.get::<SceneCameraTarget>(local).unwrap();
        assert!(target.has_tag(SCENE_CAMERA_LOCAL_PLAYER_TARGET_TAG));
        assert!(target.is_session(registry.session_id()));
        assert!(world.get::<SceneCameraTarget>(remote).is_none());
        assert_eq!(
            world.get::<MainWorldPlayer>(remote).unwrap().ownership,
            MainWorldPlayerOwnership::Remote
        );
    }

    #[test]
    fn registry_rejects_invalid_or_stale_input_without_mutating_session() {
        let mut world = World::new();
        let mut registry = MainWorldPlayerRegistry::new("session-a", 3, "local");
        let mut empty = registration("", 1, 1);
        assert_eq!(
            register(&mut world, &mut registry, empty.clone()),
            Err(MainWorldPlayerRegistrationError::EmptyCharacterId)
        );
        empty.character_id = "remote".to_owned();
        empty.server_scene_id = 99;
        assert_eq!(
            register(&mut world, &mut registry, empty.clone()),
            Err(MainWorldPlayerRegistrationError::UnexpectedScene { actual: 99 })
        );
        empty.server_scene_id = MAIN_WORLD_SERVER_SCENE_ID;
        empty.generation = 2;
        assert_eq!(
            register(&mut world, &mut registry, empty.clone()),
            Err(MainWorldPlayerRegistrationError::StaleGeneration {
                expected: 3,
                actual: 2
            })
        );
        empty.generation = 3;
        empty.transform.translation.x = f32::NAN;
        assert_eq!(
            register(&mut world, &mut registry, empty),
            Err(MainWorldPlayerRegistrationError::NonFiniteTransform)
        );
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn registry_rejects_older_frames_and_clear_only_despawns_its_session() {
        let mut world = World::new();
        let unrelated = world.spawn_empty().id();
        let mut registry = MainWorldPlayerRegistry::new("session-a", 3, "local");
        let created = register(&mut world, &mut registry, registration("remote", 1, 5)).unwrap();
        let MainWorldPlayerRegistrationResult::Created(entity) = created else {
            panic!()
        };
        assert_eq!(
            register(&mut world, &mut registry, registration("remote", 1, 4)),
            Err(MainWorldPlayerRegistrationError::StaleFrame {
                current: 5,
                actual: 4
            })
        );
        registry.clear(&mut world.commands());
        world.flush();
        assert!(world.get_entity(entity).is_err());
        assert!(world.get_entity(unrelated).is_ok());
        assert_eq!(registry.len(), 0);
    }
}
