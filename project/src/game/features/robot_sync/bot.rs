use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub(in crate::game::features::robot_sync) const ROBOT_MOVE_ACTION: &str = "robot_move";
pub(in crate::game::features::robot_sync) const ROBOT_MOVE_PAYLOAD_VERSION: u32 = 1;

const BOT_DIRECTION_SEGMENT_TICKS: u32 = 20;
const BOT_DIRECTIONS: [RobotMoveDirection; 8] = [
    RobotMoveDirection {
        dir_x: 1000,
        dir_y: 0,
    },
    RobotMoveDirection {
        dir_x: 0,
        dir_y: 1000,
    },
    RobotMoveDirection {
        dir_x: -1000,
        dir_y: 0,
    },
    RobotMoveDirection {
        dir_x: 0,
        dir_y: -1000,
    },
    RobotMoveDirection {
        dir_x: 707,
        dir_y: 707,
    },
    RobotMoveDirection {
        dir_x: -707,
        dir_y: 707,
    },
    RobotMoveDirection {
        dir_x: 707,
        dir_y: -707,
    },
    RobotMoveDirection {
        dir_x: -707,
        dir_y: -707,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game::features::robot_sync) struct RobotMoveDirection {
    pub(in crate::game::features::robot_sync) dir_x: i32,
    pub(in crate::game::features::robot_sync) dir_y: i32,
}

impl RobotMoveDirection {
    pub(in crate::game::features::robot_sync) const ZERO: Self = Self { dir_x: 0, dir_y: 0 };

    pub(in crate::game::features::robot_sync) fn is_zero(self) -> bool {
        self == Self::ZERO
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub(in crate::game::features::robot_sync) struct RobotMovePayload {
    pub(in crate::game::features::robot_sync) version: u32,
    pub(in crate::game::features::robot_sync) seq: u32,
    pub(in crate::game::features::robot_sync) bot_tick: u32,
    pub(in crate::game::features::robot_sync) dir_x: i32,
    pub(in crate::game::features::robot_sync) dir_y: i32,
    pub(in crate::game::features::robot_sync) speed: u32,
}

#[derive(Clone, Debug, Default, Resource, PartialEq, Eq)]
pub(in crate::game::features::robot_sync) struct RobotSyncBotState {
    pub(in crate::game::features::robot_sync) local_bot_slots: usize,
    pub(in crate::game::features::robot_sync) seq: u32,
    pub(in crate::game::features::robot_sync) bot_tick: u32,
    pub(in crate::game::features::robot_sync) last_sent_target_frame: Option<u32>,
    pub(in crate::game::features::robot_sync) last_dir: Option<RobotMoveDirection>,
    pub(in crate::game::features::robot_sync) last_speed: u32,
    pub(in crate::game::features::robot_sync) seed: u32,
    pub(in crate::game::features::robot_sync) seed_player_id: Option<String>,
}

impl RobotSyncBotState {
    pub(in crate::game::features::robot_sync) fn should_send_target_frame(
        &self,
        target_frame: u32,
        input_interval_frames: u32,
    ) -> bool {
        let interval = input_interval_frames.max(1);
        match self.last_sent_target_frame {
            Some(last_target_frame) if target_frame <= last_target_frame => false,
            Some(last_target_frame) => target_frame.saturating_sub(last_target_frame) >= interval,
            None => true,
        }
    }

    pub(in crate::game::features::robot_sync) fn next_move_payload(
        &mut self,
        player_id: &str,
        speed: u32,
    ) -> RobotMovePayload {
        self.ensure_seed(player_id);
        let direction = direction_for_seed_and_tick(self.seed, self.bot_tick);
        self.next_move_payload_for_direction(direction, speed)
    }

    pub(in crate::game::features::robot_sync) fn next_move_payload_for_direction(
        &mut self,
        direction: RobotMoveDirection,
        speed: u32,
    ) -> RobotMovePayload {
        self.seq = self.seq.saturating_add(1);
        let payload = RobotMovePayload {
            version: ROBOT_MOVE_PAYLOAD_VERSION,
            seq: self.seq,
            bot_tick: self.bot_tick,
            dir_x: direction.dir_x,
            dir_y: direction.dir_y,
            speed,
        };
        self.bot_tick = self.bot_tick.saturating_add(1);
        self.last_dir = Some(direction);
        self.last_speed = speed;
        payload
    }

    pub(in crate::game::features::robot_sync) fn mark_sent_target_frame(
        &mut self,
        target_frame: u32,
    ) {
        self.last_sent_target_frame = Some(target_frame);
    }

    pub(in crate::game::features::robot_sync) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(in crate::game::features::robot_sync) fn last_input_matches(
        &self,
        direction: RobotMoveDirection,
        speed: u32,
    ) -> bool {
        self.last_dir == Some(direction) && self.last_speed == speed
    }

    pub(in crate::game::features::robot_sync) fn last_input_was_stop_or_none(&self) -> bool {
        self.last_dir.is_none_or(RobotMoveDirection::is_zero) && self.last_speed == 0
    }

    fn ensure_seed(&mut self, player_id: &str) {
        if self.seed_player_id.as_deref() == Some(player_id) {
            return;
        }

        let local_bot_slots = self.local_bot_slots;
        *self = Self {
            local_bot_slots,
            seed: stable_player_seed(player_id),
            seed_player_id: Some(player_id.to_string()),
            ..Self::default()
        };
    }
}

pub(in crate::game::features::robot_sync) fn clear_robot_sync_bots(state: &mut RobotSyncBotState) {
    state.clear();
}

fn direction_for_seed_and_tick(seed: u32, bot_tick: u32) -> RobotMoveDirection {
    let segment = bot_tick / BOT_DIRECTION_SEGMENT_TICKS.max(1);
    let index = (seed as usize).wrapping_add(segment as usize) % BOT_DIRECTIONS.len();
    BOT_DIRECTIONS[index]
}

fn stable_player_seed(player_id: &str) -> u32 {
    player_id
        .as_bytes()
        .iter()
        .fold(2166136261_u32, |hash, byte| {
            (hash ^ u32::from(*byte)).wrapping_mul(16777619)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn robot_sync_payload_json_uses_camel_case_integer_fields() {
        let payload = RobotMovePayload {
            version: 1,
            seq: 12,
            bot_tick: 40,
            dir_x: -707,
            dir_y: 707,
            speed: 10000,
        };

        let json = serde_json::to_string(&payload).unwrap();
        let value = serde_json::from_str::<Value>(&json).unwrap();
        let object = value.as_object().unwrap();

        assert_eq!(object.len(), 6);
        assert_eq!(object.get("version").and_then(Value::as_i64), Some(1));
        assert_eq!(object.get("seq").and_then(Value::as_i64), Some(12));
        assert_eq!(object.get("botTick").and_then(Value::as_i64), Some(40));
        assert_eq!(object.get("dirX").and_then(Value::as_i64), Some(-707));
        assert_eq!(object.get("dirY").and_then(Value::as_i64), Some(707));
        assert_eq!(object.get("speed").and_then(Value::as_i64), Some(10000));
        assert!(!object.contains_key("bot_tick"));
        assert!(!object.contains_key("dir_x"));
        assert!(!object.contains_key("dir_y"));
    }

    #[test]
    fn robot_sync_bot_directions_never_exceed_unit_length() {
        let mut state = RobotSyncBotState::default();

        for _ in 0..200 {
            let payload = state.next_move_payload("robot-player-a", 10000);
            let length_squared = i64::from(payload.dir_x) * i64::from(payload.dir_x)
                + i64::from(payload.dir_y) * i64::from(payload.dir_y);

            assert!(length_squared <= 1000 * 1000);
        }
    }

    #[test]
    fn robot_sync_clear_resets_bot_state_to_default() {
        let mut state = RobotSyncBotState::default();
        let _ = state.next_move_payload("robot-player-a", 10000);
        state.mark_sent_target_frame(42);
        state.local_bot_slots = 3;

        clear_robot_sync_bots(&mut state);

        assert_eq!(state, RobotSyncBotState::default());
    }
}
