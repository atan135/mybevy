use bevy::prelude::*;

use crate::game::{
    navigation::AppUiMode,
    ui::{
        core::{UiLayer, UiLayerRoot, UiPanelCommand, UiPanelId, UiPanelKind, UiPanelRoot},
        i18n::{UiI18n, UiI18nText},
        style::{
            UiFontAssets, UiTheme,
            theme::{
                UiThemeBackgroundRole, UiThemeBorderRole, UiThemePanelNodeRole,
                UiThemeRootNodeRole, UiThemeTextColorRole, UiThemeTextStyleRole,
            },
        },
        widgets::{
            DisabledButton, LoadingButton, primary_action_button,
            primary_action_button_with_i18n_text, screen_label, screen_title,
            secondary_action_button, secondary_action_button_with_i18n_text,
        },
    },
};

#[derive(Clone, Debug)]
pub(in crate::game) struct UiConfirmModal {
    pub id: UiModalId,
    pub title: String,
    pub body: String,
    pub detail: Option<String>,
    pub title_i18n_text: Option<UiI18nText>,
    pub body_i18n_text: Option<UiI18nText>,
    pub detail_i18n_text: Option<UiI18nText>,
    pub actions: Vec<UiModalActionSpec>,
}

#[derive(Clone, Debug)]
pub(in crate::game) struct UiModalActionSpec {
    pub label: String,
    pub action: UiModalAction,
    pub style: UiModalActionStyle,
    pub i18n_text: Option<UiI18nText>,
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
pub(in crate::game) struct UiModalActionButton {
    id: UiModalId,
    action: UiModalAction,
}

#[derive(Clone, Debug)]
pub(in crate::game) struct UiI18nTextSpec {
    pub text: String,
    pub i18n_text: UiI18nText,
}

impl UiI18nTextSpec {
    pub(in crate::game) fn new(i18n: &UiI18n, key: &'static str, fallback: &'static str) -> Self {
        Self {
            text: i18n.tr(key, fallback),
            i18n_text: UiI18nText::new(key, fallback),
        }
    }
}

pub(in crate::game) fn handle_modal_action_buttons(
    mut modal_results: MessageWriter<UiModalResult>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
    buttons: Query<
        (&Interaction, &UiModalActionButton),
        (
            Changed<Interaction>,
            With<Button>,
            Without<DisabledButton>,
            Without<LoadingButton>,
        ),
    >,
) {
    for (interaction, action_button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }

        modal_results.write(UiModalResult {
            id: action_button.id,
            action: action_button.action,
        });
        panel_commands.write(UiPanelCommand::Close(UiPanelId::ConfirmModal));
    }
}

pub(in crate::game) fn spawn_confirm_modal(
    commands: &mut Commands,
    theme: &UiTheme,
    fonts: &UiFontAssets,
    modal: &UiConfirmModal,
    owner_mode: Option<AppUiMode>,
) {
    commands
        .spawn((
            UiPanelRoot {
                id: UiPanelId::ConfirmModal,
                kind: UiPanelKind::Modal,
                owner_mode,
            },
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
            BackgroundColor(theme.colors.modal_overlay_background),
            UiThemeBackgroundRole::ModalOverlay,
            UiThemeRootNodeRole::BlockingOverlay,
        ))
        .with_children(|root| {
            root.spawn((
                UiThemePanelNodeRole::Standard,
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
                UiThemeBackgroundRole::Panel,
                UiThemeBorderRole::Panel,
            ))
            .with_children(|panel| {
                if let Some(i18n_text) = modal.title_i18n_text.clone() {
                    panel.spawn((
                        screen_title(
                            theme,
                            fonts,
                            modal.title.clone(),
                            UiThemeTextStyleRole::Subtitle,
                        ),
                        i18n_text,
                    ));
                } else {
                    panel.spawn(screen_title(
                        theme,
                        fonts,
                        modal.title.clone(),
                        UiThemeTextStyleRole::Subtitle,
                    ));
                }

                if let Some(i18n_text) = modal.body_i18n_text.clone() {
                    panel.spawn((
                        screen_label(
                            theme,
                            fonts,
                            modal.body.clone(),
                            UiThemeTextStyleRole::Body,
                            UiThemeTextColorRole::Primary,
                        ),
                        i18n_text,
                    ));
                } else {
                    panel.spawn(screen_label(
                        theme,
                        fonts,
                        modal.body.clone(),
                        UiThemeTextStyleRole::Body,
                        UiThemeTextColorRole::Primary,
                    ));
                }

                if let Some(detail) = &modal.detail {
                    if let Some(i18n_text) = modal.detail_i18n_text.clone() {
                        panel.spawn((
                            screen_label(
                                theme,
                                fonts,
                                detail.clone(),
                                UiThemeTextStyleRole::Caption,
                                UiThemeTextColorRole::Muted,
                            ),
                            i18n_text,
                        ));
                    } else {
                        panel.spawn(screen_label(
                            theme,
                            fonts,
                            detail.clone(),
                            UiThemeTextStyleRole::Caption,
                            UiThemeTextColorRole::Muted,
                        ));
                    }
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
                                    if let Some(i18n_text) = action.i18n_text.clone() {
                                        actions.spawn((
                                            primary_action_button_with_i18n_text(
                                                theme,
                                                fonts,
                                                action.label.clone(),
                                                i18n_text,
                                            ),
                                            action_marker,
                                        ));
                                    } else {
                                        actions.spawn((
                                            primary_action_button(
                                                theme,
                                                fonts,
                                                action.label.clone(),
                                            ),
                                            action_marker,
                                        ));
                                    }
                                }
                                UiModalActionStyle::Secondary => {
                                    if let Some(i18n_text) = action.i18n_text.clone() {
                                        actions.spawn((
                                            secondary_action_button_with_i18n_text(
                                                theme,
                                                fonts,
                                                action.label.clone(),
                                                i18n_text,
                                            ),
                                            action_marker,
                                        ));
                                    } else {
                                        actions.spawn((
                                            secondary_action_button(
                                                theme,
                                                fonts,
                                                action.label.clone(),
                                            ),
                                            action_marker,
                                        ));
                                    }
                                }
                            }
                        }
                    });
            });
        });
}
