use std::collections::HashMap;

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    window::PrimaryWindow,
};

use crate::game::navigation::AppUiMode;

use super::input::{TouchSamplePayload, TouchSamplePhase};
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

#[derive(Component)]
pub(super) struct Background;

#[derive(Component)]
pub(super) struct PlayerTouchVisual {
    key: TouchVisualKey,
}

#[derive(Component)]
pub(super) struct GradientSprite;

#[derive(Component)]
pub(super) struct PulseSprite;

#[derive(Component)]
pub(super) struct Ripple {
    age: f32,
    duration: f32,
    start_diameter: f32,
    end_diameter: f32,
    color: Color,
}

#[derive(Component)]
pub(super) struct ReleasedDisc {
    age: f32,
    duration: f32,
    start_alpha: f32,
    diameter: f32,
    color: Color,
}

#[derive(Resource)]
pub(super) struct DiscImage(Handle<Image>);

#[derive(Resource)]
pub(super) struct RippleImage(Handle<Image>);

#[derive(Clone, Debug, Resource)]
pub(super) struct TouchReplayState {
    pub(super) players: HashMap<TouchVisualKey, TouchPlayerState>,
}

impl Default for TouchReplayState {
    fn default() -> Self {
        Self {
            players: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct TouchVisualKey {
    pub(super) player_id: String,
    pub(super) pointer_id: u32,
}

#[derive(Clone, Debug)]
pub(super) struct TouchPlayerState {
    pub(super) position: Vec2,
    pub(super) target_position: Vec2,
    pub(super) intensity: f32,
    pub(super) target_intensity: f32,
    pub(super) pulse_age: f32,
    pub(super) was_pressed: bool,
    pub(super) last_ripple_position: Option<Vec2>,
    pub(super) ripple_cooldown: f32,
    pub(super) press_scale: f32,
    pub(super) target_press_scale: f32,
    pub(super) last_frame_id: u32,
    pub(super) idle_age: f32,
    pub(super) color: Color,
    pub(super) release_disc: Option<ReleaseDiscRequest>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ReleaseDiscRequest {
    pub(super) position: Vec2,
    pub(super) intensity: f32,
    pub(super) press_scale: f32,
}

impl TouchPlayerState {
    pub(super) fn new(player_id: String, position: Vec2, frame_id: u32) -> Self {
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

    pub(super) fn apply_sample(
        &mut self,
        sample: TouchSamplePayload,
        frame_pressed: bool,
        frame_id: u32,
    ) {
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

    pub(super) fn release(&mut self) {
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

pub(super) fn setup_touch_assets(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let disc_texture = images.add(create_disc_image());
    let ring_texture = images.add(create_ring_image());
    commands.insert_resource(DiscImage(disc_texture));
    commands.insert_resource(RippleImage(ring_texture));
}

pub(super) fn setup_touch_background(mut commands: Commands, mut clear_color: ResMut<ClearColor>) {
    clear_color.0 = background_color_at(0.0);

    commands.spawn((
        DespawnOnExit(AppUiMode::WanfaTouchRipple),
        Sprite::from_color(background_color_at(0.0), Vec2::ONE),
        Transform::from_xyz(0.0, 0.0, -1.0),
        Background,
    ));
}
pub(super) fn animate_background(
    time: Res<Time>,
    mut clear_color: ResMut<ClearColor>,
    mut background: Single<&mut Sprite, With<Background>>,
) {
    let color = background_color_at(time.elapsed_secs());

    clear_color.0 = color;
    background.color = color;
}

pub(super) fn animate_touch_players(
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

pub(super) fn spawn_drag_ripples(
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

pub(super) fn animate_ripples(
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

pub(super) fn animate_released_discs(
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

pub(super) fn resize_background(
    window: Single<&Window, With<PrimaryWindow>>,
    mut background: Single<&mut Sprite, With<Background>>,
) {
    let size = window.size();
    if size.x <= 0.0 || size.y <= 0.0 {
        return;
    }

    background.custom_size = Some(size);
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

pub(super) fn background_color_at(elapsed_secs: f32) -> Color {
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
