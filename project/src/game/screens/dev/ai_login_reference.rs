use bevy::{ecs::hierarchy::ChildSpawnerCommands, picking::Pickable, prelude::*, ui::UiTransform};

use crate::{
    framework::ui::{
        core::{
            UiAnimationCommand, UiAnimationEasing, UiAnimationId, UiAnimationSpec, UiLayer,
            UiLayerRoot, UiPanelKind,
        },
        style::UiFontAssets,
        widgets::{FocusableButton, UiButtonEvent, UiButtonEventKind},
    },
    game::{
        navigation::{AppUiMode, game_panel_root},
        ui_ids::{OWNER_AI_LOGIN_REFERENCE, PANEL_AI_LOGIN_REFERENCE},
    },
};

const BACKGROUND_PATH: &str = "ui/images/ai_login_background.png";
const SIGIL_PATH: &str = "ui/images/ai_login_sigil.png";
const PANEL_SURFACE_PATH: &str = "ui/images/ai_login_panel_surface.png";
const PRESS_SCALE: f32 = 0.96;
const PRESS_DURATION_SECS: f32 = 0.07;
const RELEASE_DURATION_SECS: f32 = 0.13;
const PRESS_ANIMATION_ID: UiAnimationId = UiAnimationId::new("ai_login_reference.button_press");
const RELEASE_ANIMATION_ID: UiAnimationId = UiAnimationId::new("ai_login_reference.button_release");

#[derive(Component)]
pub(super) struct AiLoginReferenceButton;

pub(super) fn setup_ai_login_reference(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    fonts: Res<UiFontAssets>,
    mut clear_color: ResMut<ClearColor>,
) {
    clear_color.0 = Color::srgb_u8(6, 18, 22);
    let background = asset_server.load(BACKGROUND_PATH);
    let sigil = asset_server.load(SIGIL_PATH);
    let panel_surface = asset_server.load(PANEL_SURFACE_PATH);

    commands
        .spawn((
            DespawnOnExit(AppUiMode::AiLoginReference),
            game_panel_root(
                PANEL_AI_LOGIN_REFERENCE,
                UiPanelKind::Page,
                OWNER_AI_LOGIN_REFERENCE,
            ),
            UiLayerRoot {
                layer: UiLayer::Page,
            },
            Name::new("AI login reference root"),
            Node {
                width: percent(100),
                height: percent(100),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(6, 18, 22)),
        ))
        .with_children(|root| {
            root.spawn((
                Name::new("AI login reference background"),
                Node {
                    width: percent(100),
                    height: percent(100),
                    position_type: PositionType::Absolute,
                    ..default()
                },
                ImageNode::new(background).with_mode(NodeImageMode::Stretch),
                Pickable::IGNORE,
                ZIndex(0),
            ));
            root.spawn((
                Name::new("AI login reference sigil"),
                Node {
                    width: px(82),
                    height: px(105),
                    position_type: PositionType::Absolute,
                    left: percent(46.8),
                    top: percent(45.4),
                    ..default()
                },
                ImageNode::new(sigil).with_mode(NodeImageMode::Stretch),
                Pickable::IGNORE,
                ZIndex(2),
            ));
            root.spawn((
                Name::new("AI login reference menu"),
                Node {
                    width: percent(24.5),
                    height: percent(87.5),
                    position_type: PositionType::Absolute,
                    right: percent(0.0),
                    top: percent(6.25),
                    padding: UiRect::new(px(40), px(40), px(104), px(104)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                ZIndex(10),
            ))
            .with_children(|menu| {
                menu.spawn((
                    Name::new("AI login reference panel surface"),
                    Node {
                        width: percent(100),
                        height: percent(100),
                        position_type: PositionType::Absolute,
                        ..default()
                    },
                    ImageNode::new(panel_surface).with_mode(NodeImageMode::Stretch),
                    Pickable::IGNORE,
                    ZIndex(0),
                ));
                menu.spawn((
                    Name::new("AI login reference menu buttons"),
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(18),
                        align_items: AlignItems::Stretch,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    ZIndex(1),
                ))
                .with_children(|buttons| {
                    for label in ["选择服务器", "开始游戏", "账号管理"] {
                        spawn_ai_login_reference_button(buttons, &fonts, label);
                    }
                });
            });
        });
}

pub(super) fn animate_ai_login_reference_buttons(
    mut button_events: MessageReader<UiButtonEvent>,
    buttons: Query<(), With<AiLoginReferenceButton>>,
    mut animation_commands: MessageWriter<UiAnimationCommand>,
) {
    for event in button_events.read() {
        if !buttons.contains(event.entity) {
            continue;
        }
        match event.kind {
            UiButtonEventKind::Down => {
                animation_commands
                    .write(UiAnimationCommand::start(event.entity, press_animation()));
            }
            UiButtonEventKind::Up | UiButtonEventKind::Cancel => {
                animation_commands.write(UiAnimationCommand::continue_from_current(
                    event.entity,
                    release_animation(),
                ));
            }
            UiButtonEventKind::Click => {}
        }
    }
}

pub(super) fn sync_ai_login_reference_button_visuals(
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (With<AiLoginReferenceButton>, Changed<Interaction>),
    >,
) {
    for (interaction, mut background, mut border) in &mut buttons {
        let (surface, line) = match interaction {
            Interaction::Pressed => (
                Color::srgba(0.88, 0.79, 0.48, 0.24),
                Color::srgba(1.0, 0.91, 0.62, 0.96),
            ),
            Interaction::Hovered => (
                Color::srgba(0.67, 0.83, 0.84, 0.12),
                Color::srgba(0.89, 0.85, 0.63, 0.86),
            ),
            Interaction::None => (Color::NONE, Color::srgba(0.73, 0.69, 0.47, 0.58)),
        };
        *background = BackgroundColor(surface);
        *border = horizontal_border(line);
    }
}

fn spawn_ai_login_reference_button(
    buttons: &mut ChildSpawnerCommands,
    fonts: &UiFontAssets,
    label: &'static str,
) {
    let line = Color::srgba(0.73, 0.69, 0.47, 0.58);
    buttons.spawn((
        Name::new(format!("AI login reference button {label}")),
        Button,
        FocusableButton,
        AiLoginReferenceButton,
        UiTransform::default(),
        Node {
            width: percent(100),
            height: px(68),
            min_height: px(60),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: UiRect::vertical(px(1)),
            ..default()
        },
        BackgroundColor(Color::NONE),
        horizontal_border(line),
        children![(
            Text::new(label),
            TextFont {
                font: fonts.regular.clone(),
                font_size: 28.0,
                weight: FontWeight::BOLD,
                ..default()
            },
            TextColor(Color::srgb(0.94, 0.89, 0.69)),
        )],
    ));
}

fn horizontal_border(color: Color) -> BorderColor {
    BorderColor {
        top: color,
        right: Color::NONE,
        bottom: color,
        left: Color::NONE,
    }
}

fn press_animation() -> UiAnimationSpec {
    UiAnimationSpec::transform_scale(
        PRESS_ANIMATION_ID,
        Vec2::ONE,
        Vec2::splat(PRESS_SCALE),
        PRESS_DURATION_SECS,
    )
    .with_easing(UiAnimationEasing::EaseOutCubic)
}

fn release_animation() -> UiAnimationSpec {
    UiAnimationSpec::transform_scale(
        RELEASE_ANIMATION_ID,
        Vec2::splat(PRESS_SCALE),
        Vec2::ONE,
        RELEASE_DURATION_SECS,
    )
    .with_easing(UiAnimationEasing::EaseOutCubic)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_button_press_and_release_use_transform_scale() {
        let press = press_animation();
        let release = release_animation();
        assert_eq!(
            press.from,
            crate::framework::ui::core::UiAnimationValue::Vector(Vec2::ONE)
        );
        assert_eq!(
            press.to,
            crate::framework::ui::core::UiAnimationValue::Vector(Vec2::splat(PRESS_SCALE))
        );
        assert_eq!(
            release.to,
            crate::framework::ui::core::UiAnimationValue::Vector(Vec2::ONE)
        );
        assert_eq!(press.duration_secs, PRESS_DURATION_SECS);
        assert_eq!(release.duration_secs, RELEASE_DURATION_SECS);
    }
}
