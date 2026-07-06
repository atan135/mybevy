use bevy::prelude::*;

use crate::framework::scene::prelude::{SceneId, SceneSessionId};

use super::snapshot::{LockstepSimSnapshotError, ParsedInitialSnapshot};

#[derive(Clone, Debug, Default, Resource, PartialEq, Eq)]
pub(in crate::game) struct LockstepSimSceneState {
    pub(in crate::game::features::lockstep_sim) active: bool,
    pub(in crate::game::features::lockstep_sim) session_id: Option<SceneSessionId>,
    pub(in crate::game::features::lockstep_sim) scene_id: Option<SceneId>,
    pub(in crate::game::features::lockstep_sim) initial_snapshot: Option<ParsedInitialSnapshot>,
    pub(in crate::game::features::lockstep_sim) initial_snapshot_error:
        Option<LockstepSimSnapshotError>,
    pub(in crate::game::features::lockstep_sim) snapshot_generation: u64,
}

impl LockstepSimSceneState {
    pub(in crate::game) fn is_active(&self) -> bool {
        self.active
    }

    pub(in crate::game::features::lockstep_sim) fn activate(
        &mut self,
        scene_id: SceneId,
        session_id: SceneSessionId,
    ) {
        self.active = true;
        self.scene_id = Some(scene_id);
        self.session_id = Some(session_id);
    }

    pub(in crate::game::features::lockstep_sim) fn is_active_session(
        &self,
        session_id: &SceneSessionId,
    ) -> bool {
        self.active
            && self
                .session_id
                .as_ref()
                .is_some_and(|active| active == session_id)
    }

    pub(in crate::game::features::lockstep_sim) fn replace_initial_snapshot(
        &mut self,
        snapshot: ParsedInitialSnapshot,
    ) -> bool {
        if self.initial_snapshot.as_ref() == Some(&snapshot)
            && self.initial_snapshot_error.is_none()
        {
            return false;
        }

        self.initial_snapshot = Some(snapshot);
        self.initial_snapshot_error = None;
        self.snapshot_generation = self.snapshot_generation.saturating_add(1).max(1);
        true
    }

    pub(in crate::game::features::lockstep_sim) fn reject_initial_snapshot(
        &mut self,
        error: LockstepSimSnapshotError,
    ) {
        self.initial_snapshot = None;
        self.initial_snapshot_error = Some(error);
        self.snapshot_generation = self.snapshot_generation.saturating_add(1).max(1);
    }

    pub(in crate::game::features::lockstep_sim) fn clear_initial_snapshot(&mut self) {
        if self.initial_snapshot.is_some() || self.initial_snapshot_error.is_some() {
            self.initial_snapshot = None;
            self.initial_snapshot_error = None;
            self.snapshot_generation = self.snapshot_generation.saturating_add(1).max(1);
        }
    }

    pub(in crate::game::features::lockstep_sim) fn reset(&mut self) {
        *self = Self::default();
    }
}
