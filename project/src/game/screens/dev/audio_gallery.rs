use bevy::prelude::*;

use crate::framework::ui::{
    core::{UiLayer, UiLayerRoot, UiMetrics, UiPanelKind, UiViewport, UiWidthClass},
    i18n::UiI18n,
    style::{
        UiFontAssets, UiTheme,
        theme::{
            UiThemeBackgroundRole, UiThemeBorderRole, UiThemePanelNodeRole, UiThemeRootNodeRole,
            UiThemeTextColorRole, UiThemeTextStyleRole,
        },
    },
    widgets::{
        UiAlign, UiJustify, screen_label, screen_label_key, screen_title_key, ui_scroll_column,
    },
};
use crate::game::{
    navigation::{AppUiMode, game_panel_root, secondary_route_button_key},
    ui_ids::{OWNER_AUDIO_GALLERY, PANEL_AUDIO_GALLERY},
};

#[derive(Clone, Copy, Debug, Component, Eq, PartialEq)]
pub(super) enum AudioGalleryTextRow {
    Status,
}

#[derive(Debug, Resource)]
pub(super) struct AudioGalleryState {
    frames_open: u64,
}

impl AudioGalleryState {
    fn new() -> Self {
        Self { frames_open: 0 }
    }
}

pub(super) fn setup_audio_gallery(
    mut commands: Commands,
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    viewport: Res<UiViewport>,
    fonts: Res<UiFontAssets>,
    i18n: Res<UiI18n>,
    mut clear_color: ResMut<ClearColor>,
) {
    let theme = theme.into_inner();
    let metrics = metrics.into_inner();
    let viewport = viewport.into_inner();
    let fonts = fonts.into_inner();
    let i18n = i18n.into_inner();
    clear_color.0 = theme.colors.screen_background;
    commands.insert_resource(AudioGalleryState::new());

    commands
        .spawn((
            DespawnOnExit(AppUiMode::AudioGallery),
            game_panel_root(PANEL_AUDIO_GALLERY, UiPanelKind::Page, OWNER_AUDIO_GALLERY),
            UiLayerRoot {
                layer: UiLayer::Page,
            },
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                padding: viewport.safe_area_padding(metrics.page_padding),
                row_gap: px(theme.layout.page_gap),
                ..default()
            },
            BackgroundColor(theme.colors.screen_background),
            UiThemeBackgroundRole::Screen,
            UiThemeRootNodeRole::Screen,
        ))
        .with_children(|root| {
            root.spawn(audio_gallery_header(theme, metrics, viewport.width_class))
                .with_children(|header| {
                    header.spawn(screen_title_key(
                        theme,
                        fonts,
                        i18n,
                        "audio_gallery.title",
                        "Audio Gallery",
                        UiThemeTextStyleRole::Title,
                    ));
                    header.spawn(secondary_route_button_key(
                        theme,
                        metrics,
                        fonts,
                        i18n,
                        "nav.audio_settings",
                        "Audio Settings",
                        AppUiMode::AudioSettings,
                    ));
                    header.spawn(secondary_route_button_key(
                        theme,
                        metrics,
                        fonts,
                        i18n,
                        "nav.audio_monitor",
                        "Audio Monitor",
                        AppUiMode::AudioMonitor,
                    ));
                    header.spawn(secondary_route_button_key(
                        theme,
                        metrics,
                        fonts,
                        i18n,
                        "nav.lobby",
                        "Lobby",
                        AppUiMode::Lobby,
                    ));
                });

            root.spawn(ui_scroll_column(theme)).with_children(|body| {
                body.spawn(audio_gallery_panel(theme))
                    .with_children(|panel| {
                        panel.spawn(section_label(
                            theme,
                            fonts,
                            i18n,
                            "audio_gallery.overview.section",
                            "Planned Controls",
                        ));
                        panel.spawn(screen_label_key(
                            theme,
                            fonts,
                            i18n,
                            "audio_gallery.overview.body",
                            "Playback controls will be added in later slices.",
                            UiThemeTextStyleRole::Body,
                            UiThemeTextColorRole::Primary,
                        ));
                        panel.spawn(screen_label_key(
                            theme,
                            fonts,
                            i18n,
                            "audio_gallery.overview.boundary",
                            "This shell only verifies routing and page ownership.",
                            UiThemeTextStyleRole::Caption,
                            UiThemeTextColorRole::Muted,
                        ));
                    });

                body.spawn(audio_gallery_panel(theme))
                    .with_children(|panel| {
                        panel.spawn(section_label(
                            theme,
                            fonts,
                            i18n,
                            "audio_gallery.status.section",
                            "Status",
                        ));
                        panel.spawn((
                            metric_label(theme, fonts, audio_gallery_status_text(0)),
                            AudioGalleryTextRow::Status,
                        ));
                    });
            });
        });
}

pub(super) fn update_audio_gallery_status(
    mut state: ResMut<AudioGalleryState>,
    mut rows: Query<(&AudioGalleryTextRow, &mut Text)>,
) {
    state.frames_open = state.frames_open.saturating_add(1);

    for (row, mut text) in &mut rows {
        match row {
            AudioGalleryTextRow::Status => {
                text.0 = audio_gallery_status_text(state.frames_open);
            }
        }
    }
}

pub(super) fn clear_audio_gallery_state(mut commands: Commands) {
    commands.remove_resource::<AudioGalleryState>();
}

fn audio_gallery_header(
    theme: &UiTheme,
    metrics: &UiMetrics,
    width_class: UiWidthClass,
) -> impl Bundle {
    Node {
        width: percent(100),
        max_width: px(theme.layout.content_width.min(metrics.content_max_width)),
        align_self: AlignSelf::Center,
        align_items: if width_class == UiWidthClass::Compact {
            AlignItems::Stretch
        } else {
            UiAlign::Center.to_align_items()
        },
        justify_content: if width_class == UiWidthClass::Compact {
            JustifyContent::FlexStart
        } else {
            UiJustify::SpaceBetween.to_justify_content()
        },
        column_gap: px(metrics.control_gap),
        row_gap: px(metrics.control_gap),
        flex_wrap: FlexWrap::Wrap,
        ..default()
    }
}

fn audio_gallery_panel(theme: &UiTheme) -> impl Bundle {
    (
        UiThemePanelNodeRole::Content,
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

fn section_label(
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
        UiThemeTextStyleRole::SectionLabel,
        UiThemeTextColorRole::Muted,
    )
}

fn metric_label(theme: &UiTheme, fonts: &UiFontAssets, text: impl Into<String>) -> impl Bundle {
    screen_label(
        theme,
        fonts,
        text,
        UiThemeTextStyleRole::Caption,
        UiThemeTextColorRole::Primary,
    )
}

fn audio_gallery_status_text(frames_open: u64) -> String {
    format!("Audio Gallery shell active. Frames open: {frames_open}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_text_reports_frames_open() {
        assert_eq!(
            audio_gallery_status_text(3),
            "Audio Gallery shell active. Frames open: 3"
        );
    }
}
