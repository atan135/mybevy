use bevy::prelude::*;

use crate::game::{
    navigation::{AppUiMode, RouteButton},
    ui::{
        layer::{UiLayer, UiLayerRoot},
        theme::UiTheme,
        widgets::{primary_action_button, screen_label, screen_title, secondary_action_button},
    },
};

const DEFAULT_TOAST_DURATION_SECS: f32 = 2.4;

pub(in crate::game) struct UiRouterPlugin;

impl Plugin for UiRouterPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<UiRouteCommand>()
            .add_message::<UiModalResult>()
            .add_systems(
                Update,
                (
                    handle_route_buttons,
                    handle_ui_route_commands,
                    handle_modal_action_buttons,
                    tick_toasts,
                )
                    .chain(),
            );
    }
}

#[derive(Clone, Debug, Message)]
#[allow(dead_code)]
pub(in crate::game) enum UiRouteCommand {
    ChangeMode(AppUiMode),
    OpenModal(UiModal),
    CloseModal,
    ShowToast(UiToast),
}

#[derive(Clone, Debug)]
pub(in crate::game) enum UiModal {
    Confirm(UiConfirmModal),
}

#[derive(Clone, Debug)]
pub(in crate::game) struct UiConfirmModal {
    pub id: UiModalId,
    pub title: String,
    pub body: String,
    pub detail: Option<String>,
    pub actions: Vec<UiModalActionSpec>,
}

#[derive(Clone, Debug)]
pub(in crate::game) struct UiModalActionSpec {
    pub label: String,
    pub action: UiModalAction,
    pub style: UiModalActionStyle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(in crate::game) enum UiModalId {
    TouchRippleLaunch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(in crate::game) enum UiModalAction {
    Cancel,
    TouchRippleSinglePlayer,
    TouchRippleNetworked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(in crate::game) enum UiModalActionStyle {
    Primary,
    Secondary,
}

#[derive(Clone, Copy, Debug, Message)]
pub(in crate::game) struct UiModalResult {
    pub id: UiModalId,
    pub action: UiModalAction,
}

#[derive(Clone, Debug)]
pub(in crate::game) struct UiToast {
    pub text: String,
    pub duration_secs: f32,
}

impl UiToast {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            duration_secs: DEFAULT_TOAST_DURATION_SECS,
        }
    }
}

#[derive(Component)]
pub(in crate::game) struct UiModalRoot;

#[derive(Component)]
struct UiToastRoot {
    timer: Timer,
}

#[derive(Component)]
struct UiModalActionButton {
    id: UiModalId,
    action: UiModalAction,
}

fn handle_route_buttons(
    mut route_commands: MessageWriter<UiRouteCommand>,
    buttons: Query<(&Interaction, &RouteButton), (Changed<Interaction>, With<Button>)>,
) {
    for (interaction, route_button) in &buttons {
        if *interaction == Interaction::Pressed {
            route_commands.write(UiRouteCommand::ChangeMode(route_button.target));
        }
    }
}

fn handle_ui_route_commands(
    mut commands: Commands,
    theme: Res<UiTheme>,
    mut route_commands: MessageReader<UiRouteCommand>,
    mut next_mode: ResMut<NextState<AppUiMode>>,
    modal_roots: Query<Entity, With<UiModalRoot>>,
    toast_roots: Query<Entity, With<UiToastRoot>>,
) {
    for command in route_commands.read() {
        match command {
            UiRouteCommand::ChangeMode(mode) => {
                close_modals(&mut commands, &modal_roots);
                next_mode.set(*mode);
            }
            UiRouteCommand::OpenModal(modal) => {
                close_modals(&mut commands, &modal_roots);
                spawn_modal(&mut commands, &theme, modal);
            }
            UiRouteCommand::CloseModal => {
                close_modals(&mut commands, &modal_roots);
            }
            UiRouteCommand::ShowToast(toast) => {
                close_toasts(&mut commands, &toast_roots);
                spawn_toast(&mut commands, &theme, toast);
            }
        }
    }
}

fn handle_modal_action_buttons(
    mut commands: Commands,
    mut modal_results: MessageWriter<UiModalResult>,
    buttons: Query<(&Interaction, &UiModalActionButton), (Changed<Interaction>, With<Button>)>,
    modal_roots: Query<Entity, With<UiModalRoot>>,
) {
    for (interaction, action_button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }

        modal_results.write(UiModalResult {
            id: action_button.id,
            action: action_button.action,
        });
        close_modals(&mut commands, &modal_roots);
    }
}

fn tick_toasts(
    mut commands: Commands,
    time: Res<Time>,
    mut toasts: Query<(Entity, &mut UiToastRoot)>,
) {
    for (entity, mut toast) in &mut toasts {
        toast.timer.tick(time.delta());
        if toast.timer.is_finished() {
            commands.entity(entity).try_despawn();
        }
    }
}

fn spawn_modal(commands: &mut Commands, theme: &UiTheme, modal: &UiModal) {
    match modal {
        UiModal::Confirm(confirm) => spawn_confirm_modal(commands, theme, confirm),
    }
}

fn spawn_confirm_modal(commands: &mut Commands, theme: &UiTheme, modal: &UiConfirmModal) {
    commands
        .spawn((
            UiModalRoot,
            UiLayerRoot {
                layer: UiLayer::Modal,
            },
            Button,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(px(theme.layout.screen_padding)),
                ..default()
            },
            ZIndex(100),
            BackgroundColor(Color::srgba(0.01, 0.02, 0.03, 0.72)),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(100),
                    max_width: px(460),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(theme.layout.card_gap),
                    padding: UiRect::all(px(theme.panel.padding)),
                    border: UiRect::all(px(theme.panel.border)),
                    border_radius: BorderRadius::all(px(theme.panel.radius)),
                    ..default()
                },
                BackgroundColor(theme.colors.panel_background),
                BorderColor::all(theme.colors.panel_border),
            ))
            .with_children(|panel| {
                panel.spawn(screen_title(
                    theme,
                    modal.title.clone(),
                    theme.text.subtitle,
                ));
                panel.spawn(screen_label(
                    modal.body.clone(),
                    theme.text.body,
                    theme.colors.text_primary,
                ));

                if let Some(detail) = &modal.detail {
                    panel.spawn(screen_label(
                        detail.clone(),
                        theme.text.caption,
                        theme.colors.text_muted,
                    ));
                }

                panel
                    .spawn(Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::FlexEnd,
                        column_gap: px(theme.layout.row_column_gap),
                        margin: UiRect::top(px(theme.layout.row_gap)),
                        ..default()
                    })
                    .with_children(|actions| {
                        for action in &modal.actions {
                            let action_marker = UiModalActionButton {
                                id: modal.id,
                                action: action.action,
                            };
                            match action.style {
                                UiModalActionStyle::Primary => {
                                    actions.spawn((
                                        primary_action_button(theme, action.label.clone()),
                                        action_marker,
                                    ));
                                }
                                UiModalActionStyle::Secondary => {
                                    actions.spawn((
                                        secondary_action_button(theme, action.label.clone()),
                                        action_marker,
                                    ));
                                }
                            }
                        }
                    });
            });
        });
}

fn spawn_toast(commands: &mut Commands, theme: &UiTheme, toast: &UiToast) {
    commands.spawn((
        UiToastRoot {
            timer: Timer::from_seconds(toast.duration_secs.max(0.1), TimerMode::Once),
        },
        UiLayerRoot {
            layer: UiLayer::Toast,
        },
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(theme.layout.overlay_padding),
            justify_content: JustifyContent::Center,
            padding: UiRect::horizontal(px(theme.layout.overlay_padding)),
            ..default()
        },
        ZIndex(200),
        children![(
            Node {
                max_width: px(420),
                padding: UiRect::axes(px(18), px(12)),
                border: UiRect::all(px(theme.panel.border)),
                border_radius: BorderRadius::all(px(theme.button.radius)),
                ..default()
            },
            BackgroundColor(theme.colors.panel_background),
            BorderColor::all(theme.colors.panel_border),
            children![screen_label(
                toast.text.clone(),
                theme.text.caption,
                theme.colors.text_primary,
            )],
        )],
    ));
}

fn close_modals(commands: &mut Commands, modal_roots: &Query<Entity, With<UiModalRoot>>) {
    for entity in modal_roots {
        commands.entity(entity).try_despawn();
    }
}

fn close_toasts(commands: &mut Commands, toast_roots: &Query<Entity, With<UiToastRoot>>) {
    for entity in toast_roots {
        commands.entity(entity).try_despawn();
    }
}
