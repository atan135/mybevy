use bevy::prelude::*;

pub(in crate::game) const LOCKSTEP_SIM_ARENA_SCENE_ID: &str = "arena.lockstep_sim";

pub(super) struct LockstepSimArenaPlugin;

impl Plugin for LockstepSimArenaPlugin {
    fn build(&self, _app: &mut App) {}
}
