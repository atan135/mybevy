use std::collections::VecDeque;

use bevy::{input::touch::Touches, prelude::*, window::PrimaryWindow};
use serde::{Deserialize, Serialize};

use crate::{authority::AuthoritySession, framework::ui::core::UiInputState};

const MAX_PENDING_TOUCH_SAMPLES: usize = 64;

#[derive(Clone, Debug, Default, Resource)]
pub(super) struct TouchInputState {
    pub(super) pressed: bool,
    pub(super) last_position: Option<Vec2>,
    pub(super) pending_samples: VecDeque<TouchSamplePayload>,
    pub(super) next_seq: u32,
    pub(super) pending_seq: u32,
    pub(super) pending_pressed: bool,
    pub(super) sent_sample_count: usize,
    pub(super) sent_pressed: bool,
    pub(super) last_sent_target_frame: u32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TouchSamplePhase {
    Down,
    Move,
    Up,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TouchSamplePayload {
    pub(super) phase: TouchSamplePhase,
    pub(super) x: f32,
    pub(super) y: f32,
}
pub(super) fn capture_local_touch_input(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    window: Single<&Window, With<PrimaryWindow>>,
    session: Res<AuthoritySession>,
    ui_input: Res<UiInputState>,
    mut input_state: ResMut<TouchInputState>,
) {
    if session.local_player_id.is_none() {
        return;
    }

    let Some(screen_position) = active_screen_position(&mouse_buttons, &touches, &window) else {
        release_local_touch_input(&mut input_state, session.frame_id);
        return;
    };

    if ui_input.pointer_blocked {
        release_local_touch_input(&mut input_state, session.frame_id);
        return;
    }

    let window_size = window.size();
    if window_size.x <= 0.0 || window_size.y <= 0.0 {
        return;
    }

    let viewport_position = Vec2::new(
        (screen_position.x / window_size.x).clamp(0.0, 1.0),
        (screen_position.y / window_size.y).clamp(0.0, 1.0),
    );
    let phase = if input_state.pressed {
        TouchSamplePhase::Move
    } else {
        TouchSamplePhase::Down
    };

    input_state.pressed = true;
    input_state.last_position = Some(viewport_position);
    queue_touch_sample(
        &mut input_state,
        session.frame_id,
        phase,
        viewport_position,
        true,
    );
}

fn release_local_touch_input(input_state: &mut TouchInputState, frame_id: u32) {
    if !input_state.pressed {
        return;
    }

    input_state.pressed = false;
    if let Some(last_position) = input_state.last_position {
        queue_touch_sample(
            input_state,
            frame_id,
            TouchSamplePhase::Up,
            last_position,
            false,
        );
    }
}

fn queue_touch_sample(
    input_state: &mut TouchInputState,
    current_frame_id: u32,
    phase: TouchSamplePhase,
    position: Vec2,
    pressed: bool,
) {
    if matches!(phase, TouchSamplePhase::Down) {
        input_state.next_seq = input_state.next_seq.saturating_add(1);
        input_state.pending_seq = input_state.next_seq;
        input_state.pending_samples.clear();
        input_state.sent_sample_count = 0;
        input_state.sent_pressed = false;
    }

    input_state.pending_pressed = pressed;
    input_state.pending_samples.push_back(TouchSamplePayload {
        phase,
        x: position.x,
        y: position.y,
    });
    while input_state.pending_samples.len() > MAX_PENDING_TOUCH_SAMPLES {
        input_state.pending_samples.pop_front();
        input_state.sent_sample_count = input_state.sent_sample_count.saturating_sub(1);
    }

    debug!(
        current_frame_id,
        seq = input_state.pending_seq,
        ?phase,
        pressed,
        pending_count = input_state.pending_samples.len(),
        "queued ui touch sample"
    );
}

fn active_screen_position(
    mouse_buttons: &ButtonInput<MouseButton>,
    touches: &Touches,
    window: &Window,
) -> Option<Vec2> {
    touches
        .first_pressed_position()
        .or_else(|| {
            touches
                .iter_just_released()
                .next()
                .map(|touch| touch.position())
        })
        .or_else(|| {
            (mouse_buttons.just_pressed(MouseButton::Left)
                || mouse_buttons.pressed(MouseButton::Left)
                || mouse_buttons.just_released(MouseButton::Left))
            .then(|| window.cursor_position())
            .flatten()
        })
}
