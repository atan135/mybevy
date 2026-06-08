use std::collections::{HashMap, VecDeque};
use std::env;

use bevy::{
    asset::RenderAssetUsages,
    input::touch::Touches,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    window::PrimaryWindow,
};
use serde::{Deserialize, Serialize};

use crate::authority::{
    AuthorityCommand, AuthorityEndpoint, AuthorityEvent, AuthorityFrame, AuthorityRole,
    AuthoritySession,
};
use crate::myserver::{MyServerCommand, MyServerEvent};
use crate::network::NetworkTransport;

use super::{
    navigation::AppUiMode,
    screens::ScreensPlugin,
    ui::input::{UiInputState, UiInputSystems},
};

const UI_TOUCH_ACTION: &str = "ui_touch";
const LOCAL_TOUCH_POINTER_ID: u32 = 0;
const DEFAULT_TOUCH_PLAYER_ID: &str = "touch-local";
const DEFAULT_UI_TOUCH_ROOM_ID: &str = "ui-touch-room";
const UI_TOUCH_POLICY_ID: &str = "ui_touch_room";
const BACKGROUND_PALETTE: [Vec3; 3] = [
    Vec3::new(0.08, 0.16, 0.30),
    Vec3::new(0.08, 0.31, 0.27),
    Vec3::new(0.28, 0.12, 0.31),
];
const BACKGROUND_CYCLE_SECONDS: f32 = 9.0;
const GRADIENT_TEXTURE_SIZE: u32 = 256;
const PRESS_DIAMETER_SCREEN_RATIO: f32 = 1.0 / 3.0;
const PRESS_DISC_ALPHA: f32 = 0.58;
const PRESS_START_SCALE: f32 = 0.92;
const PRESS_RELEASE_SCALE: f32 = 0.96;
const PRESS_SCALE_SPEED: f32 = 18.0;
const PULSE_EXTRA_SCALE: f32 = 0.16;
const PULSE_DURATION_SECS: f32 = 0.24;
const PULSE_ALPHA: f32 = 0.78;
const RIPPLE_SPACING_RATIO: f32 = 0.18;
const RIPPLE_COOLDOWN_SECS: f32 = 0.035;
const RIPPLE_DURATION_SECS: f32 = 0.55;
const RIPPLE_START_SCALE: f32 = 0.70;
const RIPPLE_END_SCALE: f32 = 1.18;
const RIPPLE_ALPHA: f32 = 0.36;
const RELEASED_DISC_DURATION_SECS: f32 = 0.28;
const POSITION_SMOOTHING: f32 = 18.0;
const FADE_IN_SPEED: f32 = 16.0;
const FADE_OUT_SPEED: f32 = 4.5;
const ALPHA_EPSILON: f32 = 0.01;
const REMOTE_TOUCH_IDLE_TIMEOUT_SECS: f32 = 0.35;
const DEFAULT_TOUCH_INPUT_DELAY_FRAMES: u32 = 2;
const MAX_PENDING_TOUCH_SAMPLES: usize = 64;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ScreensPlugin)
            .insert_resource(ClearColor(background_color_at(0.0)))
            .init_resource::<TouchSyncConfig>()
            .init_resource::<TouchInputState>()
            .init_resource::<TouchReplayState>()
            .init_resource::<TouchMyServerJoinState>()
            .init_resource::<TouchLaunchMode>()
            .add_systems(Startup, (setup_camera, setup_touch_assets))
            .add_systems(
                OnEnter(AppUiMode::WanfaTouchRipple),
                setup_touch_ripple_scene,
            )
            .add_systems(OnExit(AppUiMode::WanfaTouchRipple), reset_touch_sync_state)
            .add_systems(
                Update,
                (
                    animate_background,
                    capture_local_touch_input,
                    follow_touch_myserver_events,
                    send_local_touch_input,
                    apply_authority_touch_frames,
                    release_idle_remote_touches,
                    animate_touch_players,
                    spawn_drag_ripples,
                    animate_ripples,
                    animate_released_discs,
                    resize_background,
                )
                    .chain()
                    .after(UiInputSystems::Update)
                    .run_if(in_state(AppUiMode::WanfaTouchRipple)),
            );
    }
}

#[derive(Component)]
struct Background;

#[derive(Component)]
struct PlayerTouchVisual {
    key: TouchVisualKey,
}

#[derive(Component)]
struct GradientSprite;

#[derive(Component)]
struct PulseSprite;

#[derive(Component)]
struct Ripple {
    age: f32,
    duration: f32,
    start_diameter: f32,
    end_diameter: f32,
    color: Color,
}

#[derive(Component)]
struct ReleasedDisc {
    age: f32,
    duration: f32,
    start_alpha: f32,
    diameter: f32,
    color: Color,
}

#[derive(Resource)]
struct DiscImage(Handle<Image>);

#[derive(Resource)]
struct RippleImage(Handle<Image>);

#[derive(Clone, Debug, Resource)]
struct TouchSyncConfig {
    auto_start_local_authority: bool,
    local_player_id: String,
    myserver_room_id: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Resource)]
pub(in crate::game) enum TouchLaunchMode {
    #[default]
    Auto,
    SinglePlayer,
}

impl Default for TouchSyncConfig {
    fn default() -> Self {
        Self {
            auto_start_local_authority: env_bool("TOUCH_AUTO_LOCAL_AUTHORITY", true),
            local_player_id: env_string("TOUCH_PLAYER_ID", DEFAULT_TOUCH_PLAYER_ID),
            myserver_room_id: env_string("TOUCH_ROOM_ID", DEFAULT_UI_TOUCH_ROOM_ID),
        }
    }
}

#[derive(Clone, Debug, Default, Resource)]
struct TouchInputState {
    pressed: bool,
    last_position: Option<Vec2>,
    pending_samples: VecDeque<TouchSamplePayload>,
    next_seq: u32,
    pending_seq: u32,
    pending_pressed: bool,
    sent_sample_count: usize,
    sent_pressed: bool,
    last_sent_target_frame: u32,
}

#[derive(Clone, Debug, Resource)]
struct TouchReplayState {
    players: HashMap<TouchVisualKey, TouchPlayerState>,
}

impl Default for TouchReplayState {
    fn default() -> Self {
        Self {
            players: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Resource)]
struct TouchMyServerJoinState {
    join_sent: bool,
    ready_sent: bool,
    start_sent: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct TouchVisualKey {
    player_id: String,
    pointer_id: u32,
}

#[derive(Clone, Debug)]
struct TouchPlayerState {
    position: Vec2,
    target_position: Vec2,
    intensity: f32,
    target_intensity: f32,
    pulse_age: f32,
    was_pressed: bool,
    last_ripple_position: Option<Vec2>,
    ripple_cooldown: f32,
    press_scale: f32,
    target_press_scale: f32,
    last_frame_id: u32,
    idle_age: f32,
    color: Color,
    release_disc: Option<ReleaseDiscRequest>,
}

#[derive(Clone, Copy, Debug)]
struct ReleaseDiscRequest {
    position: Vec2,
    intensity: f32,
    press_scale: f32,
}

impl TouchPlayerState {
    fn new(player_id: String, position: Vec2, frame_id: u32) -> Self {
        Self {
            color: player_color(&player_id),
            position,
            target_position: position,
            intensity: 0.0,
            target_intensity: 0.0,
            pulse_age: PULSE_DURATION_SECS,
            was_pressed: false,
            last_ripple_position: None,
            ripple_cooldown: 0.0,
            press_scale: PRESS_START_SCALE,
            target_press_scale: PRESS_START_SCALE,
            last_frame_id: frame_id,
            idle_age: 0.0,
            release_disc: None,
        }
    }

    fn apply_sample(&mut self, sample: TouchSamplePayload, frame_pressed: bool, frame_id: u32) {
        let was_pressed = self.was_pressed;
        let sample_pressed = match sample.phase {
            TouchSamplePhase::Down | TouchSamplePhase::Move => true,
            TouchSamplePhase::Up => false,
        };
        self.target_position = Vec2::new(sample.x.clamp(0.0, 1.0), sample.y.clamp(0.0, 1.0));
        self.target_intensity = if sample_pressed { 1.0 } else { 0.0 };
        self.target_press_scale = if sample_pressed {
            1.0
        } else {
            PRESS_RELEASE_SCALE
        };
        self.last_frame_id = frame_id;
        self.idle_age = 0.0;

        if sample_pressed && !self.was_pressed {
            self.position = self.target_position;
            self.intensity = 0.0;
            self.target_intensity = 1.0;
            self.pulse_age = 0.0;
            self.last_ripple_position = None;
            self.ripple_cooldown = 0.0;
            self.press_scale = PRESS_START_SCALE;
        }

        if !sample_pressed && was_pressed {
            let release_intensity = if self.intensity > ALPHA_EPSILON {
                self.intensity
            } else {
                1.0
            };
            self.release_disc = Some(ReleaseDiscRequest {
                position: self.target_position,
                intensity: release_intensity,
                press_scale: self.press_scale,
            });
        }

        self.was_pressed = sample_pressed || frame_pressed;
        if !self.was_pressed {
            self.last_ripple_position = None;
        }
    }

    fn release(&mut self) {
        if self.was_pressed && self.intensity > ALPHA_EPSILON {
            self.release_disc = Some(ReleaseDiscRequest {
                position: self.target_position,
                intensity: self.intensity,
                press_scale: self.press_scale,
            });
        }
        self.target_intensity = 0.0;
        self.was_pressed = false;
        self.last_ripple_position = None;
        self.target_press_scale = PRESS_RELEASE_SCALE;
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TouchSamplePhase {
    Down,
    Move,
    Up,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TouchSamplePayload {
    phase: TouchSamplePhase,
    x: f32,
    y: f32,
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

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn setup_touch_assets(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let disc_texture = images.add(create_disc_image());
    let ring_texture = images.add(create_ring_image());
    commands.insert_resource(DiscImage(disc_texture));
    commands.insert_resource(RippleImage(ring_texture));
}

fn setup_touch_ripple_scene(
    mut commands: Commands,
    config: Res<TouchSyncConfig>,
    launch_mode: Res<TouchLaunchMode>,
    session: Res<AuthoritySession>,
    mut authority_commands: MessageWriter<AuthorityCommand>,
    mut myserver_commands: MessageWriter<MyServerCommand>,
    mut clear_color: ResMut<ClearColor>,
) {
    clear_color.0 = background_color_at(0.0);

    commands.spawn((
        DespawnOnExit(AppUiMode::WanfaTouchRipple),
        Sprite::from_color(background_color_at(0.0), Vec2::ONE),
        Transform::from_xyz(0.0, 0.0, -1.0),
        Background,
    ));

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

fn reset_touch_sync_state(
    mut input_state: ResMut<TouchInputState>,
    mut replay_state: ResMut<TouchReplayState>,
    mut myserver_join_state: ResMut<TouchMyServerJoinState>,
) {
    *input_state = TouchInputState::default();
    replay_state.players.clear();
    *myserver_join_state = TouchMyServerJoinState::default();
}

fn animate_background(
    time: Res<Time>,
    mut clear_color: ResMut<ClearColor>,
    mut background: Single<&mut Sprite, With<Background>>,
) {
    let color = background_color_at(time.elapsed_secs());

    clear_color.0 = color;
    background.color = color;
}

fn capture_local_touch_input(
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

fn follow_touch_myserver_events(
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

fn send_local_touch_input(
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

fn apply_authority_touch_frames(
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

fn release_idle_remote_touches(time: Res<Time>, mut replay_state: ResMut<TouchReplayState>) {
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

fn animate_touch_players(
    mut commands: Commands,
    time: Res<Time>,
    window: Single<&Window, With<PrimaryWindow>>,
    disc_image: Res<DiscImage>,
    ripple_image: Res<RippleImage>,
    mut replay_state: ResMut<TouchReplayState>,
    mut gradients: Query<
        (&PlayerTouchVisual, &mut Transform, &mut Sprite),
        (With<GradientSprite>, Without<PulseSprite>),
    >,
    mut pulses: Query<
        (&PlayerTouchVisual, &mut Transform, &mut Sprite),
        (With<PulseSprite>, Without<GradientSprite>),
    >,
) {
    let existing_keys = gradients
        .iter()
        .map(|(visual, _, _)| visual.key.clone())
        .collect::<Vec<_>>();
    for key in replay_state.players.keys() {
        if !existing_keys.iter().any(|existing| existing == key) {
            spawn_touch_visuals(&mut commands, &disc_image, &ripple_image, key.clone());
        }
    }

    for state in replay_state.players.values_mut() {
        animate_touch_state(time.delta_secs(), &window, state);
        if let Some(release) = state.release_disc.take() {
            spawn_released_disc(
                &mut commands,
                &disc_image,
                viewport_to_world(release.position, window.size()),
                release.intensity,
                press_diameter(window.size()) * release.press_scale,
                state.color,
            );
        }
    }

    for (visual, mut transform, mut sprite) in &mut gradients {
        let Some(state) = replay_state.players.get(&visual.key) else {
            sprite.color = sprite.color.with_alpha(0.0);
            continue;
        };
        let world_position = viewport_to_world(state.position, window.size());
        let press_diameter = press_diameter(window.size());
        transform.translation.x = world_position.x;
        transform.translation.y = world_position.y;
        sprite.custom_size = Some(Vec2::splat(press_diameter * state.press_scale));
        sprite.color = state.color.with_alpha(state.intensity * PRESS_DISC_ALPHA);
    }

    for (visual, mut transform, mut sprite) in &mut pulses {
        let Some(state) = replay_state.players.get(&visual.key) else {
            sprite.color = sprite.color.with_alpha(0.0);
            continue;
        };
        let world_position = viewport_to_world(state.position, window.size());
        let press_diameter = press_diameter(window.size());
        let pulse_progress = (state.pulse_age / PULSE_DURATION_SECS).clamp(0.0, 1.0);
        let pulse_alpha = (1.0 - smoothstep(pulse_progress)) * state.intensity * PULSE_ALPHA;
        let pulse_scale = 1.0 + PULSE_EXTRA_SCALE * smoothstep(pulse_progress);

        transform.translation.x = world_position.x;
        transform.translation.y = world_position.y;
        sprite.custom_size = Some(Vec2::splat(press_diameter * pulse_scale));
        sprite.color = state.color.with_alpha(pulse_alpha);
    }
}

fn spawn_drag_ripples(
    mut commands: Commands,
    time: Res<Time>,
    ripple_image: Res<RippleImage>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut replay_state: ResMut<TouchReplayState>,
) {
    let press_diameter = press_diameter(window.size());
    let spacing = press_diameter * RIPPLE_SPACING_RATIO;

    for state in replay_state.players.values_mut() {
        if state.target_intensity <= 0.0 {
            state.last_ripple_position = None;
            state.ripple_cooldown = 0.0;
            continue;
        }

        state.ripple_cooldown = (state.ripple_cooldown - time.delta_secs()).max(0.0);

        let current_position = viewport_to_world(state.target_position, window.size());
        let Some(last_position) = state.last_ripple_position else {
            state.last_ripple_position = Some(current_position);
            continue;
        };

        if last_position.distance(current_position) < spacing || state.ripple_cooldown > 0.0 {
            continue;
        }

        commands.spawn((
            DespawnOnExit(AppUiMode::WanfaTouchRipple),
            Sprite {
                image: ripple_image.0.clone(),
                color: state.color.with_alpha(RIPPLE_ALPHA),
                custom_size: Some(Vec2::splat(press_diameter * RIPPLE_START_SCALE)),
                ..Default::default()
            },
            Transform::from_xyz(current_position.x, current_position.y, 0.05),
            Ripple {
                age: 0.0,
                duration: RIPPLE_DURATION_SECS,
                start_diameter: press_diameter * RIPPLE_START_SCALE,
                end_diameter: press_diameter * RIPPLE_END_SCALE,
                color: state.color,
            },
        ));

        state.last_ripple_position = Some(current_position);
        state.ripple_cooldown = RIPPLE_COOLDOWN_SECS;
    }
}

fn animate_ripples(
    mut commands: Commands,
    time: Res<Time>,
    mut ripples: Query<(Entity, &mut Ripple, &mut Sprite)>,
) {
    for (entity, mut ripple, mut sprite) in &mut ripples {
        ripple.age += time.delta_secs();

        let progress = (ripple.age / ripple.duration).clamp(0.0, 1.0);
        let eased = smoothstep(progress);
        let diameter =
            ripple.start_diameter + (ripple.end_diameter - ripple.start_diameter) * eased;
        let alpha = (1.0 - eased) * RIPPLE_ALPHA;

        sprite.custom_size = Some(Vec2::splat(diameter));
        sprite.color = ripple.color.with_alpha(alpha);

        if progress >= 1.0 {
            commands.entity(entity).despawn();
        }
    }
}

fn animate_released_discs(
    mut commands: Commands,
    time: Res<Time>,
    mut released_discs: Query<(Entity, &mut ReleasedDisc, &mut Sprite)>,
) {
    for (entity, mut released_disc, mut sprite) in &mut released_discs {
        released_disc.age += time.delta_secs();

        let progress = (released_disc.age / released_disc.duration).clamp(0.0, 1.0);
        let eased = smoothstep(progress);
        let alpha = (1.0 - eased) * released_disc.start_alpha;

        sprite.custom_size = Some(Vec2::splat(released_disc.diameter));
        sprite.color = released_disc.color.with_alpha(alpha);

        if progress >= 1.0 {
            commands.entity(entity).despawn();
        }
    }
}

fn resize_background(
    window: Single<&Window, With<PrimaryWindow>>,
    mut background: Single<&mut Sprite, With<Background>>,
) {
    let size = window.size();
    if size.x <= 0.0 || size.y <= 0.0 {
        return;
    }

    background.custom_size = Some(size);
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

fn animate_touch_state(delta: f32, window: &Window, state: &mut TouchPlayerState) {
    let previous_pressed = state.intensity > ALPHA_EPSILON;
    let previous_position = state.position;
    state.position = state.position.lerp(
        state.target_position,
        smoothing_factor(POSITION_SMOOTHING, delta),
    );

    let fade_speed = if state.target_intensity > state.intensity {
        FADE_IN_SPEED
    } else {
        FADE_OUT_SPEED
    };
    state.intensity = state
        .intensity
        .lerp(state.target_intensity, smoothing_factor(fade_speed, delta));

    if state.target_intensity == 0.0 && state.intensity < ALPHA_EPSILON {
        state.intensity = 0.0;
        state.target_press_scale = PRESS_START_SCALE;
    }

    state.press_scale = state.press_scale.lerp(
        state.target_press_scale,
        smoothing_factor(PRESS_SCALE_SPEED, delta),
    );

    if previous_pressed && state.target_intensity == 0.0 && state.intensity > ALPHA_EPSILON {
        state.position = previous_position;
    }

    state.pulse_age = (state.pulse_age + delta).min(PULSE_DURATION_SECS);

    if !state.was_pressed && state.target_intensity == 0.0 {
        let _ = window;
    }
}

fn spawn_touch_visuals(
    commands: &mut Commands,
    disc_image: &DiscImage,
    ripple_image: &RippleImage,
    key: TouchVisualKey,
) {
    commands.spawn((
        DespawnOnExit(AppUiMode::WanfaTouchRipple),
        Sprite {
            image: disc_image.0.clone(),
            color: Color::WHITE.with_alpha(0.0),
            custom_size: Some(Vec2::ONE),
            ..Default::default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
        PlayerTouchVisual { key: key.clone() },
        GradientSprite,
    ));

    commands.spawn((
        DespawnOnExit(AppUiMode::WanfaTouchRipple),
        Sprite {
            image: ripple_image.0.clone(),
            color: Color::WHITE.with_alpha(0.0),
            custom_size: Some(Vec2::ONE),
            ..Default::default()
        },
        Transform::from_xyz(0.0, 0.0, 0.1),
        PlayerTouchVisual { key },
        PulseSprite,
    ));
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

fn spawn_released_disc(
    commands: &mut Commands,
    disc_image: &DiscImage,
    position: Vec2,
    intensity: f32,
    diameter: f32,
    color: Color,
) {
    commands.spawn((
        DespawnOnExit(AppUiMode::WanfaTouchRipple),
        Sprite {
            image: disc_image.0.clone(),
            color: color.with_alpha(intensity * PRESS_DISC_ALPHA),
            custom_size: Some(Vec2::splat(diameter)),
            ..Default::default()
        },
        Transform::from_xyz(position.x, position.y, 0.0),
        ReleasedDisc {
            age: 0.0,
            duration: RELEASED_DISC_DURATION_SECS,
            start_alpha: intensity * PRESS_DISC_ALPHA,
            diameter,
            color,
        },
    ));
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

fn viewport_to_world(position: Vec2, window_size: Vec2) -> Vec2 {
    Vec2::new(
        position.x * window_size.x - window_size.x * 0.5,
        window_size.y * 0.5 - position.y * window_size.y,
    )
}

fn press_diameter(window_size: Vec2) -> f32 {
    window_size.x.min(window_size.y) * PRESS_DIAMETER_SCREEN_RATIO
}

fn smoothing_factor(speed: f32, delta_secs: f32) -> f32 {
    1.0 - (-speed * delta_secs).exp()
}

fn background_color_at(elapsed_secs: f32) -> Color {
    let rgb = palette_color_at(&BACKGROUND_PALETTE, elapsed_secs);

    Color::srgb(rgb.x, rgb.y, rgb.z)
}

fn palette_color_at(palette: &[Vec3], elapsed_secs: f32) -> Vec3 {
    let position = (elapsed_secs / BACKGROUND_CYCLE_SECONDS).rem_euclid(1.0) * palette.len() as f32;
    let from_index = position.floor() as usize;
    let to_index = (from_index + 1) % palette.len();
    let blend = smoothstep(position.fract());

    palette[from_index].lerp(palette[to_index], blend)
}

fn player_color(player_id: &str) -> Color {
    let mut hash = 0u32;
    for byte in player_id.bytes() {
        hash = hash.wrapping_mul(16777619) ^ u32::from(byte);
    }

    let hue = (hash % 360) as f32;
    Color::hsl(hue, 0.78, 0.62)
}

fn create_disc_image() -> Image {
    let texture_size = Extent3d {
        width: GRADIENT_TEXTURE_SIZE,
        height: GRADIENT_TEXTURE_SIZE,
        depth_or_array_layers: 1,
    };

    let mut image = Image::new_fill(
        texture_size,
        TextureDimension::D2,
        &[255, 255, 255, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );

    let center = Vec2::splat((GRADIENT_TEXTURE_SIZE - 1) as f32 / 2.0);
    let radius = GRADIENT_TEXTURE_SIZE as f32 / 2.0;

    for y in 0..GRADIENT_TEXTURE_SIZE {
        for x in 0..GRADIENT_TEXTURE_SIZE {
            let distance = Vec2::new(x as f32, y as f32).distance(center);
            let alpha = if distance <= radius { 255 } else { 0 };

            let pixel = image.pixel_bytes_mut(UVec3::new(x, y, 0)).unwrap();
            pixel[0] = 255;
            pixel[1] = 255;
            pixel[2] = 255;
            pixel[3] = alpha;
        }
    }

    image
}

fn create_ring_image() -> Image {
    let texture_size = Extent3d {
        width: GRADIENT_TEXTURE_SIZE,
        height: GRADIENT_TEXTURE_SIZE,
        depth_or_array_layers: 1,
    };

    let mut image = Image::new_fill(
        texture_size,
        TextureDimension::D2,
        &[255, 255, 255, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );

    let center = Vec2::splat((GRADIENT_TEXTURE_SIZE - 1) as f32 / 2.0);
    let radius = GRADIENT_TEXTURE_SIZE as f32 / 2.0;
    let inner_radius = radius * 0.90;

    for y in 0..GRADIENT_TEXTURE_SIZE {
        for x in 0..GRADIENT_TEXTURE_SIZE {
            let distance = Vec2::new(x as f32, y as f32).distance(center);
            let alpha = if (inner_radius..=radius).contains(&distance) {
                255
            } else {
                0
            };

            let pixel = image.pixel_bytes_mut(UVec3::new(x, y, 0)).unwrap();
            pixel[0] = 255;
            pixel[1] = 255;
            pixel[2] = 255;
            pixel[3] = alpha;
        }
    }

    image
}

fn smoothstep(value: f32) -> f32 {
    value * value * (3.0 - 2.0 * value)
}

fn authority_endpoint_from_env() -> Option<AuthorityEndpoint> {
    let mode = env::var("TOUCH_AUTHORITY_MODE")
        .ok()
        .unwrap_or_default()
        .to_ascii_lowercase();
    match mode.as_str() {
        "lan-client" | "client" | "remote" => Some(AuthorityEndpoint::Remote {
            host: env_string("AUTHORITY_REMOTE_HOST", "127.0.0.1"),
            port: env_u16("AUTHORITY_REMOTE_PORT", 15000),
            transport: env_transport("AUTHORITY_TRANSPORT").unwrap_or(NetworkTransport::Tcp),
        }),
        "myserver" | "server" => Some(AuthorityEndpoint::MyServer {
            host: None,
            port: None,
            transport: env_transport("MYSERVER_TRANSPORT").unwrap_or(NetworkTransport::Tcp),
        }),
        _ => None,
    }
}

fn env_string(name: &str, default: &str) -> String {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn env_bool(name: &str, default: bool) -> bool {
    env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.as_str(),
                "1" | "true" | "TRUE" | "True" | "yes" | "YES"
            )
        })
        .unwrap_or(default)
}

fn env_u16(name: &str, default: u16) -> u16 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default)
}

fn env_u32(name: &str, default: u32) -> u32 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(default)
}

fn touch_input_delay_frames() -> u32 {
    env_u32("TOUCH_INPUT_DELAY_FRAMES", DEFAULT_TOUCH_INPUT_DELAY_FRAMES).max(1)
}

fn env_transport(name: &str) -> Option<NetworkTransport> {
    match env::var(name).ok()?.trim().to_ascii_lowercase().as_str() {
        "tcp" => Some(NetworkTransport::Tcp),
        "kcp" => Some(NetworkTransport::Kcp),
        _ => None,
    }
}
