use bevy::prelude::*;

use crate::game::ui::{
    core::{UiLayer, UiLayerRoot},
    style::UiTheme,
    widgets::{primary_action_button, screen_label, screen_title, secondary_action_button},
};

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
    GalleryConfirm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(in crate::game) enum UiModalAction {
    Cancel,
    Confirm,
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

#[derive(Component)]
pub(in crate::game) struct UiModalRoot;

#[derive(Component)]
pub(in crate::game) struct UiModalActionButton {
    id: UiModalId,
    action: UiModalAction,
}

pub(in crate::game) fn handle_modal_action_buttons(
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

pub(in crate::game) fn spawn_modal(commands: &mut Commands, theme: &UiTheme, modal: &UiModal) {
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

pub(in crate::game) fn close_modals(
    commands: &mut Commands,
    modal_roots: &Query<Entity, With<UiModalRoot>>,
) {
    for entity in modal_roots {
        commands.entity(entity).try_despawn();
    }
}
