use bevy::prelude::*;

use crate::game::{
    navigation::AppUiMode,
    ui::{
        core::{
            UiFloatingPanel, UiLayer, UiLayerRoot, UiPanelCommand, UiPanelId, UiPanelKind,
            UiPanelRequest, UiPanelRoot,
        },
        i18n::{UiI18n, UiI18nText},
        overlays::{
            UiConfirmModal, UiI18nTextSpec, UiLoading, UiModalAction, UiModalActionSpec,
            UiModalActionStyle, UiModalId, UiRouteCommand, UiToast,
        },
        style::{
            UiFontAssets, UiTheme,
            theme::{UiThemeBackgroundRole, UiThemeBorderRole, UiThemeTextColorRole},
        },
        widgets::{
            DisabledButton, FocusedButton, LoadingButton, SelectedButton, UiTextInputSubmitted,
            disabled_primary_action_button_key, disabled_secondary_action_button_key,
            loading_primary_action_button_key, primary_action_button_key, screen_label_key,
            screen_title_key, secondary_action_button_key, secondary_route_button_key, text_input,
            ui_column, ui_grid, ui_scroll_column,
        },
    },
};

#[derive(Clone, Copy, Component)]
pub(super) enum GalleryActionButton {
    Toast,
    ShowLoading,
    ShowCancellableLoading,
    HideLoading,
    Confirm,
    Floating,
    CloseTop,
}

#[derive(Resource)]
pub(super) struct GalleryLoadingPreview {
    timer: Timer,
}

#[derive(Resource)]
pub(super) struct GalleryFloatingI18n {
    panel_id: UiPanelId,
    title: UiI18nTextSpec,
    body: UiI18nTextSpec,
    detail: Option<UiI18nTextSpec>,
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
    fonts: Res<UiFontAssets>,
    i18n: Res<UiI18n>,
    mut clear_color: ResMut<ClearColor>,
) {
    let theme = theme.into_inner();
    let fonts = fonts.into_inner();
    let i18n = i18n.into_inner();
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
            UiThemeBackgroundRole::Screen,
        ))
        .with_children(|root| {
            root.spawn(gallery_header(theme)).with_children(|header| {
                header.spawn(screen_title_key(
                    theme,
                    fonts,
                    i18n,
                    "ui_gallery.title",
                    "UI Gallery",
                    theme.text.title,
                ));
                header.spawn(secondary_route_button_key(
                    theme,
                    fonts,
                    i18n,
                    "nav.lobby",
                    "Lobby",
                    AppUiMode::Lobby,
                ));
            });

            root.spawn(ui_scroll_column(theme)).with_children(|body| {
                body.spawn(gallery_panel(theme))
                    .with_children(|typography_panel| {
                        typography_panel.spawn(section_label_key(
                            theme,
                            fonts,
                            i18n,
                            "ui_gallery.typography.section",
                            "Typography",
                        ));
                        typography_panel
                            .spawn(ui_column(theme.layout.row_gap))
                            .with_children(|samples| {
                                samples.spawn(screen_title_key(
                                    theme,
                                    fonts,
                                    i18n,
                                    "ui_gallery.typography.large_title",
                                    "Large Title",
                                    theme.text.title_large,
                                ));
                                samples.spawn(screen_title_key(
                                    theme,
                                    fonts,
                                    i18n,
                                    "ui_gallery.typography.section_title",
                                    "Section Title",
                                    theme.text.title,
                                ));
                                samples.spawn(screen_label_key(
                                    theme,
                                    fonts,
                                    i18n,
                                    "ui_gallery.typography.subtitle",
                                    "Subtitle text",
                                    theme.text.subtitle,
                                    UiThemeTextColorRole::Muted,
                                ));
                                samples.spawn(screen_label_key(
                                    theme,
                                    fonts,
                                    i18n,
                                    "ui_gallery.typography.body",
                                    "Body text",
                                    theme.text.body,
                                    UiThemeTextColorRole::Primary,
                                ));
                                samples.spawn(screen_label_key(
                                    theme,
                                    fonts,
                                    i18n,
                                    "ui_gallery.typography.caption",
                                    "Caption text",
                                    theme.text.caption,
                                    UiThemeTextColorRole::Muted,
                                ));
                            });
                    });

                body.spawn(gallery_panel(theme))
                    .with_children(|buttons_panel| {
                        buttons_panel.spawn(section_label_key(
                            theme,
                            fonts,
                            i18n,
                            "ui_gallery.buttons.section",
                            "Buttons",
                        ));
                        buttons_panel
                            .spawn(ui_grid(theme, 4))
                            .with_children(|buttons| {
                                buttons.spawn(primary_action_button_key(
                                    theme,
                                    fonts,
                                    i18n,
                                    "ui_gallery.buttons.primary",
                                    "Primary",
                                ));
                                buttons.spawn(secondary_action_button_key(
                                    theme,
                                    fonts,
                                    i18n,
                                    "ui_gallery.buttons.secondary",
                                    "Secondary",
                                ));
                                buttons.spawn((
                                    primary_action_button_key(
                                        theme,
                                        fonts,
                                        i18n,
                                        "ui_gallery.buttons.focused",
                                        "Focused",
                                    ),
                                    FocusedButton,
                                ));
                                buttons.spawn((
                                    secondary_action_button_key(
                                        theme,
                                        fonts,
                                        i18n,
                                        "ui_gallery.buttons.selected",
                                        "Selected",
                                    ),
                                    SelectedButton,
                                ));
                                buttons.spawn(loading_primary_action_button_key(
                                    theme,
                                    fonts,
                                    i18n,
                                    "ui_gallery.buttons.loading",
                                    "Loading",
                                ));
                                buttons.spawn(disabled_primary_action_button_key(
                                    theme,
                                    fonts,
                                    i18n,
                                    "ui_gallery.buttons.disabled",
                                    "Disabled",
                                ));
                                buttons.spawn(disabled_secondary_action_button_key(
                                    theme,
                                    fonts,
                                    i18n,
                                    "ui_gallery.buttons.unavailable",
                                    "Unavailable",
                                ));
                                buttons.spawn(primary_route_button_sample(theme, fonts, i18n));
                            });
                    });

                body.spawn(gallery_panel(theme))
                    .with_children(|inputs_panel| {
                        inputs_panel.spawn(section_label_key(
                            theme,
                            fonts,
                            i18n,
                            "ui_gallery.inputs.section",
                            "Inputs",
                        ));
                        inputs_panel
                            .spawn(ui_column(theme.layout.row_gap))
                            .with_children(|inputs| {
                                inputs.spawn(text_input(
                                    theme,
                                    fonts,
                                    i18n.tr(
                                        "ui_gallery.inputs.placeholder.player_name",
                                        "Player name",
                                    ),
                                    "Pilot 01",
                                ));
                                inputs.spawn(text_input(
                                    theme,
                                    fonts,
                                    i18n.tr("ui_gallery.inputs.placeholder.note", "Type a note"),
                                    "",
                                ));
                            });
                    });

                body.spawn(gallery_panel(theme))
                    .with_children(|overlays_panel| {
                        overlays_panel.spawn(section_label_key(
                            theme,
                            fonts,
                            i18n,
                            "ui_gallery.overlays.section",
                            "Overlays",
                        ));
                        overlays_panel
                            .spawn(ui_grid(theme, 4))
                            .with_children(|buttons| {
                                buttons.spawn((
                                    primary_action_button_key(
                                        theme,
                                        fonts,
                                        i18n,
                                        "ui_gallery.overlays.show_toast",
                                        "Show Toast",
                                    ),
                                    GalleryActionButton::Toast,
                                ));
                                buttons.spawn((
                                    secondary_action_button_key(
                                        theme,
                                        fonts,
                                        i18n,
                                        "ui_gallery.overlays.loading",
                                        "Loading",
                                    ),
                                    GalleryActionButton::ShowLoading,
                                ));
                                buttons.spawn((
                                    secondary_action_button_key(
                                        theme,
                                        fonts,
                                        i18n,
                                        "ui_gallery.overlays.cancelable",
                                        "Cancelable",
                                    ),
                                    GalleryActionButton::ShowCancellableLoading,
                                ));
                                buttons.spawn((
                                    secondary_action_button_key(
                                        theme,
                                        fonts,
                                        i18n,
                                        "ui_gallery.overlays.hide",
                                        "Hide",
                                    ),
                                    GalleryActionButton::HideLoading,
                                ));
                                buttons.spawn((
                                    primary_action_button_key(
                                        theme,
                                        fonts,
                                        i18n,
                                        "ui_gallery.overlays.show_confirm",
                                        "Show Confirm",
                                    ),
                                    GalleryActionButton::Confirm,
                                ));
                                buttons.spawn((
                                    secondary_action_button_key(
                                        theme,
                                        fonts,
                                        i18n,
                                        "ui_gallery.overlays.show_floating",
                                        "Show Floating",
                                    ),
                                    GalleryActionButton::Floating,
                                ));
                                buttons.spawn((
                                    secondary_action_button_key(
                                        theme,
                                        fonts,
                                        i18n,
                                        "ui_gallery.overlays.close_top",
                                        "Close Top",
                                    ),
                                    GalleryActionButton::CloseTop,
                                ));
                            });
                    });
            });
        });
}

pub(super) fn handle_ui_gallery_buttons(
    mut commands: Commands,
    i18n: Res<UiI18n>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
    mut route_commands: MessageWriter<UiRouteCommand>,
    buttons: Query<
        (&Interaction, &GalleryActionButton),
        (
            Changed<Interaction>,
            With<Button>,
            Without<DisabledButton>,
            Without<LoadingButton>,
        ),
    >,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match action {
            GalleryActionButton::Toast => {
                route_commands.write(UiRouteCommand::ShowToast(UiToast::new_key(
                    &i18n,
                    "ui_gallery.toast.preview",
                    "Toast from UI Gallery",
                )));
            }
            GalleryActionButton::ShowLoading => {
                commands.insert_resource(GalleryLoadingPreview::new());
                panel_commands.write(UiPanelCommand::Open(UiPanelRequest::Loading(
                    UiLoading::new_key(&i18n, "ui_gallery.loading.preview", "Loading preview"),
                )));
            }
            GalleryActionButton::ShowCancellableLoading => {
                commands.insert_resource(GalleryLoadingPreview::new());
                panel_commands.write(UiPanelCommand::Open(UiPanelRequest::Loading(
                    UiLoading::new_key(
                        &i18n,
                        "ui_gallery.loading.cancelable",
                        "Cancelable loading",
                    )
                    .cancellable(),
                )));
            }
            GalleryActionButton::HideLoading => {
                commands.remove_resource::<GalleryLoadingPreview>();
                panel_commands.write(UiPanelCommand::Close(UiPanelId::GlobalLoading));
            }
            GalleryActionButton::Confirm => {
                panel_commands.write(UiPanelCommand::Open(UiPanelRequest::Confirm(
                    gallery_confirm_modal(&i18n),
                )));
            }
            GalleryActionButton::Floating => {
                commands.insert_resource(gallery_floating_i18n(&i18n));
                panel_commands.write(UiPanelCommand::Open(UiPanelRequest::Floating(
                    gallery_floating_panel(&i18n),
                )));
            }
            GalleryActionButton::CloseTop => {
                panel_commands.write(UiPanelCommand::CloseTop);
            }
        }
    }
}

pub(super) fn log_ui_gallery_text_input_submissions(
    mut submissions: MessageReader<UiTextInputSubmitted>,
) {
    for submission in submissions.read() {
        info!(
            entity = ?submission.entity,
            value = %submission.value,
            "ui gallery text input submitted"
        );
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
    commands.remove_resource::<GalleryFloatingI18n>();
}

pub(super) fn tag_gallery_floating_i18n_texts(
    mut commands: Commands,
    floating_i18n: Option<Res<GalleryFloatingI18n>>,
    panel_roots: Query<(Entity, &UiPanelRoot)>,
    children: Query<&Children>,
    texts: Query<(Entity, &Text), Without<UiI18nText>>,
) {
    let Some(floating_i18n) = floating_i18n else {
        return;
    };

    let Some(panel_root_entity) = panel_roots
        .iter()
        .find_map(|(entity, panel)| (panel.id == floating_i18n.panel_id).then_some(entity))
    else {
        return;
    };

    for entity in children.iter_descendants(panel_root_entity) {
        let Ok((text_entity, text)) = texts.get(entity) else {
            continue;
        };

        let marker = if text.0 == floating_i18n.title.text {
            Some(floating_i18n.title.i18n_text.clone())
        } else if text.0 == floating_i18n.body.text {
            Some(floating_i18n.body.i18n_text.clone())
        } else {
            floating_i18n
                .detail
                .as_ref()
                .filter(|detail| text.0 == detail.text)
                .map(|detail| detail.i18n_text.clone())
        };

        if let Some(marker) = marker {
            commands.entity(text_entity).insert(marker);
        }
    }
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
        UiThemeBackgroundRole::Panel,
        UiThemeBorderRole::Panel,
    )
}

fn section_label_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    screen_label_key(
        theme,
        fonts,
        i18n,
        key,
        fallback,
        theme.text.section_label,
        UiThemeTextColorRole::Muted,
    )
}

fn primary_route_button_sample(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
) -> impl Bundle {
    (
        primary_action_button_key(theme, fonts, i18n, "ui_gallery.buttons.action", "Action"),
        Name::new("Gallery action sample"),
    )
}

fn gallery_confirm_modal(i18n: &UiI18n) -> UiConfirmModal {
    let title = UiI18nTextSpec::new(i18n, "ui_gallery.confirm.title", "Gallery Confirm");
    let body = UiI18nTextSpec::new(
        i18n,
        "ui_gallery.confirm.body",
        "This confirms modal layering and input blocking.",
    );
    let detail = UiI18nTextSpec::new(
        i18n,
        "ui_gallery.confirm.detail",
        "The page buttons below should not react while this is open.",
    );
    let cancel = UiI18nTextSpec::new(i18n, "common.cancel", "Cancel");
    let confirm = UiI18nTextSpec::new(i18n, "common.confirm", "Confirm");

    UiConfirmModal {
        id: UiModalId::GalleryConfirm,
        title: title.text,
        body: body.text,
        detail: Some(detail.text),
        title_i18n_text: Some(title.i18n_text),
        body_i18n_text: Some(body.i18n_text),
        detail_i18n_text: Some(detail.i18n_text),
        actions: vec![
            UiModalActionSpec {
                label: cancel.text,
                action: UiModalAction::Cancel,
                style: UiModalActionStyle::Secondary,
                i18n_text: Some(cancel.i18n_text),
            },
            UiModalActionSpec {
                label: confirm.text,
                action: UiModalAction::Confirm,
                style: UiModalActionStyle::Primary,
                i18n_text: Some(confirm.i18n_text),
            },
        ],
    }
}

fn gallery_floating_panel(i18n: &UiI18n) -> UiFloatingPanel {
    UiFloatingPanel {
        id: UiPanelId::GalleryFloating,
        title: i18n.tr("ui_gallery.floating.title", "Floating Panel"),
        body: i18n.tr(
            "ui_gallery.floating.body",
            "This panel does not cover the whole page.",
        ),
        detail: Some(i18n.tr(
            "ui_gallery.floating.detail",
            "Use Close Top or Esc to close it.",
        )),
    }
}

fn gallery_floating_i18n(i18n: &UiI18n) -> GalleryFloatingI18n {
    GalleryFloatingI18n {
        panel_id: UiPanelId::GalleryFloating,
        title: UiI18nTextSpec::new(i18n, "ui_gallery.floating.title", "Floating Panel"),
        body: UiI18nTextSpec::new(
            i18n,
            "ui_gallery.floating.body",
            "This panel does not cover the whole page.",
        ),
        detail: Some(UiI18nTextSpec::new(
            i18n,
            "ui_gallery.floating.detail",
            "Use Close Top or Esc to close it.",
        )),
    }
}
