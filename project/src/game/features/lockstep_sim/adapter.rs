#![allow(dead_code)]

use sim_core::{
    CombatConfig, CombatState, EntityId, EntityKind, Fp, FrameId, MoveCommand, MovementConfig,
    MovementState, QuantizedDir, SceneBounds, SimCommand, SimConfig, SimEntity, SimInput,
    SimInputSource, SimStepResult, SimTransform, SimWorld, StaticObstacle, StepError, TeamId,
    Vec2Fp, step,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) struct ClientSimConfig {
    pub(in crate::game) tick_rate: u16,
    pub(in crate::game) default_speed_per_second: Fp,
    pub(in crate::game) max_speed_per_second: Fp,
    pub(in crate::game) bounds_min: Vec2Fp,
    pub(in crate::game) bounds_max: Vec2Fp,
}

impl Default for ClientSimConfig {
    fn default() -> Self {
        Self {
            tick_rate: 60,
            default_speed_per_second: Fp::from_i32(6),
            max_speed_per_second: Fp::from_i32(10),
            bounds_min: Vec2Fp::new(Fp::from_i32(-100), Fp::from_i32(-100)),
            bounds_max: Vec2Fp::new(Fp::from_i32(100), Fp::from_i32(100)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game) struct ClientSimEntity {
    pub(in crate::game) entity_id: u32,
    pub(in crate::game) character_id: Option<String>,
    pub(in crate::game) team_id: u16,
    pub(in crate::game) position: Vec2Fp,
    pub(in crate::game) radius: Fp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game) struct ClientSimInput {
    pub(in crate::game) frame: u32,
    pub(in crate::game) character_id: String,
    pub(in crate::game) entity_id: u32,
    pub(in crate::game) seq: u32,
    pub(in crate::game) source: SimInputSource,
    pub(in crate::game) command: SimCommand,
}

impl ClientSimInput {
    pub(in crate::game) fn move_command(
        frame: u32,
        character_id: impl Into<String>,
        entity_id: u32,
        seq: u32,
        move_dir: QuantizedDir,
    ) -> Self {
        Self {
            frame,
            character_id: character_id.into(),
            entity_id,
            seq,
            source: SimInputSource::Real,
            command: SimCommand::Move(MoveCommand {
                dir: move_dir,
                speed_per_second: None,
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) struct ClientSimStepSummary {
    pub(in crate::game) frame: u32,
    pub(in crate::game) state_hash: u64,
    pub(in crate::game) event_count: usize,
}

pub(in crate::game) fn build_client_sim_config(config: ClientSimConfig) -> SimConfig {
    SimConfig {
        movement: MovementConfig {
            tick_rate: config.tick_rate,
            default_speed_per_second: config.default_speed_per_second,
            max_speed_per_second: config.max_speed_per_second,
            bounds: SceneBounds {
                min: config.bounds_min,
                max: config.bounds_max,
            },
            static_obstacles: Vec::<StaticObstacle>::new(),
        },
        combat: CombatConfig::default(),
    }
}

pub(in crate::game) fn build_client_sim_world(
    frame: u32,
    entities: Vec<ClientSimEntity>,
) -> Result<SimWorld, sim_core::state::SimWorldError> {
    SimWorld::new(
        FrameId::new(frame),
        entities
            .into_iter()
            .map(|entity| SimEntity {
                id: EntityId::new(entity.entity_id),
                kind: EntityKind::Player,
                owner_character_id: entity.character_id,
                team_id: TeamId::new(entity.team_id),
                transform: SimTransform {
                    pos: entity.position,
                    facing: QuantizedDir::RIGHT,
                    radius: entity.radius,
                },
                movement: MovementState::default(),
                combat: CombatState::default(),
                alive: true,
            })
            .collect(),
    )
}

pub(in crate::game) fn step_client_sim(
    world: &mut SimWorld,
    frame: u32,
    inputs: &[ClientSimInput],
    config: &SimConfig,
) -> Result<ClientSimStepSummary, StepError> {
    let sim_inputs = inputs.iter().map(to_sim_input).collect::<Vec<_>>();
    let result: SimStepResult = step(world, FrameId::new(frame), &sim_inputs, config)?;

    Ok(ClientSimStepSummary {
        frame: result.frame.raw(),
        state_hash: result.state_hash.value,
        event_count: result.events.len(),
    })
}

fn to_sim_input(input: &ClientSimInput) -> SimInput {
    SimInput {
        frame: FrameId::new(input.frame),
        character_id: input.character_id.clone(),
        entity_id: EntityId::new(input.entity_id),
        seq: input.seq,
        source: input.source,
        command: input.command,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sim_core_types_and_step_are_available_to_client_build() {
        let config = build_client_sim_config(ClientSimConfig::default());
        let mut world = build_client_sim_world(
            0,
            vec![ClientSimEntity {
                entity_id: 100,
                character_id: Some("local_player".to_owned()),
                team_id: 1,
                position: Vec2Fp::zero(),
                radius: Fp::from_milli(500),
            }],
        )
        .unwrap();
        let inputs = vec![ClientSimInput::move_command(
            1,
            "local_player",
            100,
            1,
            QuantizedDir::RIGHT,
        )];

        let summary = step_client_sim(&mut world, 1, &inputs, &config).unwrap();

        assert_eq!(summary.frame, 1);
        assert_eq!(summary.event_count, 0);
        assert_ne!(summary.state_hash, 0);

        let entity = world.entity(EntityId::new(100)).unwrap();
        assert_eq!(
            entity.transform.pos,
            Vec2Fp::new(Fp::from_milli(100), Fp::ZERO)
        );
    }
}
