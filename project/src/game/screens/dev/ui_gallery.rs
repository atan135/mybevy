use bevy::prelude::*;

use crate::game::{
    navigation::AppUiMode,
    ui::{
        core::{
            UiFloatingPanel, UiLayer, UiLayerRoot, UiPanelCommand, UiPanelId, UiPanelKind,
            UiPanelRequest, UiPanelRoot,
        },
        overlays::{
            UiConfirmModal, UiLoading, UiModalAction, UiModalActionSpec, UiModalActionStyle,
            UiModalId, UiRouteCommand, UiToast,
        },
        style::UiTheme,
        widgets::{
            DisabledButton, disabled_primary_action_button, disabled_secondary_action_button,
            primary_action_button, screen_label, screen_title, secondary_action_button,
            secondary_route_button,
        },
    },
};

#[derive(Clone, Copy, Component)]
pub(super) enum GalleryActionButton {
    Toast,
    ShowLoading,
    HideLoading,
    Confirm,
    Floating,
    CloseTop,
}

#[derive(Resource)]
pub(super) struct GalleryLoadingPreview {
    timer: Timer,
}

impl GalleryLoadingPreview {
    fn new() -> Self {
        Self {
            timer: Timer::from_seconds(1.2, TimerMode::Once),
        }
    }
}

pub(super) fn setup_ui_gallery(
    mut commands: Commands,
    theme: Res<UiTheme>,
    mut clear_color: ResMut<ClearColor>,
) {
    let theme = theme.into_inner();
    clear_color.0 = theme.colors.screen_background;

    commands
        .spawn((
            DespawnOnExit(AppUiMode::UiGallery),
            UiPanelRoot {
                id: UiPanelId::UiGalleryPage,
                kind: UiPanelKind::Page,
                owner_mode: Some(AppUiMode::UiGallery),
            },
            UiLayerRoot {
                layer: UiLayer::Page,
            },
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(theme.layout.screen_padding)),
                row_gap: px(theme.layout.page_gap),
                ..default()
            },
            BackgroundColor(theme.colors.screen_background),
        ))
        .with_children(|root| {
            root.spawn(gallery_header(theme)).with_children(|header| {
                header.spawn(screen_title(theme, "UI Gallery", theme.text.title));
                header.spawn(secondary_route_button(theme, "Lobby", AppUiMode::Lobby));
            });

            root.spawn(gallery_panel(theme))
                .with_children(|typography_panel| {
                    typography_panel.spawn(section_label(theme, "Typography"));
                    typography_panel
                        .spawn(gallery_column(theme))
                        .with_children(|samples| {
                            samples.spawn(screen_title(
                                theme,
                                "Large Title",
                                theme.text.title_large,
                            ));
                            samples.spawn(screen_title(theme, "Section Title", theme.text.title));
                            samples.spawn(screen_label(
                                "Subtitle text",
                                theme.text.subtitle,
                                theme.colors.text_muted,
                            ));
                            samples.spawn(screen_label(
                                "Body text",
                                theme.text.body,
                                theme.colors.text_primary,
                            ));
                            samples.spawn(screen_label(
                                "Caption text",
                                theme.text.caption,
                                theme.colors.text_muted,
                            ));
                        });
                });

            root.spawn(gallery_panel(theme))
                .with_children(|buttons_panel| {
                    buttons_panel.spawn(section_label(theme, "Buttons"));
                    buttons_panel
                        .spawn(gallery_button_row(theme))
                        .with_children(|buttons| {
                            buttons.spawn(primary_action_button(theme, "Primary"));
                            buttons.spawn(secondary_action_button(theme, "Secondary"));
                            buttons.spawn(disabled_primary_action_button(theme, "Disabled"));
                            buttons.spawn(disabled_secondary_action_button(theme, "Unavailable"));
                            buttons.spawn(primary_route_button_sample(theme));
                        });
                });

            root.spawn(gallery_panel(theme))
                .with_children(|overlays_panel| {
                    overlays_panel.spawn(section_label(theme, "Overlays"));
                    overlays_panel
                        .spawn(gallery_button_row(theme))
                        .with_children(|buttons| {
                            buttons.spawn((
                                primary_action_button(theme, "Show Toast"),
                                GalleryActionButton::Toast,
                            ));
                            buttons.spawn((
                                secondary_action_button(theme, "Show Loading"),
                                GalleryActionButton::ShowLoading,
                            ));
                            buttons.spawn((
                                secondary_action_button(theme, "Hide Loading"),
                                GalleryActionButton::HideLoading,
                            ));
                            buttons.spawn((
                                primary_action_button(theme, "Show Confirm"),
                                GalleryActionButton::Confirm,
                            ));
                            buttons.spawn((
                                secondary_action_button(theme, "Show Floating"),
                                GalleryActionButton::Floating,
                            ));
                            buttons.spawn((
                                secondary_action_button(theme, "Close Top"),
                                GalleryActionButton::CloseTop,
                            ));
                        });
                });
        });
}

pub(super) fn handle_ui_gallery_buttons(
    mut commands: Commands,
    mut panel_commands: MessageWriter<UiPanelCommand>,
    mut route_commands: MessageWriter<UiRouteCommand>,
    buttons: Query<
        (&Interaction, &GalleryActionButton),
        (Changed<Interaction>, With<Button>, Without<DisabledButton>),
    >,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match action {
            GalleryActionButton::Toast => {
                route_commands.write(UiRouteCommand::ShowToast(UiToast::new(
                    "Toast from UI Gallery",
                )));
            }
            GalleryActionButton::ShowLoading => {
                commands.insert_resource(GalleryLoadingPreview::new());
                panel_commands.write(UiPanelCommand::Open(UiPanelRequest::Loading(
                    UiLoading::new("Loading preview"),
                )));
            }
            GalleryActionButton::HideLoading => {
                commands.remove_resource::<GalleryLoadingPreview>();
                panel_commands.write(UiPanelCommand::Close(UiPanelId::GlobalLoading));
            }
            GalleryActionButton::Confirm => {
                panel_commands.write(UiPanelCommand::Open(UiPanelRequest::Confirm(
                    gallery_confirm_modal(),
                )));
            }
            GalleryActionButton::Floating => {
                panel_commands.write(UiPanelCommand::Open(UiPanelRequest::Floating(
                    gallery_floating_panel(),
                )));
            }
            GalleryActionButton::CloseTop => {
                panel_commands.write(UiPanelCommand::CloseTop);
            }
        }
    }
}

pub(super) fn tick_ui_gallery_loading_preview(
    mut commands: Commands,
    time: Res<Time>,
    preview: Option<ResMut<GalleryLoadingPreview>>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
) {
    let Some(mut preview) = preview else {
        return;
    };

    preview.timer.tick(time.delta());
    if preview.timer.is_finished() {
        commands.remove_resource::<GalleryLoadingPreview>();
        panel_commands.write(UiPanelCommand::Close(UiPanelId::GlobalLoading));
    }
}

pub(super) fn clear_ui_gallery_loading_preview(mut commands: Commands) {
    commands.remove_resource::<GalleryLoadingPreview>();
}

fn gallery_header(theme: &UiTheme) -> impl Bundle {
    Node {
        width: percent(100),
        max_width: px(theme.layout.content_width),
        align_self: AlignSelf::Center,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::SpaceBetween,
        column_gap: px(theme.layout.header_gap),
        ..default()
    }
}

fn gallery_panel(theme: &UiTheme) -> impl Bundle {
    (
        Node {
            width: percent(100),
            max_width: px(theme.layout.content_width),
            align_self: AlignSelf::Center,
            flex_direction: FlexDirection::Column,
            row_gap: px(theme.layout.card_gap),
            padding: UiRect::all(px(theme.layout.panel_gap)),
            border: UiRect::all(px(theme.panel.border)),
            border_radius: BorderRadius::all(px(theme.panel.radius)),
            ..default()
        },
        BackgroundColor(theme.colors.panel_background),
        BorderColor::all(theme.colors.panel_border),
    )
}

fn gallery_column(theme: &UiTheme) -> impl Bundle {
    Node {
        width: percent(100),
        flex_direction: FlexDirection::Column,
        row_gap: px(theme.layout.row_gap),
        ..default()
    }
}

fn gallery_button_row(theme: &UiTheme) -> impl Bundle {
    Node {
        width: percent(100),
        align_items: AlignItems::Center,
        column_gap: px(theme.layout.row_column_gap),
        row_gap: px(theme.layout.row_gap),
        flex_wrap: FlexWrap::Wrap,
        ..default()
    }
}

fn section_label(theme: &UiTheme, text: impl Into<String>) -> impl Bundle {
    screen_label(text, theme.text.section_label, theme.colors.text_muted)
}

fn primary_route_button_sample(theme: &UiTheme) -> impl Bundle {
    (
        primary_action_button(theme, "Action"),
        Name::new("Gallery action sample"),
    )
}

fn gallery_confirm_modal() -> UiConfirmModal {
    UiConfirmModal {
        id: UiModalId::GalleryConfirm,
        title: "Gallery Confirm".to_string(),
        body: "This confirms modal layering and input blocking.".to_string(),
        detail: Some("The page buttons below should not react while this is open.".to_string()),
        actions: vec![
            UiModalActionSpec {
                label: "Cancel".to_string(),
                action: UiModalAction::Cancel,
                style: UiModalActionStyle::Secondary,
            },
            UiModalActionSpec {
                label: "Confirm".to_string(),
                action: UiModalAction::Confirm,
                style: UiModalActionStyle::Primary,
            },
        ],
    }
}

fn gallery_floating_panel() -> UiFloatingPanel {
    UiFloatingPanel {
        id: UiPanelId::GalleryFloating,
        title: "Floating Panel".to_string(),
        body: "This panel does not cover the whole page.".to_string(),
        detail: Some("Use Close Top or Esc to close it.".to_string()),
    }
}
