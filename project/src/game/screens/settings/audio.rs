use bevy::prelude::*;

use crate::framework::{
    audio::prelude::{
        AudioBus, AudioBusMutedCommand, AudioBusVolumeCommand, AudioCommand, AudioMixer,
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
            UiAlign, UiButtonEvent, UiButtonEventKind, UiJustify, controls::UiSlider,
            screen_label_key, screen_title_key, slider_key, toggle_key, toggle_on_key,
            ui_scroll_column,
        },
    },
};
use crate::game::{
    navigation::{AppUiMode, game_panel_root, secondary_route_button_key},
    ui_ids::{OWNER_AUDIO_SETTINGS, PANEL_AUDIO_SETTINGS},
};

const VOLUME_MIN_PERCENT: f32 = 0.0;
const VOLUME_MAX_PERCENT: f32 = 100.0;
const AUDIO_VOLUME_BUSES: [AudioBus; 5] = [
    AudioBus::Master,
    AudioBus::Music,
    AudioBus::Sfx,
    AudioBus::Ui,
    AudioBus::Battle,
];

#[derive(Clone, Copy, Debug, Component, Eq, PartialEq)]
pub(super) struct AudioVolumeSlider {
    bus: AudioBus,
}

#[derive(Clone, Copy, Debug, Component)]
pub(super) struct MasterMuteToggle;

pub(super) fn setup_audio_settings(
    mut commands: Commands,
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    viewport: Res<UiViewport>,
    fonts: Res<UiFontAssets>,
    i18n: Res<UiI18n>,
    mixer: Res<AudioMixer>,
    mut clear_color: ResMut<ClearColor>,
) {
    let theme = theme.into_inner();
    let metrics = metrics.into_inner();
    let viewport = viewport.into_inner();
    let fonts = fonts.into_inner();
    let i18n = i18n.into_inner();
    let mixer = mixer.into_inner();
    clear_color.0 = theme.colors.screen_background;

    commands
        .spawn((
            DespawnOnExit(AppUiMode::AudioSettings),
            game_panel_root(
                PANEL_AUDIO_SETTINGS,
                UiPanelKind::Page,
                OWNER_AUDIO_SETTINGS,
            ),
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
            root.spawn(audio_settings_header(theme, metrics, viewport.width_class))
                .with_children(|header| {
                    header.spawn(screen_title_key(
                        theme,
                        fonts,
                        i18n,
                        "audio_settings.title",
                        "Audio Settings",
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
                body.spawn(audio_settings_panel(theme))
                    .with_children(|panel| {
                        panel.spawn(screen_label_key(
                            theme,
                            fonts,
                            i18n,
                            "audio_settings.master.section",
                            "Master",
                            UiThemeTextStyleRole::SectionLabel,
                            UiThemeTextColorRole::Muted,
                        ));
                        spawn_master_mute_toggle(panel, theme, fonts, i18n, mixer);
                    });

                body.spawn(audio_settings_panel(theme))
                    .with_children(|panel| {
                        panel.spawn(screen_label_key(
                            theme,
                            fonts,
                            i18n,
                            "audio_settings.volume.section",
                            "Volume",
                            UiThemeTextStyleRole::SectionLabel,
                            UiThemeTextColorRole::Muted,
                        ));
                        for bus in AUDIO_VOLUME_BUSES {
                            spawn_volume_slider(panel, theme, metrics, fonts, i18n, mixer, bus);
                        }
                    });
            });
        });
}

pub(super) fn handle_audio_settings_sliders(
    sliders: Query<(&AudioVolumeSlider, Ref<UiSlider>), Changed<UiSlider>>,
    mut audio_commands: MessageWriter<AudioCommand>,
) {
    for (marker, slider) in &sliders {
        if slider.is_added() {
            continue;
        }

        audio_commands.write(AudioCommand::SetBusVolume(AudioBusVolumeCommand::new(
            marker.bus,
            percent_to_bus_volume(slider.value),
        )));
    }
}

pub(super) fn handle_audio_settings_master_mute_toggle(
    toggles: Query<(), With<MasterMuteToggle>>,
    mixer: Res<AudioMixer>,
    mut button_events: MessageReader<UiButtonEvent>,
    mut audio_commands: MessageWriter<AudioCommand>,
) {
    for event in button_events.read() {
        if event.kind != UiButtonEventKind::Click {
            continue;
        }

        if toggles.get(event.entity).is_err() {
            continue;
        }
        let next_muted = !mixer.bus_state(AudioBus::Master).muted;

        audio_commands.write(AudioCommand::SetBusMuted(AudioBusMutedCommand::new(
            AudioBus::Master,
            next_muted,
        )));
    }
}

fn spawn_master_mute_toggle(
    panel: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    mixer: &AudioMixer,
) {
    let muted = mixer.bus_state(AudioBus::Master).muted;
    let mut toggle = if muted {
        panel.spawn(toggle_on_key(
            theme,
            fonts,
            i18n,
            "audio_settings.master.muted",
            "Master Muted",
        ))
    } else {
        panel.spawn(toggle_key(
            theme,
            fonts,
            i18n,
            "audio_settings.master.muted",
            "Master Muted",
        ))
    };
    toggle.insert(MasterMuteToggle);
}

fn spawn_volume_slider(
    panel: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    mixer: &AudioMixer,
    bus: AudioBus,
) {
    panel.spawn((
        slider_key(
            theme,
            metrics,
            fonts,
            i18n,
            bus_label_key(bus),
            bus_label_fallback(bus),
            bus_volume_to_percent(mixer.bus_state(bus).volume),
            VOLUME_MIN_PERCENT,
            VOLUME_MAX_PERCENT,
        ),
        AudioVolumeSlider { bus },
    ));
}

fn audio_settings_header(
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

fn audio_settings_panel(theme: &UiTheme) -> impl Bundle {
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

fn bus_volume_to_percent(volume: f32) -> f32 {
    (volume * VOLUME_MAX_PERCENT).clamp(VOLUME_MIN_PERCENT, VOLUME_MAX_PERCENT)
}

fn percent_to_bus_volume(percent: f32) -> f32 {
    (percent / VOLUME_MAX_PERCENT).clamp(0.0, 1.0)
}

fn bus_label_key(bus: AudioBus) -> &'static str {
    match bus {
        AudioBus::Master => "audio_settings.bus.master",
        AudioBus::Music => "audio_settings.bus.music",
        AudioBus::Sfx => "audio_settings.bus.sfx",
        AudioBus::Ui => "audio_settings.bus.ui",
        AudioBus::Battle => "audio_settings.bus.battle",
    }
}

fn bus_label_fallback(bus: AudioBus) -> &'static str {
    match bus {
        AudioBus::Master => "Master",
        AudioBus::Music => "Music",
        AudioBus::Sfx => "SFX",
        AudioBus::Ui => "UI",
        AudioBus::Battle => "Battle",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::ui::widgets::controls::UiToggleOn;
    use bevy::ecs::message::MessageCursor;

    fn read_audio_commands(app: &App) -> Vec<AudioCommand> {
        let messages = app.world().resource::<Messages<AudioCommand>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    #[test]
    fn volume_percent_converts_to_bus_volume() {
        assert_eq!(percent_to_bus_volume(0.0), 0.0);
        assert_eq!(percent_to_bus_volume(25.0), 0.25);
        assert_eq!(percent_to_bus_volume(100.0), 1.0);
        assert_eq!(percent_to_bus_volume(-5.0), 0.0);
        assert_eq!(percent_to_bus_volume(150.0), 1.0);
    }

    #[test]
    fn bus_volume_converts_to_slider_percent() {
        assert_eq!(bus_volume_to_percent(0.0), 0.0);
        assert_eq!(bus_volume_to_percent(0.25), 25.0);
        assert_eq!(bus_volume_to_percent(1.0), 100.0);
        assert_eq!(bus_volume_to_percent(-1.0), 0.0);
        assert_eq!(bus_volume_to_percent(1.5), 100.0);
    }

    #[test]
    fn changed_slider_sends_bus_volume_command() {
        let mut app = App::new();
        app.add_message::<AudioCommand>()
            .add_systems(Update, handle_audio_settings_sliders);

        let entity = app
            .world_mut()
            .spawn((
                AudioVolumeSlider {
                    bus: AudioBus::Music,
                },
                UiSlider::new(100.0, 0.0, 100.0),
            ))
            .id();
        app.update();
        read_audio_commands(&app);

        app.world_mut()
            .entity_mut(entity)
            .get_mut::<UiSlider>()
            .unwrap()
            .value = 37.0;
        app.update();

        assert_eq!(
            read_audio_commands(&app),
            vec![AudioCommand::SetBusVolume(AudioBusVolumeCommand::new(
                AudioBus::Music,
                0.37,
            ))]
        );
    }

    #[test]
    fn master_mute_toggle_sends_muted_true_when_mixer_unmuted_without_visual_marker() {
        let mut app = App::new();
        app.add_message::<UiButtonEvent>()
            .add_message::<AudioCommand>()
            .init_resource::<AudioMixer>()
            .add_systems(Update, handle_audio_settings_master_mute_toggle);

        let toggle = app.world_mut().spawn((Button, MasterMuteToggle)).id();
        app.world_mut().write_message(UiButtonEvent {
            entity: toggle,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.update();

        assert_eq!(
            read_audio_commands(&app),
            vec![AudioCommand::SetBusMuted(AudioBusMutedCommand::new(
                AudioBus::Master,
                true,
            ))]
        );
    }

    #[test]
    fn master_mute_toggle_sends_muted_true_when_mixer_unmuted_with_visual_marker() {
        let mut app = App::new();
        app.add_message::<UiButtonEvent>()
            .add_message::<AudioCommand>()
            .init_resource::<AudioMixer>()
            .add_systems(Update, handle_audio_settings_master_mute_toggle);

        let toggle = app
            .world_mut()
            .spawn((Button, MasterMuteToggle, UiToggleOn))
            .id();
        app.world_mut().write_message(UiButtonEvent {
            entity: toggle,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.update();

        assert_eq!(
            read_audio_commands(&app),
            vec![AudioCommand::SetBusMuted(AudioBusMutedCommand::new(
                AudioBus::Master,
                true,
            ))]
        );
    }

    #[test]
    fn master_mute_toggle_sends_muted_false_when_mixer_muted() {
        let mut app = App::new();
        app.add_message::<UiButtonEvent>()
            .add_message::<AudioCommand>()
            .init_resource::<AudioMixer>()
            .add_systems(Update, handle_audio_settings_master_mute_toggle);
        app.world_mut()
            .resource_mut::<AudioMixer>()
            .set_bus_muted(AudioBus::Master, true);

        let toggle = app.world_mut().spawn((Button, MasterMuteToggle)).id();
        app.world_mut().write_message(UiButtonEvent {
            entity: toggle,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.update();

        assert_eq!(
            read_audio_commands(&app),
            vec![AudioCommand::SetBusMuted(AudioBusMutedCommand::new(
                AudioBus::Master,
                false,
            ))]
        );
    }

    #[test]
    fn audio_settings_covers_expected_buses() {
        assert_eq!(
            AUDIO_VOLUME_BUSES,
            [
                AudioBus::Master,
                AudioBus::Music,
                AudioBus::Sfx,
                AudioBus::Ui,
                AudioBus::Battle,
            ]
        );
    }
}
