use std::env;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    framework::network::NetworkTransport,
    game::{
        authority::{
            AuthorityCommand, AuthorityEndpoint, AuthorityEvent, AuthorityFrame, AuthorityRole,
            AuthoritySession,
        },
        myserver::{MyServerCommand, MyServerEvent},
    },
};

use super::{
    config::{
        TouchLaunchMode, TouchSyncConfig, authority_endpoint_from_env, env_bool, env_transport,
        touch_input_delay_frames,
    },
    input::{TouchInputState, TouchSamplePayload, TouchSamplePhase},
    visual::{TouchPlayerState, TouchReplayState, TouchVisualKey},
};

const UI_TOUCH_ACTION: &str = "ui_touch";
const LOCAL_TOUCH_POINTER_ID: u32 = 0;
const UI_TOUCH_POLICY_ID: &str = "ui_touch_room";
const REMOTE_TOUCH_IDLE_TIMEOUT_SECS: f32 = 0.35;

#[derive(Clone, Debug, Default, Resource)]
pub(super) struct TouchMyServerJoinState {
    join_sent: bool,
    ready_sent: bool,
    start_sent: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TouchInputPayload {
    version: u8,
    seq: u32,
    space: String,
    pointer_id: u32,
    pressed: bool,
    samples: Vec<TouchSamplePayload>,
}

pub(super) fn start_touch_sync(
    config: Res<TouchSyncConfig>,
    launch_mode: Res<TouchLaunchMode>,
    session: Res<AuthoritySession>,
    mut authority_commands: MessageWriter<AuthorityCommand>,
    mut myserver_commands: MessageWriter<MyServerCommand>,
) {
    if *launch_mode == TouchLaunchMode::SinglePlayer {
        myserver_commands.write(MyServerCommand::Disconnect);
        if session.role.is_some_and(|role| role != AuthorityRole::None) {
            authority_commands.write(AuthorityCommand::Leave);
        }
        authority_commands.write(AuthorityCommand::HostLocal {
            player_id: config.local_player_id.clone(),
        });
        return;
    }

    if session.role.is_none() || session.role == Some(AuthorityRole::None) {
        if let Some(endpoint) = authority_endpoint_from_env() {
            let is_myserver = matches!(endpoint, AuthorityEndpoint::MyServer { .. });
            authority_commands.write(AuthorityCommand::Join {
                player_id: config.local_player_id.clone(),
                endpoint,
            });
            if is_myserver {
                myserver_commands.write(MyServerCommand::GuestLogin {
                    guest_id: env::var("MYSERVER_GUEST_ID")
                        .ok()
                        .filter(|value| !value.trim().is_empty()),
                    connect_game: true,
                });
            }
        } else if env_bool("TOUCH_MYSERVER_AUTO_JOIN", false) {
            authority_commands.write(AuthorityCommand::Join {
                player_id: config.local_player_id.clone(),
                endpoint: AuthorityEndpoint::MyServer {
                    host: None,
                    port: None,
                    transport: env_transport("MYSERVER_TRANSPORT").unwrap_or(NetworkTransport::Tcp),
                },
            });
            myserver_commands.write(MyServerCommand::GuestLogin {
                guest_id: env::var("MYSERVER_GUEST_ID")
                    .ok()
                    .filter(|value| !value.trim().is_empty()),
                connect_game: true,
            });
        } else if config.auto_start_local_authority {
            authority_commands.write(AuthorityCommand::HostLocal {
                player_id: config.local_player_id.clone(),
            });
        }
    }
}
pub(super) fn reset_touch_sync_state(
    mut input_state: ResMut<TouchInputState>,
    mut replay_state: ResMut<TouchReplayState>,
    mut myserver_join_state: ResMut<TouchMyServerJoinState>,
) {
    *input_state = TouchInputState::default();
    replay_state.players.clear();
    *myserver_join_state = TouchMyServerJoinState::default();
}

pub(super) fn follow_touch_myserver_events(
    config: Res<TouchSyncConfig>,
    mut state: ResMut<TouchMyServerJoinState>,
    mut events: MessageReader<MyServerEvent>,
    mut commands: MessageWriter<MyServerCommand>,
) {
    for event in events.read() {
        match event {
            MyServerEvent::Authenticated { .. } if !state.join_sent => {
                state.join_sent = true;
                info!(
                    room_id = %config.myserver_room_id,
                    policy_id = UI_TOUCH_POLICY_ID,
                    "joining ui touch room"
                );
                commands.write(MyServerCommand::JoinRoom {
                    room_id: config.myserver_room_id.clone(),
                    policy_id: UI_TOUCH_POLICY_ID.to_string(),
                });
            }
            MyServerEvent::RoomJoined(response) if response.ok && !state.ready_sent => {
                state.ready_sent = true;
                info!(room_id = %response.room_id, "ui touch room joined");
                commands.write(MyServerCommand::SetReady { ready: true });
            }
            MyServerEvent::ReadyChanged(response) if response.ok && !state.start_sent => {
                state.start_sent = true;
                info!("starting ui touch room");
                commands.write(MyServerCommand::StartRoom);
            }
            MyServerEvent::PlayerInputAccepted(response) => {
                if response.ok {
                    debug!(room_id = %response.room_id, "ui touch input accepted");
                } else {
                    warn!(
                        room_id = %response.room_id,
                        error_code = %response.error_code,
                        "ui touch input rejected"
                    );
                }
            }
            _ => {}
        }
    }
}

pub(super) fn send_local_touch_input(
    session: Res<AuthoritySession>,
    mut input_state: ResMut<TouchInputState>,
    mut authority_commands: MessageWriter<AuthorityCommand>,
) {
    if session.local_player_id.is_none() || input_state.pending_samples.is_empty() {
        return;
    }

    if input_state.sent_sample_count == input_state.pending_samples.len()
        && input_state.sent_pressed == input_state.pending_pressed
    {
        return;
    }

    let target_frame = session.frame_id.saturating_add(touch_input_delay_frames());
    if target_frame <= input_state.last_sent_target_frame {
        return;
    }

    let samples = input_state
        .pending_samples
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let payload = TouchInputPayload {
        version: 1,
        seq: input_state.pending_seq,
        space: "viewport01".to_string(),
        pointer_id: LOCAL_TOUCH_POINTER_ID,
        pressed: input_state.pending_pressed,
        samples,
    };

    let Ok(payload_json) = serde_json::to_string(&payload) else {
        return;
    };

    debug!(
        frame_id = target_frame,
        seq = payload.seq,
        pressed = payload.pressed,
        sample_count = payload.samples.len(),
        "sending ui touch input"
    );
    authority_commands.write(AuthorityCommand::SendInput {
        frame_id: target_frame,
        action: UI_TOUCH_ACTION.to_string(),
        payload_json,
    });

    input_state.last_sent_target_frame = target_frame;
    input_state.sent_sample_count = input_state.pending_samples.len();
    input_state.sent_pressed = input_state.pending_pressed;
    while input_state.pending_samples.len() > 1 {
        input_state.pending_samples.pop_front();
        input_state.sent_sample_count = input_state.sent_sample_count.saturating_sub(1);
    }
}

pub(super) fn apply_authority_touch_frames(
    mut events: MessageReader<AuthorityEvent>,
    mut replay_state: ResMut<TouchReplayState>,
) {
    for event in events.read() {
        match event {
            AuthorityEvent::FrameApplied { frame } => {
                apply_touch_frame(&mut replay_state, frame);
            }
            AuthorityEvent::Snapshot { snapshot } => {
                apply_touch_snapshot(&mut replay_state, &snapshot.game_state_json);
            }
            _ => {}
        }
    }
}

pub(super) fn release_idle_remote_touches(
    time: Res<Time>,
    mut replay_state: ResMut<TouchReplayState>,
) {
    for state in replay_state.players.values_mut() {
        if !state.was_pressed {
            continue;
        }
        state.idle_age += time.delta_secs();
        if state.idle_age >= REMOTE_TOUCH_IDLE_TIMEOUT_SECS {
            state.release();
        }
    }
}

fn apply_touch_frame(replay_state: &mut TouchReplayState, frame: &AuthorityFrame) {
    for input in &frame.inputs {
        if input.action != UI_TOUCH_ACTION {
            continue;
        }
        let Ok(payload) = serde_json::from_str::<TouchInputPayload>(&input.payload_json) else {
            continue;
        };

        let key = TouchVisualKey {
            player_id: input.player_id.clone(),
            pointer_id: payload.pointer_id,
        };
        let state = replay_state.players.entry(key).or_insert_with(|| {
            let initial = payload
                .samples
                .last()
                .copied()
                .map(|sample| Vec2::new(sample.x.clamp(0.0, 1.0), sample.y.clamp(0.0, 1.0)))
                .unwrap_or(Vec2::splat(0.5));
            TouchPlayerState::new(input.player_id.clone(), initial, frame.frame_id)
        });

        let samples_is_empty = payload.samples.is_empty();
        for sample in &payload.samples {
            state.apply_sample(*sample, payload.pressed, frame.frame_id);
        }

        if !samples_is_empty {
            debug!(
                frame_id = frame.frame_id,
                player_id = %input.player_id,
                seq = payload.seq,
                pressed = payload.pressed,
                sample_count = payload.samples.len(),
                last_phase = ?payload.samples.last().map(|sample| sample.phase),
                "applied ui touch frame"
            );
        }

        if samples_is_empty && !payload.pressed {
            state.release();
        }
    }
}

fn apply_touch_snapshot(replay_state: &mut TouchReplayState, game_state_json: &str) {
    if game_state_json.trim().is_empty() || game_state_json.trim() == "{}" {
        return;
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SnapshotState {
        #[serde(default)]
        players: Vec<SnapshotPlayer>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SnapshotPlayer {
        player_id: String,
        frame_id: u32,
        pointer_id: u32,
        pressed: bool,
        x: f32,
        y: f32,
    }

    let Ok(snapshot) = serde_json::from_str::<SnapshotState>(game_state_json) else {
        return;
    };

    for player in snapshot.players {
        let key = TouchVisualKey {
            player_id: player.player_id.clone(),
            pointer_id: player.pointer_id,
        };
        let position = Vec2::new(player.x.clamp(0.0, 1.0), player.y.clamp(0.0, 1.0));
        let state = replay_state.players.entry(key).or_insert_with(|| {
            TouchPlayerState::new(player.player_id.clone(), position, player.frame_id)
        });
        state.apply_sample(
            TouchSamplePayload {
                phase: if player.pressed {
                    TouchSamplePhase::Move
                } else {
                    TouchSamplePhase::Up
                },
                x: position.x,
                y: position.y,
            },
            player.pressed,
            player.frame_id,
        );
    }
}
