use bevy::prelude::*;

use crate::framework::scene::prelude::{SceneId, SceneSessionId};

#[derive(Clone, Debug, Default, Resource, PartialEq, Eq)]
pub(in crate::game) struct LockstepSimSceneState {
    pub(in crate::game::features::lockstep_sim) active: bool,
    pub(in crate::game::features::lockstep_sim) session_id: Option<SceneSessionId>,
    pub(in crate::game::features::lockstep_sim) scene_id: Option<SceneId>,
}

impl LockstepSimSceneState {
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

    pub(in crate::game::features::lockstep_sim) fn reset(&mut self) {
        *self = Self::default();
    }
}
