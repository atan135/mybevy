#![allow(dead_code)]

use bevy::prelude::*;
use sim_core::{
    CastSkillCommand, EntityId, FaceCommand, Fp, FrameId, MoveCommand, QuantizedDir,
    QuantizedDirError, SimCommand, SimInput, SimInputSource, SkillId, SkillTarget, Vec2Fp,
};

const QUANTIZED_DIR_SCALE: f32 = 1_000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum LockstepSimInputError {
    NonFiniteAxis,
    ZeroDirection,
    QuantizedDir(QuantizedDirError),
}

impl From<QuantizedDirError> for LockstepSimInputError {
    fn from(value: QuantizedDirError) -> Self {
        Self::QuantizedDir(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum LockstepSimSkillTargetIntent {
    None,
    Entity(u32),
    Position { x_milli: i64, y_milli: i64 },
    Direction(QuantizedDir),
}

impl LockstepSimSkillTargetIntent {
    fn into_skill_target(self) -> SkillTarget {
        match self {
            Self::None => SkillTarget::None,
            Self::Entity(entity_id) => SkillTarget::Entity(EntityId::new(entity_id)),
            Self::Position { x_milli, y_milli } => SkillTarget::Position(Vec2Fp::new(
                Fp::from_milli(x_milli),
                Fp::from_milli(y_milli),
            )),
            Self::Direction(dir) => SkillTarget::Direction(dir),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum LockstepSimInputIntent {
    Move {
        dir: QuantizedDir,
        speed_per_second: Option<Fp>,
    },
    Stop,
    Face {
        dir: QuantizedDir,
    },
    CastSkill {
        skill_id: u32,
        target: LockstepSimSkillTargetIntent,
    },
    Noop,
}

impl LockstepSimInputIntent {
    pub(in crate::game) fn move_from_quantized_dir(
        dir: QuantizedDir,
        speed_per_second: Option<Fp>,
    ) -> Result<Self, LockstepSimInputError> {
        ensure_non_zero_dir(dir)?;
        Ok(Self::Move {
            dir,
            speed_per_second,
        })
    }

    pub(in crate::game) fn move_from_axes(
        x: f32,
        y: f32,
        speed_per_second: Option<Fp>,
    ) -> Result<Self, LockstepSimInputError> {
        Self::move_from_quantized_dir(quantize_axis_dir(x, y)?, speed_per_second)
    }

    pub(in crate::game) fn face_from_quantized_dir(
        dir: QuantizedDir,
    ) -> Result<Self, LockstepSimInputError> {
        ensure_non_zero_dir(dir)?;
        Ok(Self::Face { dir })
    }

    pub(in crate::game) fn face_from_axes(x: f32, y: f32) -> Result<Self, LockstepSimInputError> {
        Self::face_from_quantized_dir(quantize_axis_dir(x, y)?)
    }

    pub(in crate::game) fn into_sim_command(self) -> SimCommand {
        match self {
            Self::Move {
                dir,
                speed_per_second,
            } => SimCommand::Move(MoveCommand {
                dir,
                speed_per_second,
            }),
            Self::Stop => SimCommand::Stop,
            Self::Face { dir } => SimCommand::Face(FaceCommand { dir }),
            Self::CastSkill { skill_id, target } => SimCommand::CastSkill(CastSkillCommand {
                skill_id: SkillId::new(skill_id),
                target: target.into_skill_target(),
            }),
            Self::Noop => SimCommand::Noop,
        }
    }

    pub(in crate::game) fn into_sim_input(
        self,
        frame: u32,
        character_id: impl Into<String>,
        entity_id: u32,
        seq: u32,
        source: SimInputSource,
    ) -> SimInput {
        SimInput {
            frame: FrameId::new(frame),
            character_id: character_id.into(),
            entity_id: EntityId::new(entity_id),
            seq,
            source,
            command: self.into_sim_command(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Resource, PartialEq, Eq)]
pub(in crate::game) struct LockstepSimInputSeq {
    next_seq: u32,
}

impl LockstepSimInputSeq {
    pub(in crate::game) fn next(&mut self) -> u32 {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.saturating_add(1);
        seq
    }

    pub(in crate::game) fn reset(&mut self) {
        self.next_seq = 0;
    }
}

pub(in crate::game) fn quantize_keyboard_axis(
    x: i32,
    y: i32,
) -> Result<QuantizedDir, LockstepSimInputError> {
    quantize_axis_dir(x as f32, y as f32)
}

pub(in crate::game) fn quantize_raw_dir(
    x: i16,
    y: i16,
) -> Result<QuantizedDir, LockstepSimInputError> {
    let dir = QuantizedDir::new(x, y)?;
    ensure_non_zero_dir(dir)?;
    Ok(dir)
}

pub(in crate::game) fn quantize_axis_dir(
    x: f32,
    y: f32,
) -> Result<QuantizedDir, LockstepSimInputError> {
    if !x.is_finite() || !y.is_finite() {
        return Err(LockstepSimInputError::NonFiniteAxis);
    }

    if x == 0.0 && y == 0.0 {
        return Err(LockstepSimInputError::ZeroDirection);
    }

    let length = (x.mul_add(x, y * y)).sqrt();
    if length == 0.0 || !length.is_finite() {
        return Err(LockstepSimInputError::ZeroDirection);
    }

    let quantized_x = ((x / length) * QUANTIZED_DIR_SCALE).round() as i16;
    let quantized_y = ((y / length) * QUANTIZED_DIR_SCALE).round() as i16;
    QuantizedDir::new(quantized_x, quantized_y).map_err(Into::into)
}

fn ensure_non_zero_dir(dir: QuantizedDir) -> Result<(), LockstepSimInputError> {
    if dir == QuantizedDir::ZERO {
        return Err(LockstepSimInputError::ZeroDirection);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantizes_horizontal_vertical_and_diagonal_axes() {
        assert_eq!(quantize_keyboard_axis(1, 0), Ok(QuantizedDir::RIGHT));
        assert_eq!(quantize_keyboard_axis(-1, 0), Ok(QuantizedDir::LEFT));
        assert_eq!(quantize_keyboard_axis(0, -1), Ok(QuantizedDir::UP));
        assert_eq!(quantize_keyboard_axis(0, 1), Ok(QuantizedDir::DOWN));

        let diagonal = quantize_keyboard_axis(1, -1).unwrap();
        assert_eq!(diagonal, QuantizedDir::UP_RIGHT);
        assert!(diagonal.length_squared() <= 1_000_000);
    }

    #[test]
    fn stop_intent_generates_stop_command_not_zero_move() {
        assert_eq!(
            LockstepSimInputIntent::Stop.into_sim_command(),
            SimCommand::Stop
        );
        assert_eq!(
            LockstepSimInputIntent::move_from_quantized_dir(QuantizedDir::ZERO, None),
            Err(LockstepSimInputError::ZeroDirection)
        );
    }

    #[test]
    fn face_and_cast_skill_generate_intent_only_commands() {
        let face = LockstepSimInputIntent::face_from_axes(-1.0, 0.0).unwrap();
        assert_eq!(
            face.into_sim_command(),
            SimCommand::Face(FaceCommand {
                dir: QuantizedDir::LEFT
            })
        );

        let cast = LockstepSimInputIntent::CastSkill {
            skill_id: 20,
            target: LockstepSimSkillTargetIntent::Entity(200),
        };
        assert_eq!(
            cast.into_sim_command(),
            SimCommand::CastSkill(CastSkillCommand {
                skill_id: SkillId::new(20),
                target: SkillTarget::Entity(EntityId::new(200)),
            })
        );
    }

    #[test]
    fn invalid_axes_return_errors() {
        assert_eq!(
            quantize_axis_dir(f32::NAN, 0.0),
            Err(LockstepSimInputError::NonFiniteAxis)
        );
        assert_eq!(
            quantize_axis_dir(0.0, 0.0),
            Err(LockstepSimInputError::ZeroDirection)
        );
        assert_eq!(
            LockstepSimInputIntent::face_from_quantized_dir(QuantizedDir::ZERO),
            Err(LockstepSimInputError::ZeroDirection)
        );
        assert_eq!(
            quantize_raw_dir(1_001, 0),
            Err(LockstepSimInputError::QuantizedDir(
                QuantizedDirError::XOutOfRange { value: 1_001 }
            ))
        );
        assert_eq!(
            quantize_raw_dir(1_000, 1_000),
            Err(LockstepSimInputError::QuantizedDir(
                QuantizedDirError::LengthSquaredTooLarge {
                    length_squared: 2_000_000
                }
            ))
        );
    }

    #[test]
    fn sim_input_conversion_keeps_identity_order_and_command() {
        let input = LockstepSimInputIntent::Move {
            dir: QuantizedDir::RIGHT,
            speed_per_second: None,
        }
        .into_sim_input(12, "alice", 100, 7, SimInputSource::Real);

        assert_eq!(input.frame, FrameId::new(12));
        assert_eq!(input.character_id, "alice");
        assert_eq!(input.entity_id, EntityId::new(100));
        assert_eq!(input.seq, 7);
        assert_eq!(input.source, SimInputSource::Real);
        assert_eq!(
            input.command,
            SimCommand::Move(MoveCommand {
                dir: QuantizedDir::RIGHT,
                speed_per_second: None,
            })
        );
    }

    #[test]
    fn seq_state_generates_monotonic_values_for_same_frame() {
        let mut seq = LockstepSimInputSeq::default();

        let first = LockstepSimInputIntent::Stop.into_sim_input(
            42,
            "alice",
            100,
            seq.next(),
            SimInputSource::Real,
        );
        let second = LockstepSimInputIntent::Face {
            dir: QuantizedDir::RIGHT,
        }
        .into_sim_input(42, "alice", 100, seq.next(), SimInputSource::Real);
        let third = LockstepSimInputIntent::CastSkill {
            skill_id: 9,
            target: LockstepSimSkillTargetIntent::None,
        }
        .into_sim_input(42, "alice", 100, seq.next(), SimInputSource::Real);

        assert_eq!((first.seq, second.seq, third.seq), (0, 1, 2));
    }

    #[test]
    fn same_physical_axis_quantizes_independent_of_render_delta() {
        fn command_for_render_delta(_dt_seconds: f32) -> SimCommand {
            LockstepSimInputIntent::move_from_axes(0.75, -0.75, None)
                .unwrap()
                .into_sim_command()
        }

        let at_30_fps = command_for_render_delta(1.0 / 30.0);
        let at_60_fps = command_for_render_delta(1.0 / 60.0);
        let at_144_fps = command_for_render_delta(1.0 / 144.0);

        assert_eq!(at_30_fps, at_60_fps);
        assert_eq!(at_60_fps, at_144_fps);
        assert_eq!(
            at_30_fps,
            SimCommand::Move(MoveCommand {
                dir: QuantizedDir::UP_RIGHT,
                speed_per_second: None,
            })
        );
    }
}
