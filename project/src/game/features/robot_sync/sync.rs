use std::collections::{BTreeMap, btree_map::Entry};

use bevy::prelude::*;

use crate::game::{
    authority::{
        AuthorityCommand, AuthorityEndpoint, AuthorityEvent, AuthorityFrame, AuthorityRole,
        AuthoritySession, AuthoritySnapshot, PlayerInput,
    },
    myserver::{MyServerCommand, MyServerEvent},
};

use super::{
    bot::{ROBOT_MOVE_ACTION, ROBOT_MOVE_PAYLOAD_VERSION, RobotMovePayload},
    config::{RobotSyncAuthorityMode, RobotSyncConfig},
    state::RobotSyncSceneState,
};

pub(in crate::game::features::robot_sync) const FIXED_UNIT: i32 = 1000;
pub(in crate::game::features::robot_sync) const ARENA_MIN_FIXED: i32 = -250_000;
pub(in crate::game::features::robot_sync) const ARENA_MAX_FIXED: i32 = 250_000;
const MAX_ROBOT_SPEED: u32 = 60_000;
const SPAWN_POINTS: [FixedPosition; 4] = [
    FixedPosition { x: -120_000, y: 0 },
    FixedPosition { x: 120_000, y: 0 },
    FixedPosition {
        x: -200_000,
        y: 200_000,
    },
    FixedPosition {
        x: 200_000,
        y: -200_000,
    },
];
#[derive(Clone, Debug, Default, Resource, PartialEq, Eq)]
pub(in crate::game) struct RobotSyncReplayState {
    pub(in crate::game::features::robot_sync) buffered_frame_count: usize,
    pub(in crate::game::features::robot_sync) last_frame_id: Option<u32>,
    pub(in crate::game::features::robot_sync) last_applied_frame: Option<u32>,
    pub(in crate::game::features::robot_sync) robots: BTreeMap<String, RobotState>,
    telemetry: RobotSyncTelemetryState,
}

#[derive(Clone, Debug, Default, Resource, PartialEq, Eq)]
pub(in crate::game::features::robot_sync) struct RobotSyncMyServerJoinState {
    pub(in crate::game::features::robot_sync) authority_started: bool,
    pub(in crate::game::features::robot_sync) login_sent: bool,
    pub(in crate::game::features::robot_sync) join_sent: bool,
    pub(in crate::game::features::robot_sync) ready_sent: bool,
    pub(in crate::game::features::robot_sync) start_sent: bool,
    pub(in crate::game::features::robot_sync) started: bool,
    authenticated_player_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::game::features::robot_sync) struct FixedPosition {
    pub(in crate::game::features::robot_sync) x: i32,
    pub(in crate::game::features::robot_sync) y: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game::features::robot_sync) struct RobotState {
    pub(in crate::game::features::robot_sync) player_id: String,
    pub(in crate::game::features::robot_sync) position: FixedPosition,
    pub(in crate::game::features::robot_sync) dir_x: i32,
    pub(in crate::game::features::robot_sync) dir_y: i32,
    pub(in crate::game::features::robot_sync) speed: u32,
    pub(in crate::game::features::robot_sync) last_input_seq: Option<u32>,
    pub(in crate::game::features::robot_sync) last_frame: Option<u32>,
    pub(in crate::game::features::robot_sync) spawn_index: usize,
    pub(in crate::game::features::robot_sync) color_index: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct RobotSyncTelemetryState {
    last_logged_frame: Option<u32>,
    last_logged_robot_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RobotSyncFrameTelemetry {
    frame_id: u32,
    robot_count: usize,
    checksum: u32,
    robots: String,
}

impl RobotSyncReplayState {
    pub(in crate::game::features::robot_sync) fn reset(&mut self) {
        *self = Self::default();
    }
}

impl RobotState {
    fn new(player_id: String, spawn_index: usize) -> Self {
        let position = spawn_point_for_index(spawn_index);

        Self {
            player_id,
            position,
            dir_x: 0,
            dir_y: 0,
            speed: 0,
            last_input_seq: None,
            last_frame: None,
            spawn_index,
            color_index: spawn_index,
        }
    }

    fn apply_input(&mut self, input: RobotMoveInput) {
        self.dir_x = input.dir_x;
        self.dir_y = input.dir_y;
        self.speed = input.speed;
        self.last_input_seq = Some(input.seq);
    }

    fn advance(&mut self, frame_id: u32, fps: u16) {
        let fps = i64::from(fps);
        let denom = i64::from(FIXED_UNIT) * fps;
        let speed = i64::from(self.speed);
        let delta_x = i64::from(self.dir_x) * speed / denom;
        let delta_y = i64::from(self.dir_y) * speed / denom;

        self.position.x = clamp_fixed_i64(i64::from(self.position.x) + delta_x);
        self.position.y = clamp_fixed_i64(i64::from(self.position.y) + delta_y);
        self.last_frame = Some(frame_id);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RobotMoveInput {
    seq: u32,
    dir_x: i32,
    dir_y: i32,
    speed: u32,
    original_index: usize,
}

impl RobotSyncMyServerJoinState {
    pub(in crate::game::features::robot_sync) fn reset(&mut self) {
        *self = Self::default();
    }
}

pub(in crate::game::features::robot_sync) fn reset_robot_sync_replay(
    state: &mut RobotSyncReplayState,
) {
    state.reset();
}

pub(in crate::game::features::robot_sync) fn apply_robot_sync_authority_events(
    scene_state: Res<RobotSyncSceneState>,
    mut events: MessageReader<AuthorityEvent>,
    mut replay_state: ResMut<RobotSyncReplayState>,
) {
    let is_active = scene_state.active;
    for event in events.read() {
        if !is_active {
            continue;
        }

        match event {
            AuthorityEvent::Snapshot { snapshot } => {
                apply_robot_sync_snapshot(&mut replay_state, snapshot);
            }
            AuthorityEvent::FrameApplied { frame } => {
                apply_robot_sync_frame(&mut replay_state, frame);
            }
            _ => {}
        }
    }
}

fn apply_robot_sync_snapshot(
    replay_state: &mut RobotSyncReplayState,
    snapshot: &AuthoritySnapshot,
) {
    let mut players = snapshot.players.clone();
    players.sort();
    players.dedup();

    let existing_players = replay_state.robots.keys().cloned().collect::<Vec<_>>();
    let player_set_changed = existing_players != players;

    if player_set_changed {
        replay_state.robots.clear();
        for (index, player_id) in players.into_iter().enumerate() {
            replay_state
                .robots
                .insert(player_id.clone(), RobotState::new(player_id, index));
        }
    } else {
        for (index, player_id) in players.into_iter().enumerate() {
            if let Some(robot) = replay_state.robots.get_mut(&player_id) {
                robot.spawn_index = index;
                robot.color_index = index;
            }
        }
    }

    if replay_state.last_applied_frame.is_none() {
        replay_state.last_applied_frame = Some(snapshot.frame_id);
    }
    replay_state.last_frame_id = Some(snapshot.frame_id);
    replay_state.buffered_frame_count = replay_state.robots.len();

    debug!(
        frame_id = snapshot.frame_id,
        authority_epoch = snapshot.authority_epoch,
        player_count = replay_state.robots.len(),
        player_set_changed,
        "applied robot sync snapshot"
    );
}

fn apply_robot_sync_frame(replay_state: &mut RobotSyncReplayState, frame: &AuthorityFrame) {
    if let Some(last_applied_frame) = replay_state.last_applied_frame {
        if frame.frame_id <= last_applied_frame {
            debug!(
                frame_id = frame.frame_id,
                last_applied_frame, "ignored duplicate or out-of-order robot sync frame"
            );
            return;
        }
    }

    if frame.fps == 0 {
        warn!(
            frame_id = frame.frame_id,
            "ignored robot sync frame with zero fps"
        );
        return;
    }

    let selected_inputs = select_robot_move_inputs(&frame.inputs, frame.frame_id);
    ensure_players_for_frame(replay_state, frame, &selected_inputs);

    for (player_id, input) in selected_inputs {
        if let Some(robot) = replay_state.robots.get_mut(&player_id) {
            robot.apply_input(input);
        }
    }

    for robot in replay_state.robots.values_mut() {
        robot.advance(frame.frame_id, frame.fps);
    }

    replay_state.last_applied_frame = Some(frame.frame_id);
    replay_state.last_frame_id = Some(frame.frame_id);
    replay_state.buffered_frame_count = replay_state.robots.len();
    maybe_log_robot_sync_frame_telemetry(replay_state, frame.frame_id);
}

fn ensure_players_for_frame(
    replay_state: &mut RobotSyncReplayState,
    frame: &AuthorityFrame,
    selected_inputs: &BTreeMap<String, RobotMoveInput>,
) {
    let mut players = frame.snapshot.players.clone();
    players.extend(selected_inputs.keys().cloned());
    players.sort();
    players.dedup();

    for (index, player_id) in players.into_iter().enumerate() {
        match replay_state.robots.entry(player_id.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(RobotState::new(player_id, index));
            }
            Entry::Occupied(mut entry) => {
                let robot = entry.get_mut();
                robot.spawn_index = index;
                robot.color_index = index;
            }
        }
    }
}

fn select_robot_move_inputs(
    inputs: &[PlayerInput],
    frame_id: u32,
) -> BTreeMap<String, RobotMoveInput> {
    let mut selected = BTreeMap::new();

    for (original_index, input) in inputs.iter().enumerate() {
        if input.action != ROBOT_MOVE_ACTION {
            debug!(
                frame_id,
                player_id = %input.player_id,
                action = %input.action,
                "ignored non robot_move authority input"
            );
            continue;
        }

        let Some(move_input) = parse_robot_move_input(input, original_index, frame_id) else {
            continue;
        };

        match selected.entry(input.player_id.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(move_input);
            }
            Entry::Occupied(mut entry) if should_replace_robot_move(*entry.get(), move_input) => {
                entry.insert(move_input);
            }
            Entry::Occupied(_) => {}
        }
    }

    selected
}

fn parse_robot_move_input(
    input: &PlayerInput,
    original_index: usize,
    frame_id: u32,
) -> Option<RobotMoveInput> {
    let payload = match serde_json::from_str::<RobotMovePayload>(&input.payload_json) {
        Ok(payload) => payload,
        Err(error) => {
            warn!(
                frame_id,
                player_id = %input.player_id,
                reason = %error,
                "ignored invalid robot_move payload"
            );
            return None;
        }
    };

    if payload.version != ROBOT_MOVE_PAYLOAD_VERSION {
        warn!(
            frame_id,
            player_id = %input.player_id,
            version = payload.version,
            "ignored unsupported robot_move payload version"
        );
        return None;
    }

    let length_squared = i64::from(payload.dir_x) * i64::from(payload.dir_x)
        + i64::from(payload.dir_y) * i64::from(payload.dir_y);
    if !(-FIXED_UNIT..=FIXED_UNIT).contains(&payload.dir_x)
        || !(-FIXED_UNIT..=FIXED_UNIT).contains(&payload.dir_y)
        || length_squared > i64::from(FIXED_UNIT) * i64::from(FIXED_UNIT)
    {
        warn!(
            frame_id,
            player_id = %input.player_id,
            dirX = payload.dir_x,
            dirY = payload.dir_y,
            "ignored out-of-range robot_move direction"
        );
        return None;
    }

    if payload.speed > MAX_ROBOT_SPEED {
        warn!(
            frame_id,
            player_id = %input.player_id,
            speed = payload.speed,
            max_speed = MAX_ROBOT_SPEED,
            "ignored out-of-range robot_move speed"
        );
        return None;
    }

    if payload.speed > 0 && payload.dir_x == 0 && payload.dir_y == 0 {
        warn!(
            frame_id,
            player_id = %input.player_id,
            speed = payload.speed,
            "ignored moving robot_move payload with zero direction"
        );
        return None;
    }

    Some(RobotMoveInput {
        seq: payload.seq,
        dir_x: payload.dir_x,
        dir_y: payload.dir_y,
        speed: payload.speed,
        original_index,
    })
}

fn should_replace_robot_move(existing: RobotMoveInput, candidate: RobotMoveInput) -> bool {
    candidate.seq > existing.seq
        || (candidate.seq == existing.seq && candidate.original_index > existing.original_index)
}

fn spawn_point_for_index(index: usize) -> FixedPosition {
    SPAWN_POINTS[index % SPAWN_POINTS.len()]
}

fn clamp_fixed_i64(value: i64) -> i32 {
    value.clamp(i64::from(ARENA_MIN_FIXED), i64::from(ARENA_MAX_FIXED)) as i32
}

fn maybe_log_robot_sync_frame_telemetry(replay_state: &mut RobotSyncReplayState, frame_id: u32) {
    let robot_count = replay_state.robots.len();
    let should_log = replay_state
        .telemetry
        .last_logged_frame
        .is_none_or(|last_frame| frame_id.saturating_sub(last_frame) >= 20)
        || replay_state.telemetry.last_logged_robot_count != robot_count;

    if !should_log {
        return;
    }

    replay_state.telemetry.last_logged_frame = Some(frame_id);
    replay_state.telemetry.last_logged_robot_count = robot_count;
    let telemetry = robot_sync_frame_telemetry(frame_id, &replay_state.robots);
    info!(
        frame_id = telemetry.frame_id,
        robot_count = telemetry.robot_count,
        checksum = format_args!("{:08x}", telemetry.checksum),
        robots = %telemetry.robots,
        "robot sync frame applied"
    );
}

fn robot_sync_frame_telemetry(
    frame_id: u32,
    robots: &BTreeMap<String, RobotState>,
) -> RobotSyncFrameTelemetry {
    RobotSyncFrameTelemetry {
        frame_id,
        robot_count: robots.len(),
        checksum: robot_sync_checksum(frame_id, robots),
        robots: robot_sync_robot_summary(robots),
    }
}

fn robot_sync_robot_summary(robots: &BTreeMap<String, RobotState>) -> String {
    robots
        .values()
        .map(|robot| {
            format!(
                "{}:x={},y={},last_frame={}",
                robot.player_id,
                robot.position.x,
                robot.position.y,
                robot
                    .last_frame
                    .map(|frame| frame.to_string())
                    .unwrap_or_else(|| "none".to_string())
            )
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn robot_sync_checksum(frame_id: u32, robots: &BTreeMap<String, RobotState>) -> u32 {
    let robot_sum = robots.values().fold(0_u32, |sum, robot| {
        let heading_milli_degrees = 0_u32;
        let last_frame = robot.last_frame.unwrap_or_default();
        let robot_value = fnv1a32(robot.player_id.as_bytes())
            ^ low32_i32(robot.position.x)
            ^ low32_i32(robot.position.y).rotate_left(1)
            ^ heading_milli_degrees.rotate_left(7)
            ^ last_frame;
        sum.wrapping_add(robot_value)
    });

    robot_sum ^ frame_id
}

fn fnv1a32(bytes: &[u8]) -> u32 {
    const FNV_OFFSET_BASIS: u32 = 2_166_136_261;
    const FNV_PRIME: u32 = 16_777_619;

    bytes.iter().fold(FNV_OFFSET_BASIS, |hash, byte| {
        (hash ^ u32::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

fn low32_i32(value: i32) -> u32 {
    value as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(frame_id: u32, players: &[&str]) -> AuthoritySnapshot {
        AuthoritySnapshot {
            authority_epoch: 1,
            frame_id,
            authority_player_id: players.first().copied().unwrap_or_default().to_string(),
            players: players.iter().map(|player| (*player).to_string()).collect(),
            game_state_json: "{}".to_string(),
        }
    }

    fn frame(
        frame_id: u32,
        fps: u16,
        players: &[&str],
        inputs: Vec<PlayerInput>,
    ) -> AuthorityFrame {
        AuthorityFrame {
            authority_epoch: 1,
            frame_id,
            fps,
            inputs,
            snapshot: snapshot(frame_id, players),
        }
    }

    fn robot_move(
        player_id: &str,
        frame_id: u32,
        seq: u32,
        dir_x: i32,
        dir_y: i32,
        speed: u32,
    ) -> PlayerInput {
        PlayerInput {
            player_id: player_id.to_string(),
            frame_id,
            action: ROBOT_MOVE_ACTION.to_string(),
            payload_json: serde_json::json!({
                "version": 1,
                "seq": seq,
                "botTick": seq,
                "dirX": dir_x,
                "dirY": dir_y,
                "speed": speed,
            })
            .to_string(),
        }
    }

    fn bad_payload(player_id: &str, frame_id: u32, payload_json: &str) -> PlayerInput {
        PlayerInput {
            player_id: player_id.to_string(),
            frame_id,
            action: ROBOT_MOVE_ACTION.to_string(),
            payload_json: payload_json.to_string(),
        }
    }

    fn robot_state(player_id: &str, x: i32, y: i32, last_frame: Option<u32>) -> RobotState {
        RobotState {
            player_id: player_id.to_string(),
            position: FixedPosition { x, y },
            dir_x: 1000,
            dir_y: 0,
            speed: 60_000,
            last_input_seq: Some(1),
            last_frame,
            spawn_index: 0,
            color_index: 0,
        }
    }

    #[test]
    fn robot_sync_snapshot_builds_sorted_initial_robot_state() {
        let mut state = RobotSyncReplayState::default();

        apply_robot_sync_snapshot(
            &mut state,
            &snapshot(10, &["player-b", "player-a", "player-a"]),
        );

        assert_eq!(state.last_applied_frame, Some(10));
        assert_eq!(state.last_frame_id, Some(10));
        assert_eq!(state.robots.len(), 2);
        let player_ids = state.robots.keys().cloned().collect::<Vec<_>>();
        assert_eq!(player_ids, vec!["player-a", "player-b"]);
        assert_eq!(
            state.robots.get("player-a").unwrap().position,
            FixedPosition { x: -120_000, y: 0 }
        );
        assert_eq!(
            state.robots.get("player-b").unwrap().position,
            FixedPosition { x: 120_000, y: 0 }
        );
    }

    #[test]
    fn robot_sync_snapshot_preserves_existing_robot_motion_state_when_player_set_is_unchanged() {
        let mut state = RobotSyncReplayState::default();
        apply_robot_sync_snapshot(&mut state, &snapshot(0, &["player-a"]));
        apply_robot_sync_frame(
            &mut state,
            &frame(
                1,
                20,
                &["player-a"],
                vec![robot_move("player-a", 1, 5, 0, 1000, 60_000)],
            ),
        );

        apply_robot_sync_snapshot(&mut state, &snapshot(1, &["player-a"]));

        let player_a = state.robots.get("player-a").unwrap();
        assert_eq!(
            player_a.position,
            FixedPosition {
                x: -120_000,
                y: 3_000
            }
        );
        assert_eq!(player_a.dir_x, 0);
        assert_eq!(player_a.dir_y, 1000);
        assert_eq!(player_a.last_input_seq, Some(5));
        assert_eq!(state.last_applied_frame, Some(1));
    }

    #[test]
    fn robot_sync_snapshot_rebuilds_state_when_player_set_changes() {
        let mut state = RobotSyncReplayState::default();
        apply_robot_sync_snapshot(&mut state, &snapshot(0, &["player-a"]));
        apply_robot_sync_frame(
            &mut state,
            &frame(
                1,
                20,
                &["player-a"],
                vec![robot_move("player-a", 1, 5, 0, 1000, 60_000)],
            ),
        );

        apply_robot_sync_snapshot(&mut state, &snapshot(1, &["player-a", "player-b"]));

        assert_eq!(
            state.robots.get("player-a").unwrap().position,
            FixedPosition { x: -120_000, y: 0 }
        );
        assert_eq!(state.robots.get("player-a").unwrap().last_input_seq, None);
        assert_eq!(
            state.robots.get("player-b").unwrap().position,
            FixedPosition { x: 120_000, y: 0 }
        );

        apply_robot_sync_snapshot(&mut state, &snapshot(2, &["player-b"]));
        assert!(!state.robots.contains_key("player-a"));
        assert!(state.robots.contains_key("player-b"));
        assert_eq!(
            state.robots.get("player-b").unwrap().position,
            FixedPosition { x: -120_000, y: 0 }
        );
    }

    #[test]
    fn robot_sync_payload_parsing_accepts_valid_and_rejects_invalid_payloads() {
        let valid = robot_move("player-a", 11, 7, 1000, 0, 60_000);
        let parsed = parse_robot_move_input(&valid, 0, 11).unwrap();
        assert_eq!(parsed.seq, 7);
        assert_eq!(parsed.dir_x, 1000);
        assert_eq!(parsed.speed, 60_000);

        let invalid_json = bad_payload("player-a", 11, "{");
        assert!(parse_robot_move_input(&invalid_json, 0, 11).is_none());

        let unknown_field = bad_payload(
            "player-a",
            11,
            r#"{"version":1,"seq":1,"botTick":1,"dirX":1000,"dirY":0,"speed":1000,"extra":1}"#,
        );
        assert!(parse_robot_move_input(&unknown_field, 0, 11).is_none());

        let diagonal_too_long = robot_move("player-a", 11, 1, 1000, 1000, 1000);
        assert!(parse_robot_move_input(&diagonal_too_long, 0, 11).is_none());

        let speed_too_high = robot_move("player-a", 11, 1, 1000, 0, 60_001);
        assert!(parse_robot_move_input(&speed_too_high, 0, 11).is_none());

        let zero_direction_with_speed = robot_move("player-a", 11, 1, 0, 0, 1);
        assert!(parse_robot_move_input(&zero_direction_with_speed, 0, 11).is_none());

        let string_seq = bad_payload(
            "player-a",
            11,
            r#"{"version":1,"seq":"1","botTick":1,"dirX":1000,"dirY":0,"speed":1000}"#,
        );
        assert!(parse_robot_move_input(&string_seq, 0, 11).is_none());

        let float_direction = bad_payload(
            "player-a",
            11,
            r#"{"version":1,"seq":1,"botTick":1,"dirX":1.5,"dirY":0,"speed":1000}"#,
        );
        assert!(parse_robot_move_input(&float_direction, 0, 11).is_none());
    }

    #[test]
    fn robot_sync_frame_selects_highest_seq_then_last_input_per_player() {
        let mut state = RobotSyncReplayState::default();
        apply_robot_sync_snapshot(&mut state, &snapshot(0, &["player-a"]));

        apply_robot_sync_frame(
            &mut state,
            &frame(
                1,
                20,
                &["player-a"],
                vec![
                    robot_move("player-a", 1, 3, 0, 1000, 60_000),
                    robot_move("player-a", 1, 5, -1000, 0, 60_000),
                    robot_move("player-a", 1, 5, 1000, 0, 60_000),
                ],
            ),
        );

        let robot = state.robots.get("player-a").unwrap();
        assert_eq!(robot.last_input_seq, Some(5));
        assert_eq!(robot.dir_x, 1000);
        assert_eq!(robot.dir_y, 0);
        assert_eq!(robot.position, FixedPosition { x: -117_000, y: 0 });
    }

    #[test]
    fn robot_sync_frame_ignores_unknown_action_and_bad_payload_without_blocking_valid_input() {
        let mut state = RobotSyncReplayState::default();
        apply_robot_sync_snapshot(&mut state, &snapshot(0, &["player-a"]));

        apply_robot_sync_frame(
            &mut state,
            &frame(
                1,
                20,
                &["player-a"],
                vec![
                    PlayerInput {
                        player_id: "player-a".to_string(),
                        frame_id: 1,
                        action: "unknown_action".to_string(),
                        payload_json: "{}".to_string(),
                    },
                    bad_payload(
                        "player-a",
                        1,
                        r#"{"version":1,"seq":2,"botTick":2,"dirX":1000,"dirY":0,"speed":70000}"#,
                    ),
                    robot_move("player-a", 1, 3, 0, 1000, 60_000),
                ],
            ),
        );

        let robot = state.robots.get("player-a").unwrap();
        assert_eq!(robot.last_input_seq, Some(3));
        assert_eq!(robot.dir_x, 0);
        assert_eq!(robot.dir_y, 1000);
        assert_eq!(
            robot.position,
            FixedPosition {
                x: -120_000,
                y: 3_000
            }
        );
        assert_eq!(state.last_applied_frame, Some(1));
    }

    #[test]
    fn robot_sync_frame_advances_all_players_with_fixed_dt() {
        let mut state = RobotSyncReplayState::default();
        apply_robot_sync_snapshot(&mut state, &snapshot(0, &["player-b", "player-a"]));

        apply_robot_sync_frame(
            &mut state,
            &frame(
                1,
                20,
                &["player-a", "player-b"],
                vec![
                    robot_move("player-b", 1, 1, -1000, 0, 60_000),
                    robot_move("player-a", 1, 1, 0, 1000, 60_000),
                ],
            ),
        );

        assert_eq!(
            state.robots.get("player-a").unwrap().position,
            FixedPosition {
                x: -120_000,
                y: 3_000
            }
        );
        assert_eq!(
            state.robots.get("player-b").unwrap().position,
            FixedPosition { x: 117_000, y: 0 }
        );
        assert_eq!(state.last_applied_frame, Some(1));
    }

    #[test]
    fn robot_sync_frame_adds_new_players_with_sorted_spawn_index() {
        let mut state = RobotSyncReplayState::default();
        apply_robot_sync_snapshot(&mut state, &snapshot(0, &["player-z"]));

        apply_robot_sync_frame(
            &mut state,
            &frame(1, 20, &["player-a", "player-z"], Vec::new()),
        );

        assert_eq!(state.robots.get("player-a").unwrap().spawn_index, 0);
        assert_eq!(state.robots.get("player-z").unwrap().spawn_index, 1);
        assert_eq!(
            state.robots.get("player-a").unwrap().position,
            FixedPosition { x: -120_000, y: 0 }
        );
        assert_eq!(
            state.robots.get("player-z").unwrap().position,
            FixedPosition { x: -120_000, y: 0 }
        );
    }

    #[test]
    fn robot_sync_frame_clamps_positions_to_arena_bounds() {
        let mut state = RobotSyncReplayState::default();
        state.robots.insert(
            "player-a".to_string(),
            RobotState {
                player_id: "player-a".to_string(),
                position: FixedPosition {
                    x: 249_000,
                    y: 249_000,
                },
                dir_x: 1000,
                dir_y: 1000,
                speed: 60_000,
                last_input_seq: None,
                last_frame: None,
                spawn_index: 0,
                color_index: 0,
            },
        );

        apply_robot_sync_frame(
            &mut state,
            &frame(
                1,
                20,
                &["player-a"],
                vec![robot_move("player-a", 1, 1, 707, 707, 60_000)],
            ),
        );

        assert_eq!(
            state.robots.get("player-a").unwrap().position,
            FixedPosition {
                x: 250_000,
                y: 250_000
            }
        );
    }

    #[test]
    fn robot_sync_frame_with_zero_fps_does_not_advance_or_mark_applied() {
        let mut state = RobotSyncReplayState::default();
        apply_robot_sync_snapshot(&mut state, &snapshot(0, &["player-a"]));

        apply_robot_sync_frame(
            &mut state,
            &frame(
                1,
                0,
                &["player-a"],
                vec![robot_move("player-a", 1, 1, 0, 1000, 60_000)],
            ),
        );

        let robot = state.robots.get("player-a").unwrap();
        assert_eq!(robot.position, FixedPosition { x: -120_000, y: 0 });
        assert_eq!(robot.last_input_seq, None);
        assert_eq!(robot.last_frame, None);
        assert_eq!(state.last_applied_frame, Some(0));
        assert_eq!(state.last_frame_id, Some(0));
    }

    #[test]
    fn robot_sync_frame_ignores_duplicate_and_out_of_order_frames() {
        let mut state = RobotSyncReplayState::default();
        apply_robot_sync_snapshot(&mut state, &snapshot(0, &["player-a"]));
        apply_robot_sync_frame(
            &mut state,
            &frame(
                2,
                20,
                &["player-a"],
                vec![robot_move("player-a", 2, 1, 1000, 0, 60_000)],
            ),
        );
        let after_frame_two = state.robots.get("player-a").unwrap().position;

        apply_robot_sync_frame(
            &mut state,
            &frame(
                2,
                20,
                &["player-a"],
                vec![robot_move("player-a", 2, 2, 0, 1000, 60_000)],
            ),
        );
        apply_robot_sync_frame(
            &mut state,
            &frame(
                1,
                20,
                &["player-a"],
                vec![robot_move("player-a", 1, 3, -1000, 0, 60_000)],
            ),
        );

        assert_eq!(
            state.robots.get("player-a").unwrap().position,
            after_frame_two
        );
        assert_eq!(state.last_applied_frame, Some(2));
    }

    #[test]
    fn robot_sync_missing_input_keeps_previous_movement_but_initially_stops() {
        let mut state = RobotSyncReplayState::default();
        apply_robot_sync_snapshot(&mut state, &snapshot(0, &["player-a", "player-b"]));

        apply_robot_sync_frame(
            &mut state,
            &frame(
                1,
                20,
                &["player-a", "player-b"],
                vec![robot_move("player-a", 1, 1, 0, 1000, 60_000)],
            ),
        );
        apply_robot_sync_frame(
            &mut state,
            &frame(2, 20, &["player-a", "player-b"], Vec::new()),
        );

        assert_eq!(
            state.robots.get("player-a").unwrap().position,
            FixedPosition {
                x: -120_000,
                y: 6_000
            }
        );
        assert_eq!(
            state.robots.get("player-b").unwrap().position,
            FixedPosition { x: 120_000, y: 0 }
        );
    }

    #[test]
    fn robot_sync_telemetry_summary_uses_stable_player_order() {
        let mut robots = BTreeMap::new();
        robots.insert(
            "player-b".to_string(),
            robot_state("player-b", 20, -30, Some(7)),
        );
        robots.insert(
            "player-a".to_string(),
            robot_state("player-a", -10, 40, Some(7)),
        );

        assert_eq!(
            robot_sync_robot_summary(&robots),
            "player-a:x=-10,y=40,last_frame=7;player-b:x=20,y=-30,last_frame=7"
        );
    }

    #[test]
    fn robot_sync_checksum_is_stable_and_uses_fixed_coordinates() {
        let mut robots = BTreeMap::new();
        robots.insert(
            "player-b".to_string(),
            robot_state("player-b", 20, -30, Some(7)),
        );
        robots.insert(
            "player-a".to_string(),
            robot_state("player-a", -10, 40, Some(7)),
        );

        let checksum = robot_sync_checksum(120, &robots);
        assert_eq!(checksum, robot_sync_checksum(120, &robots));

        let mut changed = robots.clone();
        changed.get_mut("player-a").unwrap().position.x = -9;
        assert_ne!(checksum, robot_sync_checksum(120, &changed));
    }

    #[test]
    fn robot_sync_frame_telemetry_contains_checksum_and_robot_summary() {
        let mut robots = BTreeMap::new();
        robots.insert(
            "player-a".to_string(),
            robot_state("player-a", 10240, -5000, Some(120)),
        );

        let telemetry = robot_sync_frame_telemetry(120, &robots);

        assert_eq!(telemetry.frame_id, 120);
        assert_eq!(telemetry.robot_count, 1);
        assert_eq!(telemetry.robots, "player-a:x=10240,y=-5000,last_frame=120");
        assert_eq!(telemetry.checksum, robot_sync_checksum(120, &robots));
    }

    #[test]
    fn robot_sync_authority_event_system_drains_inactive_events() {
        let mut app = App::new();
        app.add_message::<AuthorityEvent>()
            .init_resource::<RobotSyncSceneState>()
            .init_resource::<RobotSyncReplayState>()
            .add_systems(Update, apply_robot_sync_authority_events);

        app.world_mut().write_message(AuthorityEvent::Snapshot {
            snapshot: snapshot(10, &["stale-player"]),
        });
        app.update();
        assert!(
            app.world()
                .resource::<RobotSyncReplayState>()
                .robots
                .is_empty()
        );

        app.world_mut().resource_mut::<RobotSyncSceneState>().active = true;
        app.world_mut().write_message(AuthorityEvent::Snapshot {
            snapshot: snapshot(11, &["active-player"]),
        });
        app.update();

        let replay_state = app.world().resource::<RobotSyncReplayState>();
        assert!(!replay_state.robots.contains_key("stale-player"));
        assert!(replay_state.robots.contains_key("active-player"));
        assert_eq!(replay_state.last_applied_frame, Some(11));
    }
}

pub(in crate::game::features::robot_sync) fn start_robot_sync_authority(
    config: &RobotSyncConfig,
    session: &AuthoritySession,
    state: &mut RobotSyncMyServerJoinState,
    authority_commands: &mut MessageWriter<AuthorityCommand>,
    myserver_commands: &mut MessageWriter<MyServerCommand>,
) {
    if state.authority_started {
        debug!("robot sync authority startup already handled");
        return;
    }

    state.authority_started = true;

    match config.authority_mode {
        RobotSyncAuthorityMode::Off => {
            info!(
                player_id = %config.local_player_id,
                "robot sync authority startup disabled"
            );
        }
        RobotSyncAuthorityMode::Local => {
            leave_existing_authority_if_needed(session, authority_commands);
            info!(
                player_id = %config.local_player_id,
                "robot sync starting local authority"
            );
            authority_commands.write(AuthorityCommand::HostLocal {
                player_id: config.local_player_id.clone(),
            });
        }
        RobotSyncAuthorityMode::LanHost => {
            leave_existing_authority_if_needed(session, authority_commands);
            info!(
                player_id = %config.local_player_id,
                bind_addr = %config.lan_bind_addr,
                transport = ?config.transport,
                "robot sync starting LAN authority"
            );
            authority_commands.write(AuthorityCommand::HostLan {
                player_id: config.local_player_id.clone(),
                bind_addr: config.lan_bind_addr.clone(),
                transport: config.transport,
            });
        }
        RobotSyncAuthorityMode::LanClient => {
            leave_existing_authority_if_needed(session, authority_commands);
            info!(
                player_id = %config.local_player_id,
                host = %config.remote_host,
                port = config.remote_port,
                transport = ?config.transport,
                "robot sync joining LAN authority"
            );
            authority_commands.write(AuthorityCommand::Join {
                player_id: config.local_player_id.clone(),
                endpoint: AuthorityEndpoint::Remote {
                    host: config.remote_host.clone(),
                    port: config.remote_port,
                    transport: config.transport,
                },
            });
        }
        RobotSyncAuthorityMode::MyServer => {
            leave_existing_authority_if_needed(session, authority_commands);
            info!(
                player_id = %config.local_player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                transport = ?config.transport,
                "robot sync starting MyServer authority"
            );
            authority_commands.write(AuthorityCommand::Join {
                player_id: config.local_player_id.clone(),
                endpoint: AuthorityEndpoint::MyServer {
                    host: None,
                    port: None,
                    transport: config.transport,
                },
            });
            myserver_commands.write(MyServerCommand::GuestLogin {
                guest_id: config.myserver_guest_id.clone(),
                connect_game: true,
            });
            state.login_sent = true;
        }
    }
}

pub(in crate::game::features::robot_sync) fn cleanup_robot_sync_authority(
    config: &RobotSyncConfig,
    state: &mut RobotSyncMyServerJoinState,
    authority_commands: &mut MessageWriter<AuthorityCommand>,
    myserver_commands: &mut MessageWriter<MyServerCommand>,
) {
    let should_disconnect_myserver =
        matches!(config.authority_mode, RobotSyncAuthorityMode::MyServer)
            || state.login_sent
            || state.join_sent
            || state.ready_sent
            || state.start_sent;

    state.reset();
    info!(
        player_id = %config.local_player_id,
        guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
        room_id = %config.myserver_room_id,
        policy_id = %config.myserver_policy_id,
        disconnect_myserver = should_disconnect_myserver,
        "robot sync authority cleanup"
    );
    authority_commands.write(AuthorityCommand::Leave);

    if should_disconnect_myserver {
        myserver_commands.write(MyServerCommand::Disconnect);
    }
}

pub(in crate::game::features::robot_sync) fn follow_robot_sync_myserver_events(
    config: Res<RobotSyncConfig>,
    scene_state: Res<RobotSyncSceneState>,
    mut state: ResMut<RobotSyncMyServerJoinState>,
    mut events: MessageReader<MyServerEvent>,
    mut commands: MessageWriter<MyServerCommand>,
) {
    if !scene_state.active || !matches!(config.authority_mode, RobotSyncAuthorityMode::MyServer) {
        return;
    }

    for event in events.read() {
        handle_robot_sync_myserver_event(&config, &mut state, event, &mut commands);
    }
}

fn handle_robot_sync_myserver_event(
    config: &RobotSyncConfig,
    state: &mut RobotSyncMyServerJoinState,
    event: &MyServerEvent,
    commands: &mut MessageWriter<MyServerCommand>,
) {
    match event {
        MyServerEvent::Authenticated { player_id } if !state.join_sent => {
            state.authenticated_player_id = Some(player_id.clone());
            state.join_sent = true;
            info!(
                player_id = %player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                "robot sync joining MyServer room"
            );
            commands.write(MyServerCommand::JoinRoom {
                room_id: config.myserver_room_id.clone(),
                policy_id: config.myserver_policy_id.clone(),
            });
        }
        MyServerEvent::RoomJoined(response)
            if response.ok && state.join_sent && !state.ready_sent =>
        {
            state.ready_sent = true;
            info!(
                room_id = %response.room_id,
                policy_id = %config.myserver_policy_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                "robot sync MyServer room joined"
            );
            commands.write(MyServerCommand::SetReady { ready: true });
        }
        MyServerEvent::RoomJoined(response) if !response.ok => {
            warn!(
                room_id = %response.room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                error_code = %response.error_code,
                "robot sync MyServer room join rejected"
            );
        }
        MyServerEvent::ReadyChanged(response) if response.ok && state.ready_sent => {
            info!(
                room_id = %response.room_id,
                policy_id = %config.myserver_policy_id,
                ready = response.ready,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                "robot sync MyServer ready changed"
            );
        }
        MyServerEvent::ReadyChanged(response) if !response.ok => {
            warn!(
                room_id = %response.room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                error_code = %response.error_code,
                "robot sync MyServer ready rejected"
            );
        }
        MyServerEvent::RoomStarted(response) if response.ok => {
            state.started = true;
            info!(
                room_id = %response.room_id,
                policy_id = %config.myserver_policy_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                "robot sync MyServer room started"
            );
        }
        MyServerEvent::RoomStarted(response) => {
            warn!(
                room_id = %response.room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                error_code = %response.error_code,
                "robot sync MyServer room start rejected"
            );
        }
        MyServerEvent::RoomStatePush(push)
            if state.ready_sent
                && !state.start_sent
                && should_start_robot_sync_room(state, push) =>
        {
            let Some(snapshot) = push.snapshot.as_ref() else {
                return;
            };
            state.start_sent = true;
            info!(
                room_id = %snapshot.room_id,
                policy_id = %config.myserver_policy_id,
                owner_player_id = %snapshot.owner_player_id,
                member_count = snapshot.members.len(),
                "robot sync MyServer starting room after all players ready"
            );
            commands.write(MyServerCommand::StartRoom);
        }
        MyServerEvent::ConnectionFailed {
            transport,
            remote_addr,
            error,
        } => {
            error!(
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                ?transport,
                remote_addr = %remote_addr,
                reason = %error,
                "robot sync MyServer connection failed"
            );
        }
        MyServerEvent::Disconnected { reason } => {
            warn!(
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                reason = reason.as_deref().unwrap_or_default(),
                "robot sync MyServer disconnected"
            );
        }
        MyServerEvent::AuthFailed { error_code } => {
            error!(
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                error_code = %error_code,
                "robot sync MyServer auth failed"
            );
        }
        MyServerEvent::Error {
            seq,
            error_code,
            message,
        } => {
            warn!(
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                seq = *seq,
                error_code = %error_code,
                reason = %message,
                "robot sync MyServer error"
            );
        }
        MyServerEvent::ProtocolError { error } => {
            error!(
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                reason = %error,
                "robot sync MyServer protocol error"
            );
        }
        MyServerEvent::RequestFailed {
            seq,
            message_type,
            error,
        } => {
            warn!(
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                ?seq,
                ?message_type,
                reason = %error,
                "robot sync MyServer request failed"
            );
        }
        _ => {}
    }
}

fn should_start_robot_sync_room(
    state: &RobotSyncMyServerJoinState,
    push: &crate::game::myserver::protocol::pb::RoomStatePush,
) -> bool {
    let Some(snapshot) = push.snapshot.as_ref() else {
        return false;
    };
    let Some(player_id) = state.authenticated_player_id.as_deref() else {
        return false;
    };
    if snapshot.owner_player_id != player_id || snapshot.state == "in_game" {
        return false;
    }

    if snapshot.members.len() < 2 {
        return false;
    }

    for member in &snapshot.members {
        if !member.ready {
            return false;
        }
    }

    true
}

fn leave_existing_authority_if_needed(
    session: &AuthoritySession,
    authority_commands: &mut MessageWriter<AuthorityCommand>,
) {
    if session.role.is_some_and(|role| role != AuthorityRole::None) {
        authority_commands.write(AuthorityCommand::Leave);
    }
}
