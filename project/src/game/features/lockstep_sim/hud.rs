use crate::game::authority::{AuthorityRole, AuthoritySession};

use super::{
    config::{LockstepSimAuthorityMode, LockstepSimConfig},
    diagnostics::{LockstepSimDiagnosticsState, LockstepSimHashMatchStatus, format_sim_hash},
    replay::LockstepSimReplayState,
    state::LockstepSimSceneState,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game) struct LockstepSimHudSnapshot {
    pub(in crate::game) room: String,
    pub(in crate::game) policy: String,
    pub(in crate::game) player: String,
    pub(in crate::game) authority_status: String,
    pub(in crate::game) frame: String,
    pub(in crate::game) fps: String,
    pub(in crate::game) entity_count: usize,
    pub(in crate::game) event_count: usize,
    pub(in crate::game) local_hash: String,
    pub(in crate::game) server_hash: String,
    pub(in crate::game) mismatch: String,
    pub(in crate::game) rollback: String,
    pub(in crate::game) first_mismatch: String,
}

pub(in crate::game) fn lockstep_sim_hud_snapshot(
    config: &LockstepSimConfig,
    scene_state: &LockstepSimSceneState,
    authority_session: &AuthoritySession,
    replay_state: &LockstepSimReplayState,
) -> LockstepSimHudSnapshot {
    let latest_hash = replay_state.hash_history.back();
    let diagnostics = &replay_state.diagnostics;
    let player = authority_session
        .local_player_id
        .as_deref()
        .unwrap_or(config.local_player_id.as_str())
        .to_string();
    let tick_rate = scene_state
        .initial_snapshot
        .as_ref()
        .map(|snapshot| snapshot.tick_rate)
        .or_else(|| {
            replay_state
                .config
                .as_ref()
                .map(|config| config.movement.tick_rate)
        });

    LockstepSimHudSnapshot {
        room: lockstep_sim_room_label(config, scene_state),
        policy: config.myserver_policy_id.clone(),
        player,
        authority_status: lockstep_sim_authority_status(
            config.authority_mode,
            scene_state.active,
            authority_session.role,
        ),
        frame: lockstep_sim_frame_label(
            replay_state.last_applied_frame,
            authority_session.frame_id,
        ),
        fps: lockstep_sim_fps_label(tick_rate, authority_session.fps),
        entity_count: replay_state
            .world
            .as_ref()
            .map(|world| world.entities_sorted_by_id().len())
            .unwrap_or(0),
        event_count: replay_state
            .event_history
            .back()
            .map(|events| events.events.len())
            .unwrap_or(0),
        local_hash: latest_hash
            .map(|hash| format_sim_hash(hash.local_hash))
            .unwrap_or_else(|| "pending".to_string()),
        server_hash: latest_hash
            .and_then(|hash| hash.server_hash.as_ref())
            .map(super::diagnostics::format_server_hash)
            .unwrap_or_else(|| "pending".to_string()),
        mismatch: mismatch_label(diagnostics),
        rollback: format!("rollback={}", diagnostics.rollback_count),
        first_mismatch: diagnostics
            .first_mismatch
            .as_ref()
            .map(|mismatch| mismatch.summary())
            .unwrap_or_else(|| "first_mismatch=none".to_string()),
    }
}

pub(in crate::game) fn format_lockstep_sim_hud_status(snapshot: &LockstepSimHudSnapshot) -> String {
    format!(
        "room={} policy={} player={} authority={} frame={} fps={} entities={} events={}\nlocal_hash={} server_hash={} mismatch={} {}\n{}",
        snapshot.room,
        snapshot.policy,
        snapshot.player,
        snapshot.authority_status,
        snapshot.frame,
        snapshot.fps,
        snapshot.entity_count,
        snapshot.event_count,
        snapshot.local_hash,
        snapshot.server_hash,
        snapshot.mismatch,
        snapshot.rollback,
        snapshot.first_mismatch
    )
}

fn lockstep_sim_room_label(
    config: &LockstepSimConfig,
    scene_state: &LockstepSimSceneState,
) -> String {
    scene_state
        .initial_snapshot
        .as_ref()
        .map(|snapshot| snapshot.room_id.clone())
        .unwrap_or_else(|| match config.authority_mode {
            LockstepSimAuthorityMode::MyServer => config.myserver_room_id.clone(),
            LockstepSimAuthorityMode::Off => "off".to_string(),
        })
}

fn lockstep_sim_authority_status(
    mode: LockstepSimAuthorityMode,
    scene_active: bool,
    role: Option<AuthorityRole>,
) -> String {
    let scene = if scene_active { "active" } else { "inactive" };
    let role = match role {
        Some(AuthorityRole::Host) => "host",
        Some(AuthorityRole::Client) => "client",
        Some(AuthorityRole::None) => "none",
        None => "pending",
    };
    format!("{mode:?}/{scene}/{role}")
}

fn lockstep_sim_frame_label(last_applied_frame: Option<u32>, authority_frame: u32) -> String {
    match last_applied_frame {
        Some(frame) => frame.to_string(),
        None => format!("pending(authority={authority_frame})"),
    }
}

fn lockstep_sim_fps_label(tick_rate: Option<u16>, authority_fps: u16) -> String {
    match tick_rate {
        Some(tick_rate) => format!("tick={tick_rate}/authority={authority_fps}"),
        None => format!("tick=pending/authority={authority_fps}"),
    }
}

fn mismatch_label(diagnostics: &LockstepSimDiagnosticsState) -> String {
    match diagnostics.last_match_status {
        LockstepSimHashMatchStatus::Pending if diagnostics.first_mismatch.is_none() => {
            "pending".to_string()
        }
        status => status.as_str().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        framework::{network::NetworkTransport, scene::prelude::SceneId},
        game::{
            authority::AuthoritySession,
            features::lockstep_sim::{
                config::LOCKSTEP_SIM_MYSERVER_POLICY_ID,
                diagnostics::LockstepSimMismatchDiagnostic,
                replay::{LockstepSimFrameHash, SimHashEnvelope},
            },
            scenes::LOCKSTEP_SIM_ARENA_SCENE_ID,
        },
    };
    use sim_core::{FrameId, SimHash};

    #[test]
    fn lockstep_hud_formats_hash_and_mismatch_fields() {
        let mut diagnostics = LockstepSimDiagnosticsState::default();
        diagnostics.last_match_status = LockstepSimHashMatchStatus::Mismatch;
        diagnostics.first_mismatch = Some(LockstepSimMismatchDiagnostic {
            frame: 7,
            local_hash: "7:0000000000000001".to_string(),
            server_hash: "7:0000000000000002".to_string(),
            entity_summary: "id=1 kind=Player owner=player-a pos=(0, 0) hp=1/1 alive=true"
                .to_string(),
        });
        let snapshot = LockstepSimHudSnapshot {
            room: "room-a".to_string(),
            policy: "policy-a".to_string(),
            player: "player-a".to_string(),
            authority_status: "MyServer/active/client".to_string(),
            frame: "7".to_string(),
            fps: "tick=20/authority=20".to_string(),
            entity_count: 2,
            event_count: 3,
            local_hash: "7:0000000000000001".to_string(),
            server_hash: "7:0000000000000002".to_string(),
            mismatch: mismatch_label(&diagnostics),
            rollback: "rollback=0".to_string(),
            first_mismatch: diagnostics.first_mismatch.as_ref().unwrap().summary(),
        };

        let status = format_lockstep_sim_hud_status(&snapshot);

        assert!(status.contains("room=room-a policy=policy-a player=player-a"));
        assert!(status.contains("entities=2 events=3"));
        assert!(status.contains("local_hash=7:0000000000000001"));
        assert!(status.contains("server_hash=7:0000000000000002"));
        assert!(status.contains("mismatch=mismatch rollback=0"));
        assert!(status.contains("first_mismatch frame=7"));
    }

    #[test]
    fn lockstep_hud_snapshot_reports_no_server_hash_without_mismatch() {
        let config = test_config();
        let mut scene_state = LockstepSimSceneState::default();
        scene_state.active = true;
        let mut authority_session = AuthoritySession::default();
        authority_session.local_player_id = Some("player-a".to_string());
        authority_session.frame_id = 11;
        authority_session.fps = 20;
        let mut replay_state = LockstepSimReplayState::default();
        replay_state.last_applied_frame = Some(10);
        replay_state.diagnostics.last_match_status = LockstepSimHashMatchStatus::NoServerHash;
        replay_state.hash_history.push_back(LockstepSimFrameHash {
            frame: 10,
            local_hash: SimHash {
                frame: FrameId::new(10),
                value: 0x1234,
            },
            server_hash: None,
            event_count: 0,
        });

        let snapshot =
            lockstep_sim_hud_snapshot(&config, &scene_state, &authority_session, &replay_state);

        assert_eq!(snapshot.room, "lockstep-room");
        assert_eq!(snapshot.policy, LOCKSTEP_SIM_MYSERVER_POLICY_ID);
        assert_eq!(snapshot.frame, "10");
        assert_eq!(snapshot.local_hash, "10:0000000000001234");
        assert_eq!(snapshot.server_hash, "pending");
        assert_eq!(snapshot.mismatch, "no-server-hash");
        assert_eq!(snapshot.rollback, "rollback=0");
    }

    #[test]
    fn lockstep_hud_snapshot_formats_matching_server_hash() {
        let config = test_config();
        let scene_state = LockstepSimSceneState::default();
        let mut replay_state = LockstepSimReplayState::default();
        replay_state.diagnostics.last_match_status = LockstepSimHashMatchStatus::Matched;
        replay_state.hash_history.push_back(LockstepSimFrameHash {
            frame: 2,
            local_hash: SimHash {
                frame: FrameId::new(2),
                value: 0xbeef,
            },
            server_hash: Some(SimHashEnvelope {
                frame: 2,
                value: 0xbeef,
                hex: "000000000000beef".to_string(),
            }),
            event_count: 1,
        });

        let snapshot = lockstep_sim_hud_snapshot(
            &config,
            &scene_state,
            &AuthoritySession::default(),
            &replay_state,
        );

        assert_eq!(snapshot.local_hash, "2:000000000000beef");
        assert_eq!(snapshot.server_hash, "2:000000000000beef");
        assert_eq!(snapshot.mismatch, "matched");
    }

    fn test_config() -> LockstepSimConfig {
        LockstepSimConfig {
            scene_id: SceneId::from(LOCKSTEP_SIM_ARENA_SCENE_ID),
            local_player_id: "player-a".to_string(),
            authority_mode: LockstepSimAuthorityMode::MyServer,
            transport: NetworkTransport::Tcp,
            myserver_guest_id: None,
            myserver_room_id: "lockstep-room".to_string(),
            myserver_policy_id: LOCKSTEP_SIM_MYSERVER_POLICY_ID.to_string(),
            debug_diagnostics: false,
        }
    }
}
