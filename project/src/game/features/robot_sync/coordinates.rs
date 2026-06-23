use bevy::prelude::*;

use super::sync::{FIXED_UNIT, FixedPosition};

pub(in crate::game) const ROBOT_SYNC_WORLD_UNITS_PER_SYNC_UNIT: f32 = 0.1;
pub(in crate::game::features::robot_sync) const ROBOT_SYNC_ROBOT_FOOT_WORLD_Y: f32 = 0.05;

pub(in crate::game::features::robot_sync) fn robot_sync_axis_sync_units_from_fixed(
    value: i32,
) -> f64 {
    f64::from(value) / f64::from(FIXED_UNIT)
}

pub(in crate::game) fn robot_sync_axis_world_units_from_sync(value: f32) -> f32 {
    value * ROBOT_SYNC_WORLD_UNITS_PER_SYNC_UNIT
}

pub(in crate::game) fn robot_sync_world_position_from_sync(sync_x: f32, sync_y: f32) -> Vec2 {
    Vec2::new(
        robot_sync_axis_world_units_from_sync(sync_x),
        robot_sync_axis_world_units_from_sync(sync_y),
    )
}

pub(in crate::game::features::robot_sync) fn robot_sync_axis_world_units_from_fixed(
    value: i32,
) -> f64 {
    robot_sync_axis_sync_units_from_fixed(value) * f64::from(ROBOT_SYNC_WORLD_UNITS_PER_SYNC_UNIT)
}

pub(in crate::game::features::robot_sync) fn robot_sync_world_position_from_fixed(
    position: FixedPosition,
) -> Vec3 {
    Vec3::new(
        robot_sync_axis_world_units_from_fixed(position.x) as f32,
        ROBOT_SYNC_ROBOT_FOOT_WORLD_Y,
        robot_sync_axis_world_units_from_fixed(position.y) as f32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn robot_sync_world_position_maps_fixed_xy_to_scaled_world_xz() {
        let world = robot_sync_world_position_from_fixed(FixedPosition {
            x: 123_000,
            y: -45_000,
        });

        assert_eq!(ROBOT_SYNC_WORLD_UNITS_PER_SYNC_UNIT, 0.1);
        assert_eq!(world, Vec3::new(12.3, ROBOT_SYNC_ROBOT_FOOT_WORLD_Y, -4.5));
    }

    #[test]
    fn robot_sync_axis_helpers_keep_sync_units_separate_from_world_units() {
        assert_eq!(robot_sync_axis_sync_units_from_fixed(10_240), 10.24);
        assert!((robot_sync_axis_world_units_from_fixed(10_240) - 1.024).abs() < 0.000_001);
        assert_eq!(robot_sync_axis_world_units_from_sync(50.0), 5.0);
        assert_eq!(
            robot_sync_world_position_from_sync(-120.0, 40.0),
            Vec2::new(-12.0, 4.0)
        );
    }
}
