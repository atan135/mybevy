use sim_core::{SimEntity, SimHash, SimWorld};

use super::replay::SimHashEnvelope;

const ENTITY_SUMMARY_LIMIT: usize = 6;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) struct LockstepSimDiagnosticsState {
    pub(in crate::game::features::lockstep_sim) first_mismatch:
        Option<LockstepSimMismatchDiagnostic>,
    pub(in crate::game::features::lockstep_sim) last_match_status: LockstepSimHashMatchStatus,
    pub(in crate::game::features::lockstep_sim) rollback_count: u32,
}

impl LockstepSimDiagnosticsState {
    pub(in crate::game::features::lockstep_sim) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(in crate::game::features::lockstep_sim) fn record_frame(
        &mut self,
        frame: u32,
        local_hash: SimHash,
        server_hash: Option<&SimHashEnvelope>,
        world: &SimWorld,
    ) -> LockstepSimHashMatchStatus {
        let status = hash_match_status(local_hash, server_hash);
        if matches!(status, LockstepSimHashMatchStatus::Mismatch) && self.first_mismatch.is_none() {
            self.first_mismatch = Some(LockstepSimMismatchDiagnostic {
                frame,
                local_hash: format_sim_hash(local_hash),
                server_hash: server_hash
                    .map(format_server_hash)
                    .unwrap_or_else(|| "none".to_string()),
                entity_summary: summarize_world_entities(world),
            });
        }
        self.last_match_status = status;
        status
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) struct LockstepSimMismatchDiagnostic {
    pub(in crate::game::features::lockstep_sim) frame: u32,
    pub(in crate::game::features::lockstep_sim) local_hash: String,
    pub(in crate::game::features::lockstep_sim) server_hash: String,
    pub(in crate::game::features::lockstep_sim) entity_summary: String,
}

impl LockstepSimMismatchDiagnostic {
    pub(in crate::game::features::lockstep_sim) fn summary(&self) -> String {
        format!(
            "first_mismatch frame={} local={} server={} entities={}",
            self.frame, self.local_hash, self.server_hash, self.entity_summary
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) enum LockstepSimHashMatchStatus {
    #[default]
    Pending,
    Matched,
    Mismatch,
    NoServerHash,
}

impl LockstepSimHashMatchStatus {
    pub(in crate::game::features::lockstep_sim) fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Matched => "matched",
            Self::Mismatch => "mismatch",
            Self::NoServerHash => "no-server-hash",
        }
    }
}

pub(in crate::game::features::lockstep_sim) fn hash_match_status(
    local_hash: SimHash,
    server_hash: Option<&SimHashEnvelope>,
) -> LockstepSimHashMatchStatus {
    let Some(server_hash) = server_hash else {
        return LockstepSimHashMatchStatus::NoServerHash;
    };
    if server_hash.frame == local_hash.frame.raw() && server_hash.value == local_hash.value {
        LockstepSimHashMatchStatus::Matched
    } else {
        LockstepSimHashMatchStatus::Mismatch
    }
}

pub(in crate::game::features::lockstep_sim) fn format_sim_hash(hash: SimHash) -> String {
    format!("{}:{:016x}", hash.frame.raw(), hash.value)
}

pub(in crate::game::features::lockstep_sim) fn format_server_hash(
    hash: &SimHashEnvelope,
) -> String {
    format!("{}:{}", hash.frame, hash.hex)
}

pub(in crate::game::features::lockstep_sim) fn summarize_world_entities(
    world: &SimWorld,
) -> String {
    if world.entities_sorted_by_id().is_empty() {
        return "none".to_string();
    }

    let mut parts = world
        .entities_sorted_by_id()
        .iter()
        .take(ENTITY_SUMMARY_LIMIT)
        .map(summarize_entity)
        .collect::<Vec<_>>();
    if world.entities_sorted_by_id().len() > ENTITY_SUMMARY_LIMIT {
        parts.push(format!(
            "...+{}",
            world.entities_sorted_by_id().len() - ENTITY_SUMMARY_LIMIT
        ));
    }
    parts.join(" | ")
}

fn summarize_entity(entity: &SimEntity) -> String {
    format!(
        "id={} kind={:?} owner={} pos=({}, {}) hp={}/{} alive={}",
        entity.id.raw(),
        entity.kind,
        entity.owner_character_id.as_deref().unwrap_or("-"),
        entity.transform.pos.x.raw(),
        entity.transform.pos.y.raw(),
        entity.combat.hp,
        entity.combat.max_hp,
        entity.alive
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::{
        CombatState, EntityId, EntityKind, Fp, FrameId, MovementState, QuantizedDir, SimEntity,
        SimHash, SimTransform, SimWorld, TeamId, Vec2Fp,
    };

    #[test]
    fn diagnostics_records_first_mismatch_with_entity_summary() {
        let mut diagnostics = LockstepSimDiagnosticsState::default();
        let world = world_fixture();
        let local_hash = SimHash {
            frame: FrameId::new(3),
            value: 0xabc,
        };
        let server_hash = SimHashEnvelope {
            frame: 3,
            value: 0xdef,
            hex: "0000000000000def".to_string(),
        };

        let status = diagnostics.record_frame(3, local_hash, Some(&server_hash), &world);

        assert_eq!(status, LockstepSimHashMatchStatus::Mismatch);
        let mismatch = diagnostics.first_mismatch.as_ref().unwrap();
        assert_eq!(mismatch.frame, 3);
        assert_eq!(mismatch.local_hash, "3:0000000000000abc");
        assert_eq!(mismatch.server_hash, "3:0000000000000def");
        assert!(mismatch.entity_summary.contains("id=10"));
        assert!(mismatch.entity_summary.contains("owner=player-a"));
        assert!(mismatch.entity_summary.contains("hp=42/100"));
    }

    #[test]
    fn diagnostics_does_not_report_missing_server_hash_as_mismatch() {
        let mut diagnostics = LockstepSimDiagnosticsState::default();

        let status = diagnostics.record_frame(
            1,
            SimHash {
                frame: FrameId::new(1),
                value: 1,
            },
            None,
            &world_fixture(),
        );

        assert_eq!(status, LockstepSimHashMatchStatus::NoServerHash);
        assert!(diagnostics.first_mismatch.is_none());
    }

    fn world_fixture() -> SimWorld {
        SimWorld::new(
            FrameId::new(2),
            vec![SimEntity {
                id: EntityId::new(10),
                kind: EntityKind::Player,
                owner_character_id: Some("player-a".to_string()),
                team_id: TeamId::new(1),
                transform: SimTransform {
                    pos: Vec2Fp::new(Fp::from_i32(7), Fp::from_i32(-2)),
                    facing: QuantizedDir::RIGHT,
                    radius: Fp::from_milli(500),
                },
                movement: MovementState::default(),
                combat: CombatState {
                    hp: 42,
                    max_hp: 100,
                    ..Default::default()
                },
                alive: true,
            }],
        )
        .unwrap()
    }
}
