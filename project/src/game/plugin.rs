use bevy::{
    asset::RenderAssetUsages,
    input::touch::Touches,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    window::PrimaryWindow,
};

use super::{navigation::AppScreen, screens::ScreensPlugin};

const BACKGROUND_PALETTE: [Vec3; 3] = [
    Vec3::new(0.08, 0.16, 0.30),
    Vec3::new(0.08, 0.31, 0.27),
    Vec3::new(0.28, 0.12, 0.31),
];
const PRESS_PALETTE: [Vec3; 3] = [
    Vec3::new(0.45, 0.95, 0.86),
    Vec3::new(1.00, 0.42, 0.52),
    Vec3::new(1.00, 0.76, 0.25),
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

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ScreensPlugin)
            .insert_resource(ClearColor(background_color_at(0.0)))
            .init_resource::<TouchGradientState>()
            .add_systems(Startup, (setup_camera, setup_touch_assets))
            .add_systems(OnEnter(AppScreen::TouchRipple), setup_scene)
            .add_systems(
                Update,
                (
                    animate_background,
                    update_pointer_target,
                    animate_gradient,
                    spawn_drag_ripples,
                    animate_ripples,
                    animate_released_discs,
                    resize_background,
                )
                    .chain()
                    .run_if(in_state(AppScreen::TouchRipple)),
            );
    }
}

#[derive(Component)]
struct Background;

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
}

#[derive(Component)]
struct ReleasedDisc {
    age: f32,
    duration: f32,
    start_alpha: f32,
    diameter: f32,
}

#[derive(Resource)]
struct DiscImage(Handle<Image>);

#[derive(Resource)]
struct RippleImage(Handle<Image>);

#[derive(Resource, Debug)]
struct TouchGradientState {
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
}

impl Default for TouchGradientState {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            target_position: Vec2::ZERO,
            intensity: 0.0,
            target_intensity: 0.0,
            pulse_age: PULSE_DURATION_SECS,
            was_pressed: false,
            last_ripple_position: None,
            ripple_cooldown: 0.0,
            press_scale: PRESS_START_SCALE,
            target_press_scale: PRESS_START_SCALE,
        }
    }
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

fn setup_scene(
    mut commands: Commands,
    disc_image: Res<DiscImage>,
    ripple_image: Res<RippleImage>,
    mut state: ResMut<TouchGradientState>,
) {
    *state = TouchGradientState::default();

    commands.spawn((
        DespawnOnExit(AppScreen::TouchRipple),
        Sprite::from_color(background_color_at(0.0), Vec2::ONE),
        Transform::from_xyz(0.0, 0.0, -1.0),
        Background,
    ));

    commands.spawn((
        DespawnOnExit(AppScreen::TouchRipple),
        Sprite {
            image: disc_image.0.clone(),
            color: Color::WHITE.with_alpha(0.0),
            custom_size: Some(Vec2::ONE),
            ..Default::default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
        GradientSprite,
    ));

    commands.spawn((
        DespawnOnExit(AppScreen::TouchRipple),
        Sprite {
            image: ripple_image.0.clone(),
            color: Color::WHITE.with_alpha(0.0),
            custom_size: Some(Vec2::ONE),
            ..Default::default()
        },
        Transform::from_xyz(0.0, 0.0, 0.1),
        PulseSprite,
    ));
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

fn update_pointer_target(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    time: Res<Time>,
    disc_image: Res<DiscImage>,
    mut commands: Commands,
    mut state: ResMut<TouchGradientState>,
    ui_buttons: Query<&Interaction, With<Button>>,
) {
    let Some(screen_position) = active_screen_position(&mouse_buttons, &touches, &window) else {
        release_pointer_state(&mut state);
        return;
    };

    if ui_buttons
        .iter()
        .any(|interaction| matches!(*interaction, Interaction::Pressed | Interaction::Hovered))
    {
        release_pointer_state(&mut state);
        return;
    }

    let (camera, camera_transform) = *camera;
    let Ok(world_position) = camera.viewport_to_world_2d(camera_transform, screen_position) else {
        release_pointer_state(&mut state);
        return;
    };

    state.target_position = world_position;
    state.target_intensity = 1.0;
    state.target_press_scale = 1.0;

    if !state.was_pressed {
        if state.intensity > ALPHA_EPSILON {
            spawn_released_disc(
                &mut commands,
                &disc_image,
                state.position,
                state.intensity,
                press_diameter(window.size()) * state.press_scale,
                press_color_at(time.elapsed_secs()),
            );
        }

        state.position = world_position;
        state.target_position = world_position;
        state.intensity = 0.0;
        state.target_intensity = 1.0;
        state.pulse_age = 0.0;
        state.last_ripple_position = Some(world_position);
        state.ripple_cooldown = 0.0;
        state.press_scale = PRESS_START_SCALE;
    }
    state.was_pressed = true;

    if state.intensity <= ALPHA_EPSILON {
        state.position = world_position;
    }
}

fn release_pointer_state(state: &mut TouchGradientState) {
    state.target_intensity = 0.0;
    state.was_pressed = false;
    state.last_ripple_position = None;
    state.target_press_scale = PRESS_RELEASE_SCALE;
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
        DespawnOnExit(AppScreen::TouchRipple),
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
        },
    ));
}

fn animate_gradient(
    time: Res<Time>,
    window: Single<&Window, With<PrimaryWindow>>,
    state: ResMut<TouchGradientState>,
    mut gradient: Single<
        (&mut Transform, &mut Sprite),
        (With<GradientSprite>, Without<PulseSprite>),
    >,
    mut pulse: Single<(&mut Transform, &mut Sprite), (With<PulseSprite>, Without<GradientSprite>)>,
) {
    let state = state.into_inner();
    let delta = time.delta_secs();
    let press_diameter = press_diameter(window.size());

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

    let (transform, sprite) = &mut *gradient;
    transform.translation.x = state.position.x;
    transform.translation.y = state.position.y;
    sprite.custom_size = Some(Vec2::splat(press_diameter * state.press_scale));
    sprite.color =
        press_color_at(time.elapsed_secs()).with_alpha(state.intensity * PRESS_DISC_ALPHA);

    state.pulse_age = (state.pulse_age + delta).min(PULSE_DURATION_SECS);

    let pulse_progress = (state.pulse_age / PULSE_DURATION_SECS).clamp(0.0, 1.0);
    let pulse_alpha = (1.0 - smoothstep(pulse_progress)) * state.intensity * PULSE_ALPHA;
    let pulse_scale = 1.0 + PULSE_EXTRA_SCALE * smoothstep(pulse_progress);
    let pulse_color = press_color_at(time.elapsed_secs());

    let (pulse_transform, pulse_sprite) = &mut *pulse;
    pulse_transform.translation.x = state.position.x;
    pulse_transform.translation.y = state.position.y;
    pulse_sprite.custom_size = Some(Vec2::splat(press_diameter * pulse_scale));
    pulse_sprite.color = pulse_color.with_alpha(pulse_alpha);
}

fn spawn_drag_ripples(
    time: Res<Time>,
    mut commands: Commands,
    ripple_image: Res<RippleImage>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut state: ResMut<TouchGradientState>,
) {
    if state.target_intensity <= 0.0 {
        state.last_ripple_position = None;
        state.ripple_cooldown = 0.0;
        return;
    }

    let press_diameter = press_diameter(window.size());
    let spacing = press_diameter * RIPPLE_SPACING_RATIO;
    let current_position = state.target_position;

    state.ripple_cooldown = (state.ripple_cooldown - time.delta_secs()).max(0.0);

    let Some(last_position) = state.last_ripple_position else {
        state.last_ripple_position = Some(current_position);
        return;
    };

    if last_position.distance(current_position) < spacing || state.ripple_cooldown > 0.0 {
        return;
    }

    commands.spawn((
        DespawnOnExit(AppScreen::TouchRipple),
        Sprite {
            image: ripple_image.0.clone(),
            color: press_color_at(time.elapsed_secs()).with_alpha(RIPPLE_ALPHA),
            custom_size: Some(Vec2::splat(press_diameter * RIPPLE_START_SCALE)),
            ..Default::default()
        },
        Transform::from_xyz(current_position.x, current_position.y, 0.05),
        Ripple {
            age: 0.0,
            duration: RIPPLE_DURATION_SECS,
            start_diameter: press_diameter * RIPPLE_START_SCALE,
            end_diameter: press_diameter * RIPPLE_END_SCALE,
        },
    ));

    state.last_ripple_position = Some(current_position);
    state.ripple_cooldown = RIPPLE_COOLDOWN_SECS;
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
        sprite.color = press_color_at(time.elapsed_secs()).with_alpha(alpha);

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
        sprite.color = press_color_at(time.elapsed_secs()).with_alpha(alpha);

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

fn active_screen_position(
    mouse_buttons: &ButtonInput<MouseButton>,
    touches: &Touches,
    window: &Window,
) -> Option<Vec2> {
    touches.first_pressed_position().or_else(|| {
        mouse_buttons
            .pressed(MouseButton::Left)
            .then(|| window.cursor_position())
            .flatten()
    })
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

fn press_color_at(elapsed_secs: f32) -> Color {
    let rgb = palette_color_at(&PRESS_PALETTE, elapsed_secs);

    Color::srgb(rgb.x, rgb.y, rgb.z)
}

fn palette_color_at(palette: &[Vec3], elapsed_secs: f32) -> Vec3 {
    let position = (elapsed_secs / BACKGROUND_CYCLE_SECONDS).rem_euclid(1.0) * palette.len() as f32;
    let from_index = position.floor() as usize;
    let to_index = (from_index + 1) % palette.len();
    let blend = smoothstep(position.fract());

    palette[from_index].lerp(palette[to_index], blend)
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
