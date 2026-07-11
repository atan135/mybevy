use std::{
    collections::{HashMap, VecDeque},
    env, fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use bevy::{app::AppExit, prelude::*};
use serde_json::json;
use sim_core::{
    BuffDefinition, BuffId, CastSkillCommand, CombatConfig, CombatEffect, CombatState,
    DamageFormula, EntityId, EntityKind, Fp, FrameId, MoveCommand, MovementConfig, MovementState,
    QuantizedDir, SceneBounds, SimCommand, SimConfig, SimEntity, SimEvent, SimInput,
    SimInputSource, SimRngState, SimTransform, SimWorld, SkillDefinition, SkillId, SkillSlot,
    SkillTarget, SkillTargetType, TeamId, Vec2Fp, hash_world, step,
};

use crate::{
    framework::ui::audit::{UiScreenshotCommand, UiScreenshotEvent, UiScreenshotSaved},
    game::{
        authority::{AuthorityCommand, AuthoritySession},
        myserver::{MyServerCommand, MyServerEvent, MyServerSession},
        navigation::{AppUiMode, GameRouteCommand},
    },
};

use super::{
    combat_events::{LockstepSimCombatEventKind, LockstepSimCombatEventState},
    config::LockstepSimConfig,
    hud::{format_lockstep_sim_hud_status, lockstep_sim_hud_snapshot},
    payload::{build_sim_input_envelope, gate_lockstep_sim_input},
    replay::{
        LockstepSimFrameEvents, LockstepSimFrameHash, LockstepSimReplayState,
        LockstepSimWorldSnapshot,
    },
    snapshot::{ParsedInitialSnapshot, SimHashEnvelope},
    state::LockstepSimSceneState,
    sync::LockstepSimMyServerJoinState,
    visual::LockstepSimVisualState,
};

const VISUAL_SMOKE_SKILL_ID: SkillId = SkillId::new(1);
const VISUAL_SMOKE_TARGET_ID: EntityId = EntityId::new(9000);
const VISUAL_SMOKE_SETTLE_FRAMES: u32 = 30;
const VISUAL_SMOKE_MAX_INPUT_ATTEMPTS: u32 = 8;
// Matches MyServer's lockstep_sim_demo input_delay_frames policy window.
const VISUAL_SMOKE_INPUT_LEAD_FRAMES: u32 = 2;
const OFFLINE_FIXTURE_START_FRAME: u32 = 100;
const OFFLINE_FIXTURE_SKILL_ID: SkillId = SkillId::new(500);
const OFFLINE_FIXTURE_BUFF_ID: BuffId = BuffId::new(900);

pub(in crate::game) struct LockstepSimVisualSmokePlugin;

impl Plugin for LockstepSimVisualSmokePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LockstepSimVisualSmokeConfig>()
            .init_resource::<LockstepSimVisualSmokeState>()
            .add_systems(
                Update,
                (
                    keep_visual_smoke_gameplay_ui,
                    cleanup_stale_visual_smoke_combat_entities,
                    drive_lockstep_sim_visual_smoke,
                )
                    .chain()
                    .run_if(visual_smoke_enabled),
            );
    }
}

#[derive(Clone, Debug, Resource, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) struct LockstepSimVisualSmokeConfig {
    pub(in crate::game::features::lockstep_sim) enabled: bool,
    run_id: String,
    screenshot_path: Option<PathBuf>,
    report_path: Option<PathBuf>,
    offline_screenshot_path: Option<PathBuf>,
    offline_report_path: Option<PathBuf>,
    timeout: Duration,
}

impl Default for LockstepSimVisualSmokeConfig {
    fn default() -> Self {
        Self::from_env_reader(|name| env::var(name).ok())
    }
}

impl LockstepSimVisualSmokeConfig {
    fn from_env_reader(mut read: impl FnMut(&str) -> Option<String>) -> Self {
        Self {
            enabled: read_bool(&mut read, "LOCKSTEP_SIM_VISUAL_SMOKE"),
            run_id: read_non_empty(&mut read, "LOCKSTEP_SIM_VISUAL_SMOKE_RUN_ID")
                .unwrap_or_else(|| "lockstep-visual-smoke".to_string()),
            screenshot_path: read_non_empty(&mut read, "LOCKSTEP_SIM_VISUAL_SMOKE_SCREENSHOT")
                .map(PathBuf::from),
            report_path: read_non_empty(&mut read, "LOCKSTEP_SIM_VISUAL_SMOKE_REPORT")
                .map(PathBuf::from),
            offline_screenshot_path: read_non_empty(
                &mut read,
                "LOCKSTEP_SIM_VISUAL_SMOKE_OFFLINE_SCREENSHOT",
            )
            .map(PathBuf::from),
            offline_report_path: read_non_empty(
                &mut read,
                "LOCKSTEP_SIM_VISUAL_SMOKE_OFFLINE_REPORT",
            )
            .map(PathBuf::from),
            timeout: Duration::from_millis(
                read_non_empty(&mut read, "LOCKSTEP_SIM_VISUAL_SMOKE_TIMEOUT_MS")
                    .and_then(|value| value.parse::<u64>().ok())
                    .filter(|value| *value > 0)
                    .unwrap_or(30_000),
            ),
        }
    }
}

fn read_non_empty(read: &mut impl FnMut(&str) -> Option<String>, name: &str) -> Option<String> {
    read(name)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_bool(read: &mut impl FnMut(&str) -> Option<String>, name: &str) -> bool {
    matches!(
        read_non_empty(read, name)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum VisualSmokeCleanupPhase {
    #[default]
    None,
    EndingRoom,
    LeavingRoom,
    Disconnecting,
}

#[derive(Resource, Debug)]
struct LockstepSimVisualSmokeState {
    started_at: Instant,
    ui_mode: AppUiMode,
    input_frame: Option<u32>,
    input_accepted: bool,
    input_attempts: u32,
    input_retry_after_frame: Option<u32>,
    stop_frame: Option<u32>,
    stop_accepted: bool,
    stop_attempts: u32,
    stop_retry_after_frame: Option<u32>,
    settle_frames: u32,
    screenshot_requested: bool,
    screenshot: Option<UiScreenshotSaved>,
    offline_fixture_injected: bool,
    offline_screenshot_requested: bool,
    offline_screenshot: Option<UiScreenshotSaved>,
    offline_report_written: bool,
    stale_combat_visuals: Vec<Entity>,
    cleanup_phase: VisualSmokeCleanupPhase,
    report_written: bool,
    failure: Option<String>,
}

impl Default for LockstepSimVisualSmokeState {
    fn default() -> Self {
        Self {
            started_at: Instant::now(),
            ui_mode: AppUiMode::Login,
            input_frame: None,
            input_accepted: false,
            input_attempts: 0,
            input_retry_after_frame: None,
            stop_frame: None,
            stop_accepted: false,
            stop_attempts: 0,
            stop_retry_after_frame: None,
            settle_frames: 0,
            screenshot_requested: false,
            screenshot: None,
            offline_fixture_injected: false,
            offline_screenshot_requested: false,
            offline_screenshot: None,
            offline_report_written: false,
            stale_combat_visuals: Vec::new(),
            cleanup_phase: VisualSmokeCleanupPhase::None,
            report_written: false,
            failure: None,
        }
    }
}

fn visual_smoke_enabled(config: Res<LockstepSimVisualSmokeConfig>) -> bool {
    config.enabled
}

fn keep_visual_smoke_gameplay_ui(
    scene_state: Res<LockstepSimSceneState>,
    ui_mode: Res<State<AppUiMode>>,
    mut state: ResMut<LockstepSimVisualSmokeState>,
    mut route_commands: MessageWriter<GameRouteCommand>,
) {
    state.ui_mode = *ui_mode.get();
    if state.failure.is_none() && scene_state.active && state.ui_mode != AppUiMode::RobotSyncScene {
        route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::RobotSyncScene));
    }
}

fn cleanup_stale_visual_smoke_combat_entities(
    mut commands: Commands,
    mut state: ResMut<LockstepSimVisualSmokeState>,
) {
    for entity in state.stale_combat_visuals.drain(..) {
        commands.entity(entity).despawn();
    }
}

fn apply_visual_smoke_input_response(
    state: &mut LockstepSimVisualSmokeState,
    accepted: bool,
    error_code: &str,
) {
    if accepted {
        if state.stop_frame.is_some() && !state.stop_accepted {
            state.stop_accepted = true;
        } else if state.input_frame.is_some() && !state.input_accepted {
            state.input_accepted = true;
        }
        return;
    }

    if error_code != "INPUT_FRAME_EXPIRED" {
        state.failure = Some(format!("player input rejected: {error_code}"));
        return;
    }

    state.settle_frames = 0;
    if state.stop_frame.is_some() && !state.stop_accepted {
        if state.stop_attempts >= VISUAL_SMOKE_MAX_INPUT_ATTEMPTS {
            state.failure = Some(format!(
                "stop input remained expired after {} attempts",
                state.stop_attempts
            ));
        } else {
            state.stop_retry_after_frame = state.stop_frame;
            state.stop_frame = None;
        }
    } else if state.input_frame.is_some() && !state.input_accepted {
        if state.input_attempts >= VISUAL_SMOKE_MAX_INPUT_ATTEMPTS {
            state.failure = Some(format!(
                "move/skill input remained expired after {} attempts",
                state.input_attempts
            ));
        } else {
            state.input_retry_after_frame = state.input_frame;
            state.input_frame = None;
        }
    } else {
        state.failure = Some("received expired response with no pending visual input".to_string());
    }
}

#[allow(clippy::too_many_arguments)]
fn drive_lockstep_sim_visual_smoke(
    config: Res<LockstepSimVisualSmokeConfig>,
    lockstep_config: Res<LockstepSimConfig>,
    mut scene_state: ResMut<LockstepSimSceneState>,
    join_state: Res<LockstepSimMyServerJoinState>,
    myserver_session: Res<MyServerSession>,
    authority: Res<AuthoritySession>,
    mut replay: ResMut<LockstepSimReplayState>,
    visual: Res<LockstepSimVisualState>,
    mut combat: ResMut<LockstepSimCombatEventState>,
    mut state: ResMut<LockstepSimVisualSmokeState>,
    mut authority_commands: MessageWriter<AuthorityCommand>,
    mut myserver_commands: MessageWriter<MyServerCommand>,
    mut myserver_events: MessageReader<MyServerEvent>,
    mut screenshot_commands: MessageWriter<UiScreenshotCommand>,
    mut screenshot_events: MessageReader<UiScreenshotEvent>,
    mut app_exit: MessageWriter<AppExit>,
) {
    for event in screenshot_events.read() {
        match event {
            UiScreenshotEvent::Saved(saved) => {
                if config
                    .screenshot_path
                    .as_ref()
                    .is_some_and(|path| saved.request.path == *path)
                {
                    state.screenshot = Some(saved.clone());
                } else if config
                    .offline_screenshot_path
                    .as_ref()
                    .is_some_and(|path| saved.request.path == *path)
                {
                    state.offline_screenshot = Some(saved.clone());
                }
            }
            UiScreenshotEvent::Failed(failed) => {
                if config
                    .screenshot_path
                    .as_ref()
                    .is_some_and(|path| failed.request.path == *path)
                {
                    state.failure = Some(format!("screenshot failed: {}", failed.reason));
                } else if config
                    .offline_screenshot_path
                    .as_ref()
                    .is_some_and(|path| failed.request.path == *path)
                {
                    state.failure = Some(format!(
                        "offline fixture screenshot failed: {}",
                        failed.reason
                    ));
                }
            }
        }
    }

    let mut room_ended = false;
    let mut room_left = false;
    let mut disconnected = false;
    for event in myserver_events.read() {
        match event {
            MyServerEvent::RoomEnded(response) => {
                room_ended = true;
                if !response.ok {
                    state.failure = Some(format!("room end rejected: {}", response.error_code));
                }
            }
            MyServerEvent::RoomLeft(response) => {
                room_left = true;
                if !response.ok {
                    state.failure = Some(format!("room leave rejected: {}", response.error_code));
                }
            }
            MyServerEvent::Disconnected { .. } => disconnected = true,
            MyServerEvent::ConnectionFailed { error, .. } => {
                state.failure = Some(format!("connection failed: {error}"));
            }
            MyServerEvent::AuthFailed { error_code }
            | MyServerEvent::GameAuthRejected { error_code, .. } => {
                state.failure = Some(format!("authentication rejected: {error_code}"));
            }
            MyServerEvent::RoomJoined(response) if !response.ok => {
                state.failure = Some(format!("room join rejected: {}", response.error_code));
            }
            MyServerEvent::ReadyChanged(response) if !response.ok => {
                state.failure = Some(format!("room ready rejected: {}", response.error_code));
            }
            MyServerEvent::RoomStarted(response) if !response.ok => {
                state.failure = Some(format!("room start rejected: {}", response.error_code));
            }
            MyServerEvent::PlayerInputAccepted(response) => {
                apply_visual_smoke_input_response(&mut state, response.ok, &response.error_code);
            }
            MyServerEvent::ProtocolError { error } => {
                state.failure = Some(format!("protocol error: {error}"));
            }
            _ => {}
        }
    }

    if state.started_at.elapsed() >= config.timeout && state.failure.is_none() {
        state.failure = Some("visual smoke timed out".to_string());
    }
    if state.failure.is_none()
        && let Some(error) = scene_state.initial_snapshot_error.as_ref()
    {
        state.failure = Some(format!("initial snapshot rejected: {error}"));
    }
    if state.failure.is_none()
        && scene_state.initial_snapshot.is_some()
        && let Some(error) = replay.last_error.as_ref()
    {
        state.failure = Some(format!("local replay failed: {error}"));
    }

    if state.failure.is_some() && !state.report_written {
        if let Err(error) = write_visual_smoke_report(
            &config,
            &lockstep_config,
            &scene_state,
            &authority,
            &replay,
            &visual,
            &combat,
            state.ui_mode,
            state.screenshot.as_ref(),
            state.failure.as_deref(),
        ) {
            state.failure = Some(format!("visual report write failed: {error}"));
        }
        state.report_written = true;
    }

    match state.cleanup_phase {
        VisualSmokeCleanupPhase::EndingRoom if room_ended => {
            myserver_commands.write(MyServerCommand::LeaveRoom);
            state.cleanup_phase = VisualSmokeCleanupPhase::LeavingRoom;
            return;
        }
        VisualSmokeCleanupPhase::LeavingRoom if room_left => {
            authority_commands.write(AuthorityCommand::Leave);
            myserver_commands.write(MyServerCommand::Disconnect);
            state.cleanup_phase = VisualSmokeCleanupPhase::Disconnecting;
            return;
        }
        VisualSmokeCleanupPhase::Disconnecting
            if disconnected || myserver_session.connection_id.is_none() =>
        {
            if state.failure.is_some() {
                app_exit.write(AppExit::error());
                return;
            }
            let player_id = authority
                .local_player_id
                .clone()
                .unwrap_or_else(|| "offline-visual-player".to_string());
            state.stale_combat_visuals = combat.visual_entities.iter().copied().collect();
            if let Err(error) = inject_offline_visual_fixture(
                &mut scene_state,
                &mut replay,
                &mut combat,
                &player_id,
            ) {
                state.failure = Some(format!("offline visual fixture failed: {error}"));
                app_exit.write(AppExit::error());
                return;
            }
            state.offline_fixture_injected = true;
            state.settle_frames = 0;
            state.cleanup_phase = VisualSmokeCleanupPhase::None;
            return;
        }
        _ => {}
    }
    if state.cleanup_phase != VisualSmokeCleanupPhase::None {
        return;
    }
    if state.failure.is_none() && scene_state.active && state.ui_mode != AppUiMode::RobotSyncScene {
        return;
    }
    if state.offline_fixture_injected {
        if state.failure.is_some() {
            app_exit.write(AppExit::error());
            return;
        }
        if !offline_visual_evidence_ready(&replay, &visual, &combat) {
            return;
        }
        if state.settle_frames < VISUAL_SMOKE_SETTLE_FRAMES {
            state.settle_frames += 1;
            return;
        }
        if !state.offline_screenshot_requested {
            let Some(path) = config.offline_screenshot_path.clone() else {
                state.failure =
                    Some("LOCKSTEP_SIM_VISUAL_SMOKE_OFFLINE_SCREENSHOT is missing".to_string());
                return;
            };
            screenshot_commands.write(UiScreenshotCommand::Capture {
                path,
                label: "lockstep_sim_offline_visual_fixture".to_string(),
            });
            state.offline_screenshot_requested = true;
            return;
        }
        if state.offline_screenshot.is_some() && !state.offline_report_written {
            if let Err(error) = write_offline_visual_fixture_report(
                &config,
                &lockstep_config,
                &scene_state,
                &authority,
                &replay,
                &visual,
                &combat,
                state.ui_mode,
                state.offline_screenshot.as_ref(),
            ) {
                state.failure = Some(format!("offline visual report write failed: {error}"));
                app_exit.write(AppExit::error());
                return;
            }
            state.offline_report_written = true;
            app_exit.write(AppExit::Success);
        }
        return;
    }
    if state.failure.is_some() {
        if join_state.started {
            myserver_commands.write(MyServerCommand::EndRoom {
                reason: "mybevy-lockstep-visual-smoke-failed".to_string(),
            });
            state.cleanup_phase = VisualSmokeCleanupPhase::EndingRoom;
        } else if myserver_session.connection_id.is_some() {
            authority_commands.write(AuthorityCommand::Leave);
            myserver_commands.write(MyServerCommand::Disconnect);
            state.cleanup_phase = VisualSmokeCleanupPhase::Disconnecting;
        } else {
            app_exit.write(AppExit::error());
        }
        return;
    }

    if state.input_frame.is_none()
        && join_state.started
        && scene_state.initial_snapshot.is_some()
        && state.input_retry_after_frame.is_none_or(|frame| {
            replay
                .last_applied_frame
                .is_some_and(|applied| applied >= frame)
        })
    {
        match send_first_input(&scene_state, &authority, &replay, &mut authority_commands) {
            Ok(frame) => {
                state.input_frame = Some(frame);
                state.input_attempts = state.input_attempts.saturating_add(1);
                state.input_retry_after_frame = None;
            }
            Err(error) => state.failure = Some(error),
        }
        return;
    }
    if let Some(input_frame) = state.input_frame
        && state.input_accepted
        && state.stop_frame.is_none()
        && replay
            .last_applied_frame
            .is_some_and(|frame| frame >= input_frame)
        && state.stop_retry_after_frame.is_none_or(|frame| {
            replay
                .last_applied_frame
                .is_some_and(|applied| applied >= frame)
        })
    {
        match send_stop_input(&authority, &replay, input_frame, &mut authority_commands) {
            Ok(frame) => {
                state.stop_frame = Some(frame);
                state.stop_attempts = state.stop_attempts.saturating_add(1);
                state.stop_retry_after_frame = None;
            }
            Err(error) => state.failure = Some(error),
        }
        return;
    }
    let Some(stop_frame) = state.stop_frame else {
        return;
    };
    if !state.stop_accepted {
        return;
    }
    if replay.last_applied_frame.unwrap_or_default() < stop_frame.saturating_add(2) {
        return;
    }
    if !core_visual_evidence_ready(&replay, &visual, &combat) {
        return;
    }
    if state.settle_frames < VISUAL_SMOKE_SETTLE_FRAMES {
        state.settle_frames += 1;
        return;
    }
    if !state.screenshot_requested {
        let Some(path) = config.screenshot_path.clone() else {
            state.failure = Some("LOCKSTEP_SIM_VISUAL_SMOKE_SCREENSHOT is missing".to_string());
            return;
        };
        screenshot_commands.write(UiScreenshotCommand::Capture {
            path,
            label: "lockstep_sim_visual_smoke".to_string(),
        });
        state.screenshot_requested = true;
        return;
    }
    if state.screenshot.is_some() && !state.report_written {
        if let Err(error) = write_visual_smoke_report(
            &config,
            &lockstep_config,
            &scene_state,
            &authority,
            &replay,
            &visual,
            &combat,
            state.ui_mode,
            state.screenshot.as_ref(),
            None,
        ) {
            state.failure = Some(format!("visual report write failed: {error}"));
        }
        state.report_written = true;
        myserver_commands.write(MyServerCommand::EndRoom {
            reason: "mybevy-lockstep-visual-smoke-complete".to_string(),
        });
        state.cleanup_phase = VisualSmokeCleanupPhase::EndingRoom;
    }
}

fn send_first_input(
    scene_state: &LockstepSimSceneState,
    authority: &AuthoritySession,
    replay: &LockstepSimReplayState,
    commands: &mut MessageWriter<AuthorityCommand>,
) -> Result<u32, String> {
    let player_id = authority
        .local_player_id
        .as_deref()
        .ok_or_else(|| "authority has no gameplay character id".to_string())?;
    gate_lockstep_sim_input(
        scene_state,
        Some(player_id),
        None,
        None,
        Some(sim_core::SIM_CORE_SCHEMA_VERSION),
    )
    .map_err(|error| error.to_string())?;
    let snapshot_frame = scene_state
        .initial_snapshot
        .as_ref()
        .map(|snapshot| snapshot.start_frame)
        .unwrap_or_default();
    let frame = next_visual_input_frame(
        authority.frame_id,
        replay.last_applied_frame,
        snapshot_frame,
    );
    let input = build_sim_input_envelope(
        frame,
        1,
        &[
            SimCommand::Move(MoveCommand {
                dir: QuantizedDir::RIGHT,
                speed_per_second: Some(Fp::from_i32(6)),
            }),
            SimCommand::CastSkill(CastSkillCommand {
                skill_id: VISUAL_SMOKE_SKILL_ID,
                target: SkillTarget::Entity(VISUAL_SMOKE_TARGET_ID),
            }),
        ],
    )
    .map_err(|error| error.to_string())?;
    commands.write(input.into_authority_command());
    Ok(frame)
}

fn send_stop_input(
    authority: &AuthoritySession,
    replay: &LockstepSimReplayState,
    input_frame: u32,
    commands: &mut MessageWriter<AuthorityCommand>,
) -> Result<u32, String> {
    let frame = next_visual_input_frame(authority.frame_id, replay.last_applied_frame, input_frame);
    let input = build_sim_input_envelope(frame, 2, &[SimCommand::Stop])
        .map_err(|error| error.to_string())?;
    commands.write(input.into_authority_command());
    Ok(frame)
}

fn next_visual_input_frame(
    authority_frame: u32,
    replay_frame: Option<u32>,
    minimum_frame: u32,
) -> u32 {
    authority_frame
        .max(replay_frame.unwrap_or(minimum_frame))
        .max(minimum_frame)
        .saturating_add(VISUAL_SMOKE_INPUT_LEAD_FRAMES)
}

fn core_visual_evidence_ready(
    replay: &LockstepSimReplayState,
    visual: &LockstepSimVisualState,
    combat: &LockstepSimCombatEventState,
) -> bool {
    let event_kinds = replay
        .event_history
        .iter()
        .flat_map(|frame| frame.events.iter())
        .map(sim_event_kind)
        .collect::<Vec<_>>();
    event_kinds.contains(&"skill_cast")
        && event_kinds.contains(&"damage_applied")
        && visual.tracked_entity_count >= 2
        && combat
            .entries
            .iter()
            .any(|entry| entry.kind == LockstepSimCombatEventKind::DamageNumber)
}

fn offline_visual_evidence_ready(
    replay: &LockstepSimReplayState,
    visual: &LockstepSimVisualState,
    combat: &LockstepSimCombatEventState,
) -> bool {
    let event_kinds = replay
        .event_history
        .iter()
        .flat_map(|frame| frame.events.iter())
        .map(sim_event_kind)
        .collect::<Vec<_>>();
    event_kinds.contains(&"buff_applied")
        && event_kinds.contains(&"buff_tick")
        && event_kinds.contains(&"damage_applied")
        && event_kinds.contains(&"entity_died")
        && visual.tracked_entity_count >= 2
        && combat
            .entries
            .iter()
            .any(|entry| entry.kind == LockstepSimCombatEventKind::BuffTick)
        && combat
            .entries
            .iter()
            .any(|entry| entry.kind == LockstepSimCombatEventKind::DeathState)
}

fn inject_offline_visual_fixture(
    scene_state: &mut LockstepSimSceneState,
    replay: &mut LockstepSimReplayState,
    combat_state: &mut LockstepSimCombatEventState,
    player_id: &str,
) -> Result<(), String> {
    let combat = CombatConfig::from_definitions(
        vec![SkillDefinition {
            id: OFFLINE_FIXTURE_SKILL_ID,
            cooldown_frames: 10,
            cast_range: Fp::from_i32(12),
            target_type: SkillTargetType::Enemy,
            effects: vec![CombatEffect::AddBuff {
                buff_id: OFFLINE_FIXTURE_BUFF_ID,
            }],
        }],
        vec![BuffDefinition {
            id: OFFLINE_FIXTURE_BUFF_ID,
            duration_frames: 4,
            interval_frames: 2,
            max_stacks: 1,
            effects: vec![CombatEffect::Damage {
                formula: DamageFormula::TrueDamage { amount: 7 },
            }],
        }],
    )
    .map_err(|error| error.to_string())?;
    let config = SimConfig {
        movement: MovementConfig {
            tick_rate: 20,
            default_speed_per_second: Fp::from_i32(6),
            max_speed_per_second: Fp::from_i32(12),
            bounds: SceneBounds {
                min: Vec2Fp::new(Fp::from_i32(-100), Fp::from_i32(-100)),
                max: Vec2Fp::new(Fp::from_i32(100), Fp::from_i32(100)),
            },
            static_obstacles: Vec::new(),
        },
        combat,
    };
    let player = SimEntity {
        id: EntityId::new(1000),
        kind: EntityKind::Player,
        owner_character_id: Some(player_id.to_string()),
        team_id: TeamId::new(1),
        transform: SimTransform {
            pos: Vec2Fp::zero(),
            facing: QuantizedDir::RIGHT,
            radius: Fp::from_milli(500),
        },
        movement: MovementState::default(),
        combat: CombatState {
            hp: 100,
            max_hp: 100,
            attack: 10,
            defense: 0,
            speed: 6,
            crit_rate_bps: 0,
            crit_damage_bps: 10_000,
            skill_slots: vec![SkillSlot {
                skill_id: OFFLINE_FIXTURE_SKILL_ID,
                cooldown_remaining: 0,
            }],
            buffs: Vec::new(),
        },
        alive: true,
    };
    let target = SimEntity {
        id: VISUAL_SMOKE_TARGET_ID,
        kind: EntityKind::Monster,
        owner_character_id: None,
        team_id: TeamId::new(2),
        transform: SimTransform {
            pos: Vec2Fp::new(Fp::from_i32(8), Fp::ZERO),
            facing: QuantizedDir::LEFT,
            radius: Fp::from_milli(500),
        },
        movement: MovementState::default(),
        combat: CombatState {
            hp: 7,
            max_hp: 7,
            defense: 0,
            ..Default::default()
        },
        alive: true,
    };
    let mut world = SimWorld::with_rng(
        FrameId::new(OFFLINE_FIXTURE_START_FRAME),
        SimRngState {
            seed: 0x5649_5355_414c_534d,
            counter: 0,
        },
        vec![player, target],
    )
    .map_err(|error| error.to_string())?;
    let mut event_history = VecDeque::new();
    let mut input_history = VecDeque::new();
    for frame in OFFLINE_FIXTURE_START_FRAME + 1..=OFFLINE_FIXTURE_START_FRAME + 4 {
        let inputs = if frame == OFFLINE_FIXTURE_START_FRAME + 1 {
            vec![SimInput {
                frame: FrameId::new(frame),
                character_id: player_id.to_string(),
                entity_id: EntityId::new(1000),
                seq: 1,
                source: SimInputSource::Real,
                command: SimCommand::CastSkill(CastSkillCommand {
                    skill_id: OFFLINE_FIXTURE_SKILL_ID,
                    target: SkillTarget::Entity(VISUAL_SMOKE_TARGET_ID),
                }),
            }]
        } else {
            Vec::new()
        };
        let result = step(&mut world, FrameId::new(frame), &inputs, &config)
            .map_err(|error| error.to_string())?;
        if !inputs.is_empty() {
            input_history.push_back(super::replay::LockstepSimFrameInputs {
                frame,
                sim_inputs: inputs,
                raw_input_count: 1,
                sim_action_count: 1,
                sim_command_count: 1,
            });
        }
        event_history.push_back(LockstepSimFrameEvents {
            frame,
            events: result.events,
        });
    }
    let final_hash = hash_world(&world);
    let snapshot = ParsedInitialSnapshot {
        room_id: "offline-visual-fixture".to_string(),
        start_frame: world.frame.raw(),
        tick_rate: config.movement.tick_rate,
        config_version: 1,
        config_hash: "offline_visual_fixture.v1".to_string(),
        sim_schema_version: sim_core::SIM_CORE_SCHEMA_VERSION,
        rng_seed: world.rng.seed,
        state_hash: SimHashEnvelope {
            frame: final_hash.frame.raw(),
            value: final_hash.value,
            hex: format!("{:016x}", final_hash.value),
        },
        world: world.clone(),
        config: config.clone(),
        control_bindings: HashMap::from([(player_id.to_string(), EntityId::new(1000))]),
        entities: world.entities_sorted_by_id().to_vec(),
    };
    scene_state.replace_initial_snapshot(snapshot);
    replay.reset();
    replay.world = Some(world.clone());
    replay.config = Some(config);
    replay.snapshot_generation = scene_state.snapshot_generation;
    replay.snapshot_start_frame = Some(world.frame.raw());
    replay.last_applied_frame = Some(world.frame.raw());
    replay.input_history = input_history;
    replay.event_history = event_history;
    replay.hash_history = VecDeque::from([LockstepSimFrameHash {
        frame: world.frame.raw(),
        local_hash: final_hash,
        server_hash: None,
        event_count: replay
            .event_history
            .iter()
            .map(|events| events.events.len())
            .sum(),
    }]);
    replay.world_snapshots = VecDeque::from([LockstepSimWorldSnapshot {
        frame: world.frame.raw(),
        world,
        hash: final_hash,
    }]);
    combat_state.clear();
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_offline_visual_fixture_report(
    config: &LockstepSimVisualSmokeConfig,
    lockstep_config: &LockstepSimConfig,
    scene_state: &LockstepSimSceneState,
    authority: &AuthoritySession,
    replay: &LockstepSimReplayState,
    visual: &LockstepSimVisualState,
    combat: &LockstepSimCombatEventState,
    ui_mode: AppUiMode,
    screenshot: Option<&UiScreenshotSaved>,
) -> Result<(), String> {
    let report_path = config
        .offline_report_path
        .as_deref()
        .ok_or_else(|| "LOCKSTEP_SIM_VISUAL_SMOKE_OFFLINE_REPORT is missing".to_string())?;
    let event_kinds = replay
        .event_history
        .iter()
        .flat_map(|frame| frame.events.iter())
        .map(sim_event_kind)
        .collect::<Vec<_>>();
    let combat_kinds = combat
        .entries
        .iter()
        .map(|entry| combat_event_kind(entry.kind))
        .collect::<Vec<_>>();
    let target = replay
        .world
        .as_ref()
        .and_then(|world| world.entity(VISUAL_SMOKE_TARGET_ID));
    let buff_dot = event_kinds.contains(&"buff_applied")
        && event_kinds.contains(&"buff_tick")
        && event_kinds.contains(&"damage_applied")
        && combat_kinds.contains(&"buff_applied")
        && combat_kinds.contains(&"buff_tick")
        && combat_kinds.contains(&"damage_number");
    let death = event_kinds.contains(&"entity_died")
        && combat_kinds.contains(&"death_state")
        && target.is_some_and(|entity| !entity.alive && entity.combat.hp == 0);
    let hud = format_lockstep_sim_hud_status(&lockstep_sim_hud_snapshot(
        lockstep_config,
        scene_state,
        authority,
        replay,
    ));
    let passed = screenshot.is_some()
        && buff_dot
        && death
        && visual.tracked_entity_count >= 2
        && ui_mode == AppUiMode::RobotSyncScene;
    let value = json!({
        "schema": "mybevy.lockstep.visual-smoke",
        "schemaVersion": 1,
        "runId": config.run_id,
        "source": "offline_visual_fixture",
        "uiMode": ui_mode.canonical_screen(),
        "status": if passed { "passed" } else { "incomplete" },
        "passed": passed,
        "room": "offline-visual-fixture",
        "frame": replay.last_applied_frame,
        "eventKinds": event_kinds,
        "combatVisualKinds": combat_kinds,
        "hud": hud,
        "target": target.map(|entity| json!({
            "entityId": entity.id.raw(),
            "hp": entity.combat.hp,
            "maxHp": entity.combat.max_hp,
            "alive": entity.alive,
            "buffs": entity.combat.buffs.iter().map(|buff| buff.buff_id.raw()).collect::<Vec<_>>(),
        })),
        "visual": {
            "trackedEntityCount": visual.tracked_entity_count,
            "lastSyncedFrame": visual.last_synced_frame,
            "authoritativePositionSource": "LockstepSimReplayState.world/SimWorld",
            "renderDeltaWritesAuthority": false,
        },
        "coverage": {
            "buffApplied": event_kinds.contains(&"buff_applied"),
            "buffTick": event_kinds.contains(&"buff_tick"),
            "dotDamageNumber": buff_dot,
            "deathState": death,
        },
        "screenshot": screenshot.map(|saved| json!({
            "path": saved.request.path,
            "displayPath": saved.request.display_path,
            "width": saved.captured_size.0,
            "height": saved.captured_size.1,
            "completionFrame": saved.completion_frame,
        })),
    });
    write_json_file(report_path, &value)
}

#[allow(clippy::too_many_arguments)]
fn write_visual_smoke_report(
    config: &LockstepSimVisualSmokeConfig,
    lockstep_config: &LockstepSimConfig,
    scene_state: &LockstepSimSceneState,
    authority: &AuthoritySession,
    replay: &LockstepSimReplayState,
    visual: &LockstepSimVisualState,
    combat: &LockstepSimCombatEventState,
    ui_mode: AppUiMode,
    screenshot: Option<&UiScreenshotSaved>,
    failure: Option<&str>,
) -> Result<(), String> {
    let report_path = config
        .report_path
        .as_deref()
        .ok_or_else(|| "LOCKSTEP_SIM_VISUAL_SMOKE_REPORT is missing".to_string())?;
    let player_id = authority.local_player_id.as_deref().unwrap_or_default();
    let player_entity_id = scene_state
        .initial_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.control_bindings.get(player_id))
        .copied();
    let initial_player = player_entity_id.and_then(|entity_id| {
        scene_state
            .initial_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.world.entity(entity_id))
    });
    let world = replay.world.as_ref();
    let player =
        player_entity_id.and_then(|entity_id| world.and_then(|world| world.entity(entity_id)));
    let target = world.and_then(|world| world.entity(VISUAL_SMOKE_TARGET_ID));
    let visual_player = player_entity_id.and_then(|entity_id| {
        visual
            .debug_entries
            .iter()
            .find(|entry| entry.entity_id == entity_id)
    });
    let event_kinds = replay
        .event_history
        .iter()
        .flat_map(|frame| frame.events.iter())
        .map(sim_event_kind)
        .collect::<Vec<_>>();
    let combat_kinds = combat
        .entries
        .iter()
        .map(|entry| combat_event_kind(entry.kind))
        .collect::<Vec<_>>();
    let latest_hash = replay.hash_history.back();
    let hash_matched = latest_hash.is_some_and(|hash| {
        hash.server_hash
            .as_ref()
            .is_some_and(|server| server.value == hash.local_hash.value)
    });
    let hud = format_lockstep_sim_hud_status(&lockstep_sim_hud_snapshot(
        lockstep_config,
        scene_state,
        authority,
        replay,
    ));
    let moved = initial_player
        .zip(player)
        .is_some_and(|(initial, current)| initial.transform.pos != current.transform.pos);
    let skill = event_kinds.contains(&"skill_cast");
    let damage = event_kinds.contains(&"damage_applied")
        && combat_kinds.contains(&"damage_number")
        && combat_kinds.contains(&"hit");
    let buff_dot = event_kinds
        .iter()
        .any(|kind| matches!(*kind, "buff_applied" | "buff_tick" | "buff_expired"));
    let death = event_kinds.contains(&"entity_died");
    let hud_readable = [
        "room=",
        "policy=",
        "frame=",
        "local_hash=",
        "server_hash=",
        "mismatch=",
        "events=",
    ]
    .iter()
    .all(|field| hud.contains(field));
    let visual_matches_sim_world = visual_player.zip(player).is_some_and(|(visual, player)| {
        visual.raw_x == player.transform.pos.x.raw() && visual.raw_y == player.transform.pos.y.raw()
    });
    let core_smoke_passed = failure.is_none()
        && screenshot.is_some()
        && moved
        && skill
        && damage
        && hud_readable
        && hash_matched
        && visual_matches_sim_world
        && ui_mode == AppUiMode::RobotSyncScene;
    let mut fixture_gaps = Vec::new();
    if !buff_dot {
        fixture_gaps.push("buff_dot_not_configured_by_lockstep_sim_demo");
    }
    if !death {
        fixture_gaps.push("death_not_reached_by_single_demo_cast");
    }

    let value = json!({
        "schema": "mybevy.lockstep.visual-smoke",
        "schemaVersion": 1,
        "runId": config.run_id,
        "source": "myserver_authority",
        "uiMode": ui_mode.canonical_screen(),
        "status": if failure.is_some() { "failed" } else if core_smoke_passed { "captured_with_fixture_gaps" } else { "incomplete" },
        "coreSmokePassed": core_smoke_passed,
        "acceptanceComplete": core_smoke_passed && buff_dot && death,
        "room": lockstep_config.myserver_room_id,
        "policy": lockstep_config.myserver_policy_id,
        "player": player_id,
        "frame": replay.last_applied_frame,
        "localHash": latest_hash.map(|hash| format!("{:016x}", hash.local_hash.value)),
        "serverHash": latest_hash.and_then(|hash| hash.server_hash.as_ref()).map(|hash| hash.hex.clone()),
        "mismatch": latest_hash.map(|_| !hash_matched),
        "eventKinds": event_kinds,
        "combatVisualKinds": combat_kinds,
        "hud": hud,
        "entities": {
            "player": player.map(|entity| json!({
                "entityId": entity.id.raw(),
                "initialFixedPositionMilli": initial_player.map(|initial| json!({"x": initial.transform.pos.x.raw(), "y": initial.transform.pos.y.raw()})),
                "finalFixedPositionMilli": {"x": entity.transform.pos.x.raw(), "y": entity.transform.pos.y.raw()},
                "hp": entity.combat.hp,
                "alive": entity.alive,
            })),
            "target": target.map(|entity| json!({
                "entityId": entity.id.raw(),
                "fixedPositionMilli": {"x": entity.transform.pos.x.raw(), "y": entity.transform.pos.y.raw()},
                "hp": entity.combat.hp,
                "maxHp": entity.combat.max_hp,
                "alive": entity.alive,
            })),
        },
        "visual": {
            "trackedEntityCount": visual.tracked_entity_count,
            "simEntityCount": visual.sim_entity_count,
            "lastSyncedFrame": visual.last_synced_frame,
            "playerRawPositionMatchesSimWorld": visual_matches_sim_world,
            "authoritativePositionSource": "LockstepSimReplayState.world/SimWorld",
            "renderDeltaWritesAuthority": false,
        },
        "coverage": {
            "movement": moved,
            "skillCast": skill,
            "hitAndDamageNumber": damage,
            "buffDot": buff_dot,
            "deathState": death,
            "hudReadable": hud_readable,
            "hashMatched": hash_matched,
        },
        "fixtureGaps": fixture_gaps,
        "screenshot": screenshot.map(|saved| json!({
            "path": saved.request.path,
            "displayPath": saved.request.display_path,
            "width": saved.captured_size.0,
            "height": saved.captured_size.1,
            "completionFrame": saved.completion_frame,
        })),
        "failure": failure,
    });
    write_json_file(report_path, &value)
}

fn write_json_file(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    fs::write(path, bytes).map_err(|error| error.to_string())
}

fn sim_event_kind(event: &SimEvent) -> &'static str {
    match event {
        SimEvent::SkillCast { .. } => "skill_cast",
        SimEvent::DamageApplied { .. } => "damage_applied",
        SimEvent::HealApplied { .. } => "heal_applied",
        SimEvent::BuffApplied { .. } => "buff_applied",
        SimEvent::BuffExpired { .. } => "buff_expired",
        SimEvent::EntityDied { .. } => "entity_died",
        SimEvent::BuffTick { .. } => "buff_tick",
    }
}

fn combat_event_kind(kind: LockstepSimCombatEventKind) -> &'static str {
    match kind {
        LockstepSimCombatEventKind::SkillCast => "skill_cast",
        LockstepSimCombatEventKind::Hit => "hit",
        LockstepSimCombatEventKind::DamageNumber => "damage_number",
        LockstepSimCombatEventKind::HealNumber => "heal_number",
        LockstepSimCombatEventKind::BuffApplied => "buff_applied",
        LockstepSimCombatEventKind::BuffTick => "buff_tick",
        LockstepSimCombatEventKind::BuffExpired => "buff_expired",
        LockstepSimCombatEventKind::DeathState => "death_state",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_smoke_is_disabled_by_default_and_requires_explicit_paths() {
        let default = LockstepSimVisualSmokeConfig::from_env_reader(|_| None);
        assert!(!default.enabled);
        assert_eq!(default.screenshot_path, None);
        assert_eq!(default.report_path, None);

        let configured = LockstepSimVisualSmokeConfig::from_env_reader(|name| match name {
            "LOCKSTEP_SIM_VISUAL_SMOKE" => Some("true".to_string()),
            "LOCKSTEP_SIM_VISUAL_SMOKE_RUN_ID" => Some("visual-run".to_string()),
            "LOCKSTEP_SIM_VISUAL_SMOKE_SCREENSHOT" => Some("logs/smoke.png".to_string()),
            "LOCKSTEP_SIM_VISUAL_SMOKE_REPORT" => Some("logs/smoke.json".to_string()),
            "LOCKSTEP_SIM_VISUAL_SMOKE_OFFLINE_SCREENSHOT" => {
                Some("logs/offline-smoke.png".to_string())
            }
            "LOCKSTEP_SIM_VISUAL_SMOKE_OFFLINE_REPORT" => {
                Some("logs/offline-smoke.json".to_string())
            }
            "LOCKSTEP_SIM_VISUAL_SMOKE_TIMEOUT_MS" => Some("12000".to_string()),
            _ => None,
        });
        assert!(configured.enabled);
        assert_eq!(configured.run_id, "visual-run");
        assert_eq!(configured.timeout, Duration::from_secs(12));
        assert_eq!(
            configured.screenshot_path,
            Some(PathBuf::from("logs/smoke.png"))
        );
        assert_eq!(
            configured.offline_screenshot_path,
            Some(PathBuf::from("logs/offline-smoke.png"))
        );
    }

    #[test]
    fn visual_smoke_waits_for_ack_and_retries_only_expired_inputs() {
        let mut state = LockstepSimVisualSmokeState {
            input_frame: Some(1),
            input_attempts: 1,
            ..Default::default()
        };

        apply_visual_smoke_input_response(&mut state, false, "INPUT_FRAME_EXPIRED");
        assert_eq!(state.input_frame, None);
        assert_eq!(state.input_retry_after_frame, Some(1));
        assert!(!state.input_accepted);
        assert_eq!(state.failure, None);

        state.input_frame = Some(2);
        state.input_attempts = 2;
        apply_visual_smoke_input_response(&mut state, true, "");
        assert!(state.input_accepted);
        assert_eq!(state.failure, None);

        state.stop_frame = Some(3);
        state.stop_attempts = 1;
        apply_visual_smoke_input_response(&mut state, false, "INPUT_FRAME_EXPIRED");
        assert_eq!(state.stop_frame, None);
        assert_eq!(state.stop_retry_after_frame, Some(3));
        assert!(!state.stop_accepted);
        assert_eq!(state.failure, None);

        state.stop_frame = Some(4);
        state.stop_attempts = 2;
        apply_visual_smoke_input_response(&mut state, true, "");
        assert!(state.stop_accepted);
        assert_eq!(state.failure, None);
    }

    #[test]
    fn visual_smoke_rejects_non_expired_or_exhausted_input_failures() {
        let mut rejected = LockstepSimVisualSmokeState {
            input_frame: Some(1),
            input_attempts: 1,
            ..Default::default()
        };
        apply_visual_smoke_input_response(&mut rejected, false, "INPUT_INVALID");
        assert_eq!(
            rejected.failure.as_deref(),
            Some("player input rejected: INPUT_INVALID")
        );

        let mut exhausted = LockstepSimVisualSmokeState {
            input_frame: Some(8),
            input_attempts: VISUAL_SMOKE_MAX_INPUT_ATTEMPTS,
            ..Default::default()
        };
        apply_visual_smoke_input_response(&mut exhausted, false, "INPUT_FRAME_EXPIRED");
        assert!(
            exhausted
                .failure
                .as_deref()
                .is_some_and(|failure| failure.contains("after 8 attempts"))
        );
    }

    #[test]
    fn visual_smoke_targets_policy_input_window_after_latest_replay() {
        assert_eq!(next_visual_input_frame(10, Some(12), 9), 14);
        assert_eq!(next_visual_input_frame(14, Some(12), 9), 16);
        assert_eq!(next_visual_input_frame(0, None, 7), 9);
        assert_eq!(next_visual_input_frame(u32::MAX, Some(3), 2), u32::MAX);
    }

    #[test]
    fn offline_visual_fixture_covers_buff_dot_damage_and_death() {
        let mut scene = LockstepSimSceneState::default();
        scene.activate(
            crate::framework::scene::prelude::SceneId::from("arena.lockstep_sim"),
            crate::framework::scene::prelude::SceneSessionId::from("offline-visual-test"),
        );
        let mut replay = LockstepSimReplayState::default();
        let mut combat = LockstepSimCombatEventState::default();

        inject_offline_visual_fixture(&mut scene, &mut replay, &mut combat, "player-a").unwrap();

        let events = replay
            .event_history
            .iter()
            .flat_map(|frame| frame.events.iter())
            .map(sim_event_kind)
            .collect::<Vec<_>>();
        assert!(events.contains(&"buff_applied"));
        assert!(events.contains(&"buff_tick"));
        assert!(events.contains(&"damage_applied"));
        assert!(events.contains(&"entity_died"));
        let target = replay
            .world
            .as_ref()
            .unwrap()
            .entity(VISUAL_SMOKE_TARGET_ID)
            .unwrap();
        assert_eq!(target.combat.hp, 0);
        assert!(!target.alive);
    }
}
