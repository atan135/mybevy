use bevy::prelude::*;

use crate::framework::{
    audio::prelude::{
        AudioBus, AudioDebugConfig, AudioDebugInstanceInfo, AudioDebugLoadingGroupInfo,
        AudioDebugSnapshot,
    },
    ui::{
        core::{UiLayer, UiLayerRoot, UiMetrics, UiPanelKind, UiViewport, UiWidthClass},
        i18n::UiI18n,
        style::{
            UiFontAssets, UiTheme,
            theme::{
                UiThemeBackgroundRole, UiThemeBorderRole, UiThemePanelNodeRole,
                UiThemeRootNodeRole, UiThemeTextColorRole, UiThemeTextStyleRole,
            },
        },
        widgets::{
            UiAlign, UiJustify, screen_label, screen_label_key, screen_title_key, ui_scroll_column,
        },
    },
};
use crate::game::{
    navigation::{AppUiMode, game_panel_root, secondary_route_button_key},
    ui_ids::{OWNER_AUDIO_MONITOR, PANEL_AUDIO_MONITOR},
};

const MONITORED_BUSES: [AudioBus; 4] = [
    AudioBus::Music,
    AudioBus::Sfx,
    AudioBus::Ui,
    AudioBus::Battle,
];
const DIRECTORY_ROW_LIMIT: usize = 8;
const LOADING_ROW_LIMIT: usize = 8;
const THRESHOLD_ROW_LIMIT: usize = 8;
const RECENT_ROW_LIMIT: usize = 8;
const INSTANCE_ROW_LIMIT: usize = 24;

#[derive(Clone, Copy, Debug, Component, Eq, PartialEq)]
pub(super) enum AudioMonitorTextRow {
    Summary(usize),
    Memory(usize),
    Directory(usize),
    Loading(usize),
    Started(usize),
    Skipped(usize),
    Failure(usize),
    Hint(usize),
    Instance(usize),
}

pub(super) fn enable_audio_monitor_debug(mut config: ResMut<AudioDebugConfig>) {
    config.enabled = true;
}

pub(super) fn setup_audio_monitor(
    mut commands: Commands,
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    viewport: Res<UiViewport>,
    fonts: Res<UiFontAssets>,
    i18n: Res<UiI18n>,
    snapshot: Res<AudioDebugSnapshot>,
    mut clear_color: ResMut<ClearColor>,
) {
    let theme = theme.into_inner();
    let metrics = metrics.into_inner();
    let viewport = viewport.into_inner();
    let fonts = fonts.into_inner();
    let i18n = i18n.into_inner();
    let snapshot = snapshot.into_inner();
    clear_color.0 = theme.colors.screen_background;

    commands
        .spawn((
            DespawnOnExit(AppUiMode::AudioMonitor),
            game_panel_root(PANEL_AUDIO_MONITOR, UiPanelKind::Page, OWNER_AUDIO_MONITOR),
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
            root.spawn(audio_monitor_header(theme, metrics, viewport.width_class))
                .with_children(|header| {
                    header.spawn(screen_title_key(
                        theme,
                        fonts,
                        i18n,
                        "audio_monitor.title",
                        "Audio Monitor",
                        UiThemeTextStyleRole::Title,
                    ));
                    header.spawn(secondary_route_button_key(
                        theme,
                        metrics,
                        fonts,
                        i18n,
                        "nav.audio_gallery",
                        "Audio Gallery",
                        AppUiMode::AudioGallery,
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
                        "nav.lobby",
                        "Lobby",
                        AppUiMode::Lobby,
                    ));
                });

            root.spawn(ui_scroll_column(theme)).with_children(|body| {
                body.spawn(audio_monitor_panel(theme))
                    .with_children(|panel| {
                        panel.spawn(section_label(
                            theme,
                            fonts,
                            i18n,
                            "audio_monitor.summary.section",
                            "Playback",
                        ));
                        for index in 0..5 {
                            spawn_metric_slot(
                                panel,
                                theme,
                                fonts,
                                AudioMonitorTextRow::Summary(index),
                                snapshot,
                            );
                        }
                    });

                body.spawn(audio_monitor_panel(theme))
                    .with_children(|panel| {
                        panel.spawn(section_label(
                            theme,
                            fonts,
                            i18n,
                            "audio_monitor.memory.section",
                            "Audio Resource Bytes",
                        ));
                        for index in 0..3 {
                            spawn_metric_slot(
                                panel,
                                theme,
                                fonts,
                                AudioMonitorTextRow::Memory(index),
                                snapshot,
                            );
                        }
                        for index in 0..DIRECTORY_ROW_LIMIT {
                            spawn_metric_slot(
                                panel,
                                theme,
                                fonts,
                                AudioMonitorTextRow::Directory(index),
                                snapshot,
                            );
                        }
                    });

                body.spawn(audio_monitor_panel(theme))
                    .with_children(|panel| {
                        panel.spawn(section_label(
                            theme,
                            fonts,
                            i18n,
                            "audio_monitor.loading.section",
                            "Loading Groups",
                        ));
                        for index in 0..LOADING_ROW_LIMIT {
                            spawn_metric_slot(
                                panel,
                                theme,
                                fonts,
                                AudioMonitorTextRow::Loading(index),
                                snapshot,
                            );
                        }
                    });

                body.spawn(audio_monitor_panel(theme))
                    .with_children(|panel| {
                        panel.spawn(section_label(
                            theme,
                            fonts,
                            i18n,
                            "audio_monitor.recent.section",
                            "Recent Activity",
                        ));
                        panel.spawn(metric_label(theme, fonts, "Started cues:"));
                        for index in 0..RECENT_ROW_LIMIT {
                            spawn_metric_slot(
                                panel,
                                theme,
                                fonts,
                                AudioMonitorTextRow::Started(index),
                                snapshot,
                            );
                        }
                        panel.spawn(metric_label(theme, fonts, "Skipped cues:"));
                        for index in 0..RECENT_ROW_LIMIT {
                            spawn_metric_slot(
                                panel,
                                theme,
                                fonts,
                                AudioMonitorTextRow::Skipped(index),
                                snapshot,
                            );
                        }
                        panel.spawn(metric_label(theme, fonts, "Load failures:"));
                        for index in 0..RECENT_ROW_LIMIT {
                            spawn_metric_slot(
                                panel,
                                theme,
                                fonts,
                                AudioMonitorTextRow::Failure(index),
                                snapshot,
                            );
                        }
                    });

                body.spawn(audio_monitor_panel(theme))
                    .with_children(|panel| {
                        panel.spawn(section_label(
                            theme,
                            fonts,
                            i18n,
                            "audio_monitor.thresholds.section",
                            "Threshold Hints",
                        ));
                        for index in 0..THRESHOLD_ROW_LIMIT {
                            spawn_metric_slot(
                                panel,
                                theme,
                                fonts,
                                AudioMonitorTextRow::Hint(index),
                                snapshot,
                            );
                        }
                    });

                body.spawn(audio_monitor_panel(theme))
                    .with_children(|panel| {
                        panel.spawn(section_label(
                            theme,
                            fonts,
                            i18n,
                            "audio_monitor.instances.section",
                            "Instances",
                        ));
                        for index in 0..=(INSTANCE_ROW_LIMIT + 1) {
                            spawn_metric_slot(
                                panel,
                                theme,
                                fonts,
                                AudioMonitorTextRow::Instance(index),
                                snapshot,
                            );
                        }
                    });
            });
        });
}

pub(super) fn refresh_audio_monitor_text(
    snapshot: Res<AudioDebugSnapshot>,
    mut rows: Query<(&AudioMonitorTextRow, &mut Text)>,
) {
    if !snapshot.is_changed() {
        return;
    }

    for (row, mut text) in &mut rows {
        text.0 = audio_monitor_row_text(*row, &snapshot);
    }
}

fn spawn_metric_slot(
    panel: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    fonts: &UiFontAssets,
    row: AudioMonitorTextRow,
    snapshot: &AudioDebugSnapshot,
) {
    panel.spawn((
        metric_label(theme, fonts, audio_monitor_row_text(row, snapshot)),
        row,
    ));
}

fn audio_monitor_header(
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

fn audio_monitor_panel(theme: &UiTheme) -> impl Bundle {
    (
        UiThemePanelNodeRole::Content,
        Node {
            width: percent(100),
            max_width: px(theme.layout.content_width),
            align_self: AlignSelf::Center,
            flex_direction: FlexDirection::Column,
            row_gap: px(theme.layout.row_gap),
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

fn monitored_bus_counts(snapshot: &AudioDebugSnapshot) -> String {
    MONITORED_BUSES
        .iter()
        .map(|bus| format!("{}={}", bus, bus_count(snapshot, *bus)))
        .collect::<Vec<_>>()
        .join("  ")
}

fn bus_count(snapshot: &AudioDebugSnapshot, bus: AudioBus) -> usize {
    snapshot
        .active_instances
        .by_bus
        .iter()
        .find_map(|entry| (entry.bus == bus).then_some(entry.count))
        .unwrap_or(0)
}

fn loading_group_line(group: &AudioDebugLoadingGroupInfo) -> String {
    format!(
        "{}: {}/{} loaded, {} failed, required {}/{} loaded, {} required failed",
        group.group_id,
        group.loaded,
        group.total,
        group.failed,
        group.required_loaded,
        group.required_total,
        group.required_failed
    )
}

fn audio_monitor_row_text(row: AudioMonitorTextRow, snapshot: &AudioDebugSnapshot) -> String {
    match row {
        AudioMonitorTextRow::Summary(0) => {
            format!("Debug enabled: {}", yes_no(snapshot.enabled))
        }
        AudioMonitorTextRow::Summary(1) => {
            format!("Active total: {}", snapshot.active_instances.total)
        }
        AudioMonitorTextRow::Summary(2) => {
            format!("By bus: {}", monitored_bus_counts(snapshot))
        }
        AudioMonitorTextRow::Summary(3) => format!(
            "Paused: {}   Stopping/fading: {}   Spatial: {}   Looped: {}",
            snapshot.performance.paused_instances,
            snapshot.performance.stopping_or_fading_instances,
            snapshot.performance.spatial_instances,
            snapshot.performance.looped_instances
        ),
        AudioMonitorTextRow::Summary(4) => format!(
            "Referenced resource bytes estimate: {}",
            format_bytes(snapshot.performance.referenced_resource_estimated_bytes)
        ),
        AudioMonitorTextRow::Memory(0) => {
            "Numbers are audio resource bytes / estimated memory, not process memory.".to_string()
        }
        AudioMonitorTextRow::Memory(1) => format!(
            "Resources: {}   Total: {}",
            snapshot.resource_memory.resource_count,
            format_bytes(snapshot.resource_memory.total_estimated_bytes)
        ),
        AudioMonitorTextRow::Memory(2) => format!(
            "Largest: {} ({})",
            snapshot
                .resource_memory
                .largest_resource_path
                .as_deref()
                .unwrap_or("none"),
            format_bytes(snapshot.resource_memory.largest_resource_estimated_bytes)
        ),
        AudioMonitorTextRow::Directory(index) => snapshot
            .resource_memory
            .by_directory
            .get(index)
            .map(|entry| {
                format!(
                    "{}: {}",
                    entry.directory,
                    format_bytes(entry.estimated_bytes)
                )
            })
            .unwrap_or_default(),
        AudioMonitorTextRow::Loading(index) => {
            if snapshot.loading_groups.is_empty() && index == 0 {
                "No loading groups.".to_string()
            } else {
                snapshot
                    .loading_groups
                    .get(index)
                    .map(loading_group_line)
                    .unwrap_or_default()
            }
        }
        AudioMonitorTextRow::Started(index) => snapshot
            .recent_started_cues
            .iter()
            .rev()
            .nth(index)
            .map(|cue| {
                format!(
                    "  {} -> {} #{} {} {}",
                    cue.cue_id, cue.clip_id, cue.instance_id, cue.bus, cue.scope
                )
            })
            .or_else(|| {
                (snapshot.recent_started_cues.is_empty() && index == 0)
                    .then(|| "  none".to_string())
            })
            .unwrap_or_default(),
        AudioMonitorTextRow::Skipped(index) => snapshot
            .recent_skipped_cues
            .iter()
            .rev()
            .nth(index)
            .map(|cue| format!("  {} {:?} {}", cue.cue_id, cue.reason, cue.scope))
            .or_else(|| {
                (snapshot.recent_skipped_cues.is_empty() && index == 0)
                    .then(|| "  none".to_string())
            })
            .unwrap_or_default(),
        AudioMonitorTextRow::Failure(index) => snapshot
            .recent_load_failures
            .iter()
            .rev()
            .nth(index)
            .map(|failure| {
                format!(
                    "  {} {}",
                    failure.asset_path.as_deref().unwrap_or("unknown asset"),
                    failure.message
                )
            })
            .or_else(|| {
                (snapshot.recent_load_failures.is_empty() && index == 0)
                    .then(|| "  none".to_string())
            })
            .unwrap_or_default(),
        AudioMonitorTextRow::Hint(index) => {
            if snapshot.performance.threshold_hints.is_empty() && index == 0 {
                "No threshold hints.".to_string()
            } else {
                snapshot
                    .performance
                    .threshold_hints
                    .get(index)
                    .map(|hint| format!("! {hint}"))
                    .unwrap_or_default()
            }
        }
        AudioMonitorTextRow::Instance(0) => {
            "id | clip | cue | bus | scope | path | flags | progress".to_string()
        }
        AudioMonitorTextRow::Instance(1) if snapshot.instance_details.is_empty() => {
            "No active instances.".to_string()
        }
        AudioMonitorTextRow::Instance(index @ 1..=INSTANCE_ROW_LIMIT) => snapshot
            .instance_details
            .get(index - 1)
            .map(instance_line)
            .unwrap_or_default(),
        AudioMonitorTextRow::Instance(index) if index == INSTANCE_ROW_LIMIT + 1 => {
            if snapshot.instance_details.len() > INSTANCE_ROW_LIMIT {
                format!(
                    "... {} more instances",
                    snapshot.instance_details.len() - INSTANCE_ROW_LIMIT
                )
            } else {
                String::new()
            }
        }
        _ => String::new(),
    }
}

fn instance_line(instance: &AudioDebugInstanceInfo) -> String {
    format!(
        "{} | {} | {} | {} | {} | {} | paused={} stopping={} failed={} spatial={} loop={} | {:.2}/{}, start {:.2}, seek {}",
        instance.instance_id,
        instance.clip_id,
        instance
            .cue_id
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "-".to_string()),
        instance.bus,
        instance.scope,
        instance.asset_path,
        yes_no(instance.paused),
        yes_no(instance.stopping),
        yes_no(instance.failed),
        yes_no(instance.spatial),
        yes_no(instance.looped),
        instance.position_seconds,
        instance
            .duration_seconds
            .map(|seconds| format!("{seconds:.2}"))
            .unwrap_or_else(|| "-".to_string()),
        instance.start_seconds,
        instance
            .pending_seek_seconds
            .map(|seconds| format!("{seconds:.2}"))
            .unwrap_or_else(|| "-".to_string())
    )
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!(
            "{bytes} bytes ({:.2} MiB)",
            bytes as f64 / (1024.0 * 1024.0)
        )
    } else if bytes >= 1024 {
        format!("{bytes} bytes ({:.1} KiB)", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} bytes")
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::audio::prelude::{
        AudioDebugActiveInstanceCounts, AudioDebugBusInstanceCount, AudioInstanceId,
    };
    use bevy::state::app::StatesPlugin;

    #[test]
    fn monitored_bus_counts_includes_expected_runtime_buses() {
        let snapshot = AudioDebugSnapshot {
            active_instances: AudioDebugActiveInstanceCounts {
                total: 3,
                by_bus: vec![
                    AudioDebugBusInstanceCount {
                        bus: AudioBus::Ui,
                        count: 2,
                    },
                    AudioDebugBusInstanceCount {
                        bus: AudioBus::Music,
                        count: 1,
                    },
                ],
            },
            ..default()
        };

        assert_eq!(
            monitored_bus_counts(&snapshot),
            "music=1  sfx=0  ui=2  battle=0"
        );
    }

    #[test]
    fn bytes_are_formatted_with_raw_bytes_and_units() {
        assert_eq!(format_bytes(12), "12 bytes");
        assert_eq!(format_bytes(2048), "2048 bytes (2.0 KiB)");
        assert_eq!(format_bytes(2 * 1024 * 1024), "2097152 bytes (2.00 MiB)");
    }

    #[test]
    fn instance_line_contains_required_debug_columns() {
        let line = instance_line(&AudioDebugInstanceInfo {
            instance_id: AudioInstanceId::from_raw(7),
            clip_id: "ui.click".try_into().unwrap(),
            cue_id: Some("ui.button.click".try_into().unwrap()),
            scope: crate::framework::audio::prelude::AudioScope::Ui,
            bus: AudioBus::Ui,
            asset_path: "audio/ui/click.wav".to_string(),
            paused: true,
            stopping: false,
            failed: false,
            spatial: true,
            looped: false,
            start_seconds: 1.0,
            position_seconds: 1.5,
            duration_seconds: Some(3.0),
            pending_seek_seconds: Some(2.0),
        });

        assert!(line.contains("7 | ui.click | ui.button.click | ui | ui"));
        assert!(line.contains("audio/ui/click.wav"));
        assert!(line.contains("paused=yes"));
        assert!(line.contains("spatial=yes"));
        assert!(line.contains("loop=no"));
        assert!(line.contains("1.50/3.00"));
        assert!(line.contains("seek 2.00"));
    }

    #[test]
    fn refresh_audio_monitor_text_updates_marked_rows_from_snapshot() {
        let mut app = App::new();
        app.init_resource::<AudioDebugSnapshot>()
            .add_systems(Update, refresh_audio_monitor_text);
        let entity = app
            .world_mut()
            .spawn((Text::new("old"), AudioMonitorTextRow::Summary(1)))
            .id();
        app.world_mut()
            .resource_mut::<AudioDebugSnapshot>()
            .active_instances
            .total = 12;

        app.update();

        assert_eq!(
            app.world().entity(entity).get::<Text>().unwrap().0,
            "Active total: 12"
        );
    }

    #[test]
    fn entering_audio_monitor_enables_debug_capture() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .insert_resource(AudioDebugConfig { enabled: false })
            .init_state::<AppUiMode>()
            .add_systems(OnEnter(AppUiMode::AudioMonitor), enable_audio_monitor_debug);

        app.world_mut()
            .resource_mut::<NextState<AppUiMode>>()
            .set(AppUiMode::AudioMonitor);
        app.update();

        assert!(app.world().resource::<AudioDebugConfig>().enabled);
    }
}
