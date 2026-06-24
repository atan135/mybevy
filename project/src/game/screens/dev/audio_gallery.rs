use std::fmt;

use bevy::prelude::*;

use crate::framework::{
    audio::prelude::{
        AudioBus, AudioClipId, AudioClipRequest, AudioCommand, AudioCrossfadeMusicRequest,
        AudioCueId, AudioCueRequest, AudioEvent, AudioInstanceCommand, AudioInstanceControlAction,
        AudioInstanceControlFailed, AudioInstanceControlFailureReason, AudioInstanceId,
        AudioInstanceProgress, AudioLoadFailed, AudioMusicChanged, AudioMusicFadeCommand,
        AudioMusicRequest, AudioScope, AudioScopeFadeCommand, AudioSeekInstanceCommand,
        AudioStopInstanceCommand,
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
            UiAlign, UiButtonEvent, UiButtonEventKind, UiJustify, UiResponsiveGridColumns,
            screen_label, screen_label_key, screen_title_key, secondary_action_button_key,
            toggle_key, toggle_on_key, ui_responsive_grid, ui_scroll_column,
        },
    },
};
use crate::game::{
    audio::dev_samples::{
        AUDIO_GALLERY_CAR_HORN_CUE_ID, AUDIO_GALLERY_FOOTSTEP_CUE_ID,
        AUDIO_GALLERY_MENU_MUSIC_CLIP_ID, AUDIO_GALLERY_RAIN_LOOP_CUE_ID,
        AUDIO_GALLERY_STEALTH_MUSIC_CLIP_ID, AUDIO_GALLERY_SWORD_HIT_CUE_ID,
        AUDIO_GALLERY_UI_NOTIFY_CUE_ID, AUDIO_GALLERY_VOICE_CLIP_ID,
    },
    navigation::{AppUiMode, game_panel_root, secondary_route_button_key},
    ui_ids::{OWNER_AUDIO_GALLERY, PANEL_AUDIO_GALLERY},
};

const AUDIO_GALLERY_SCOPE_ID: &str = "dev.audio_gallery";
const DEFAULT_LONG_SEEK_SECONDS: f32 = 8.0;
const DEFAULT_MUSIC_START_SECONDS: f32 = 12.0;
const DEFAULT_MUSIC_CROSSFADE_SECONDS: f32 = 1.5;

#[derive(Clone, Copy, Debug, Component, Eq, PartialEq)]
pub(super) enum AudioGalleryTextRow {
    Parameters,
    Instances,
    Status,
}

#[derive(Clone, Copy, Debug, Component, Eq, PartialEq)]
pub(super) enum AudioGalleryButton {
    PlaySfx(AudioGallerySfxCue),
    PlayLoop,
    PauseLoop,
    ResumeLoop,
    StopLoop,
    FadeOutLoop,
    PlayMusic(AudioGalleryMusicClip),
    PlayMusicFromStart(AudioGalleryMusicClip),
    PauseMusic,
    ResumeMusic,
    StopMusic,
    FadeOutMusic,
    QueryMusicProgress,
    CrossfadeMusic(AudioGalleryMusicClip, AudioGalleryMusicFadePreset),
    PlayClip,
    PlayLong,
    PauseRecent,
    ResumeRecent,
    StopRecent,
    SeekLong,
    QueryLongProgress,
    Volume(AudioGalleryVolumePreset),
    Pitch(AudioGalleryPitchPreset),
    ToggleLooped,
    FadeIn(AudioGalleryFadePreset),
    FadeOut(AudioGalleryFadePreset),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AudioGallerySfxCue {
    Notify,
    Footstep,
    SwordHit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AudioGalleryMusicClip {
    Menu,
    Stealth,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AudioGalleryMusicFadePreset {
    Instant,
    Smooth,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AudioGalleryVolumePreset {
    Soft,
    Normal,
    Loud,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AudioGalleryPitchPreset {
    Low,
    Normal,
    High,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AudioGalleryFadePreset {
    Off,
    Short,
    Long,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AudioGalleryInstanceSlot {
    Sfx,
    Loop,
    Clip,
    Long,
    Music,
    Spatial,
}

#[derive(Clone, Debug, PartialEq)]
enum AudioGalleryLaunchKind {
    Cue {
        cue_id: AudioCueId,
        slot: AudioGalleryInstanceSlot,
    },
    Clip {
        clip_id: AudioClipId,
        slot: AudioGalleryInstanceSlot,
    },
    Music {
        clip_id: AudioClipId,
        start_seconds: Option<f32>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AudioGalleryPlaybackParams {
    volume: f32,
    pitch: f32,
    looped: bool,
    fade_in_seconds: Option<f32>,
    fade_out_seconds: Option<f32>,
}

impl Default for AudioGalleryPlaybackParams {
    fn default() -> Self {
        Self {
            volume: 1.0,
            pitch: 1.0,
            looped: false,
            fade_in_seconds: None,
            fade_out_seconds: Some(0.5),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct AudioGalleryInstanceRecord {
    instance_id: Option<AudioInstanceId>,
    label: Option<String>,
    paused: bool,
    position_seconds: Option<f32>,
}

#[derive(Debug, Resource)]
pub(super) struct AudioGalleryState {
    frames_open: u64,
    params: AudioGalleryPlaybackParams,
    pending_launches: Vec<AudioGalleryLaunchKind>,
    last_sfx: AudioGalleryInstanceRecord,
    loop_instance: AudioGalleryInstanceRecord,
    clip_instance: AudioGalleryInstanceRecord,
    long_instance: AudioGalleryInstanceRecord,
    music_instance: AudioGalleryInstanceRecord,
    spatial_instance: AudioGalleryInstanceRecord,
    status: String,
}

impl AudioGalleryState {
    fn new() -> Self {
        Self {
            frames_open: 0,
            params: AudioGalleryPlaybackParams::default(),
            pending_launches: Vec::new(),
            last_sfx: AudioGalleryInstanceRecord::default(),
            loop_instance: AudioGalleryInstanceRecord::default(),
            clip_instance: AudioGalleryInstanceRecord::default(),
            long_instance: AudioGalleryInstanceRecord::default(),
            music_instance: AudioGalleryInstanceRecord::default(),
            spatial_instance: AudioGalleryInstanceRecord::default(),
            status: "Ready. Use the buttons to start dev audio samples.".to_string(),
        }
    }

    fn record_pending_launch(&mut self, launch: AudioGalleryLaunchKind) {
        self.pending_launches.push(launch);
    }

    fn record_started(
        &mut self,
        slot: AudioGalleryInstanceSlot,
        instance_id: AudioInstanceId,
        label: String,
    ) {
        let record = self.slot_record_mut(slot);
        record.instance_id = Some(instance_id);
        record.label = Some(label);
        record.paused = false;
        record.position_seconds = None;
    }

    fn mark_instance_paused(&mut self, instance_id: AudioInstanceId, paused: bool) {
        for record in self.records_mut() {
            if record.instance_id == Some(instance_id) {
                record.paused = paused;
            }
        }
    }

    fn clear_instance(&mut self, instance_id: AudioInstanceId) -> bool {
        let mut cleared = false;
        for record in self.records_mut() {
            if record.instance_id == Some(instance_id) {
                record.instance_id = None;
                record.paused = false;
                record.position_seconds = None;
                cleared = true;
            }
        }
        cleared
    }

    fn update_progress(&mut self, progress: &AudioInstanceProgress) -> bool {
        let Some(record) = self
            .records_mut()
            .into_iter()
            .find(|record| record.instance_id == Some(progress.instance_id))
        else {
            return false;
        };

        record.paused = progress.paused;
        record.position_seconds = Some(progress.position_seconds);
        true
    }

    fn recent_standard_instance(&self) -> Option<AudioInstanceId> {
        self.clip_instance
            .instance_id
            .or(self.last_sfx.instance_id)
            .or(self.loop_instance.instance_id)
    }

    fn long_instance_id(&self) -> Option<AudioInstanceId> {
        self.long_instance
            .instance_id
            .or(self.loop_instance.instance_id)
    }

    fn slot_record_mut(
        &mut self,
        slot: AudioGalleryInstanceSlot,
    ) -> &mut AudioGalleryInstanceRecord {
        match slot {
            AudioGalleryInstanceSlot::Sfx => &mut self.last_sfx,
            AudioGalleryInstanceSlot::Loop => &mut self.loop_instance,
            AudioGalleryInstanceSlot::Clip => &mut self.clip_instance,
            AudioGalleryInstanceSlot::Long => &mut self.long_instance,
            AudioGalleryInstanceSlot::Music => &mut self.music_instance,
            AudioGalleryInstanceSlot::Spatial => &mut self.spatial_instance,
        }
    }

    fn records_mut(&mut self) -> [&mut AudioGalleryInstanceRecord; 6] {
        [
            &mut self.last_sfx,
            &mut self.loop_instance,
            &mut self.clip_instance,
            &mut self.long_instance,
            &mut self.music_instance,
            &mut self.spatial_instance,
        ]
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
                            "audio_gallery.parameters.section",
                            "Playback Parameters",
                        ));
                        panel.spawn((
                            metric_label(
                                theme,
                                fonts,
                                audio_gallery_params_text(&AudioGalleryPlaybackParams::default()),
                            ),
                            AudioGalleryTextRow::Parameters,
                        ));
                        panel
                            .spawn(audio_gallery_grid(
                                metrics,
                                viewport.width_class,
                                audio_gallery_parameter_columns(),
                            ))
                            .with_children(|buttons| {
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::Volume(AudioGalleryVolumePreset::Soft),
                                    "audio_gallery.params.volume.soft",
                                    "Volume 50%",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::Volume(AudioGalleryVolumePreset::Normal),
                                    "audio_gallery.params.volume.normal",
                                    "Volume 100%",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::Volume(AudioGalleryVolumePreset::Loud),
                                    "audio_gallery.params.volume.loud",
                                    "Volume 140%",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::Pitch(AudioGalleryPitchPreset::Low),
                                    "audio_gallery.params.pitch.low",
                                    "Pitch 80%",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::Pitch(AudioGalleryPitchPreset::Normal),
                                    "audio_gallery.params.pitch.normal",
                                    "Pitch 100%",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::Pitch(AudioGalleryPitchPreset::High),
                                    "audio_gallery.params.pitch.high",
                                    "Pitch 120%",
                                );
                                spawn_looped_toggle(buttons, theme, fonts, i18n, false);
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::FadeIn(AudioGalleryFadePreset::Off),
                                    "audio_gallery.params.fade_in.off",
                                    "Fade In Off",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::FadeIn(AudioGalleryFadePreset::Short),
                                    "audio_gallery.params.fade_in.short",
                                    "Fade In 0.5s",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::FadeOut(AudioGalleryFadePreset::Off),
                                    "audio_gallery.params.fade_out.off",
                                    "Fade Out Off",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::FadeOut(AudioGalleryFadePreset::Short),
                                    "audio_gallery.params.fade_out.short",
                                    "Fade Out 0.5s",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::FadeOut(AudioGalleryFadePreset::Long),
                                    "audio_gallery.params.fade_out.long",
                                    "Fade Out 2s",
                                );
                            });
                    });

                body.spawn(audio_gallery_panel(theme))
                    .with_children(|panel| {
                        panel.spawn(section_label(
                            theme,
                            fonts,
                            i18n,
                            "audio_gallery.music.section",
                            "Music",
                        ));
                        panel
                            .spawn(audio_gallery_grid(
                                metrics,
                                viewport.width_class,
                                audio_gallery_button_columns(),
                            ))
                            .with_children(|buttons| {
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::PlayMusic(AudioGalleryMusicClip::Menu),
                                    "audio_gallery.music.play_menu",
                                    "Play Menu",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::PlayMusic(AudioGalleryMusicClip::Stealth),
                                    "audio_gallery.music.play_stealth",
                                    "Play Stealth",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::PlayMusicFromStart(
                                        AudioGalleryMusicClip::Menu,
                                    ),
                                    "audio_gallery.music.play_menu_start",
                                    "Menu From 12s",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::PauseMusic,
                                    "audio_gallery.music.pause",
                                    "Pause Music",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::ResumeMusic,
                                    "audio_gallery.music.resume",
                                    "Resume Music",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::StopMusic,
                                    "audio_gallery.music.stop",
                                    "Stop Music",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::FadeOutMusic,
                                    "audio_gallery.music.fade_stop",
                                    "Fade Stop Music",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::QueryMusicProgress,
                                    "audio_gallery.music.progress",
                                    "Music Progress",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::CrossfadeMusic(
                                        AudioGalleryMusicClip::Stealth,
                                        AudioGalleryMusicFadePreset::Instant,
                                    ),
                                    "audio_gallery.music.crossfade_0",
                                    "Crossfade 0s",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::CrossfadeMusic(
                                        AudioGalleryMusicClip::Menu,
                                        AudioGalleryMusicFadePreset::Smooth,
                                    ),
                                    "audio_gallery.music.crossfade_smooth",
                                    "Crossfade 1.5s",
                                );
                            });
                    });

                body.spawn(audio_gallery_panel(theme))
                    .with_children(|panel| {
                        panel.spawn(section_label(
                            theme,
                            fonts,
                            i18n,
                            "audio_gallery.sfx.section",
                            "SFX",
                        ));
                        panel
                            .spawn(audio_gallery_grid(
                                metrics,
                                viewport.width_class,
                                audio_gallery_button_columns(),
                            ))
                            .with_children(|buttons| {
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::PlaySfx(AudioGallerySfxCue::Notify),
                                    "audio_gallery.sfx.notify",
                                    "Notify",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::PlaySfx(AudioGallerySfxCue::Footstep),
                                    "audio_gallery.sfx.footstep",
                                    "Footstep",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::PlaySfx(AudioGallerySfxCue::SwordHit),
                                    "audio_gallery.sfx.sword_hit",
                                    "Sword Hit",
                                );
                            });
                    });

                body.spawn(audio_gallery_panel(theme))
                    .with_children(|panel| {
                        panel.spawn(section_label(
                            theme,
                            fonts,
                            i18n,
                            "audio_gallery.loop.section",
                            "Loop / Ambience",
                        ));
                        panel
                            .spawn(audio_gallery_grid(
                                metrics,
                                viewport.width_class,
                                audio_gallery_button_columns(),
                            ))
                            .with_children(|buttons| {
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::PlayLoop,
                                    "audio_gallery.loop.play",
                                    "Play Rain Loop",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::PauseLoop,
                                    "audio_gallery.loop.pause",
                                    "Pause Loop",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::ResumeLoop,
                                    "audio_gallery.loop.resume",
                                    "Resume Loop",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::StopLoop,
                                    "audio_gallery.loop.stop",
                                    "Stop Loop",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::FadeOutLoop,
                                    "audio_gallery.loop.fade_out",
                                    "Fade Stop Loop",
                                );
                            });
                    });

                body.spawn(audio_gallery_panel(theme))
                    .with_children(|panel| {
                        panel.spawn(section_label(
                            theme,
                            fonts,
                            i18n,
                            "audio_gallery.instances.section",
                            "Instance Controls",
                        ));
                        panel.spawn((
                            metric_label(
                                theme,
                                fonts,
                                audio_gallery_instances_text(&AudioGalleryState::new()),
                            ),
                            AudioGalleryTextRow::Instances,
                        ));
                        panel
                            .spawn(audio_gallery_grid(
                                metrics,
                                viewport.width_class,
                                audio_gallery_button_columns(),
                            ))
                            .with_children(|buttons| {
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::PlayClip,
                                    "audio_gallery.instances.play_clip",
                                    "Play Clip",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::PlayLong,
                                    "audio_gallery.instances.play_long",
                                    "Play Long Audio",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::PauseRecent,
                                    "audio_gallery.instances.pause",
                                    "Pause Recent",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::ResumeRecent,
                                    "audio_gallery.instances.resume",
                                    "Resume Recent",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::StopRecent,
                                    "audio_gallery.instances.stop",
                                    "Stop Recent",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::SeekLong,
                                    "audio_gallery.instances.seek",
                                    "Seek Long +8s",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::QueryLongProgress,
                                    "audio_gallery.instances.progress",
                                    "Query Progress",
                                );
                            });
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
                            metric_label(
                                theme,
                                fonts,
                                audio_gallery_status_text(&AudioGalleryState::new()),
                            ),
                            AudioGalleryTextRow::Status,
                        ));
                    });
            });
        });
}

pub(super) fn handle_audio_gallery_buttons(
    buttons: Query<&AudioGalleryButton>,
    mut button_events: MessageReader<UiButtonEvent>,
    mut state: ResMut<AudioGalleryState>,
    mut audio_commands: MessageWriter<AudioCommand>,
) {
    for event in button_events.read() {
        if event.kind != UiButtonEventKind::Click {
            continue;
        }

        let Ok(button) = buttons.get(event.entity) else {
            continue;
        };

        let outcome = apply_audio_gallery_button(&mut state, *button);
        state.status = outcome.status;
        for launch in outcome.launches {
            state.record_pending_launch(launch);
        }
        for command in outcome.commands {
            audio_commands.write(command);
        }
    }
}

pub(super) fn handle_audio_gallery_events(
    mut state: ResMut<AudioGalleryState>,
    mut audio_events: MessageReader<AudioEvent>,
) {
    for event in audio_events.read() {
        apply_audio_gallery_event(&mut state, event);
    }
}

pub(super) fn update_audio_gallery_status(
    mut state: ResMut<AudioGalleryState>,
    mut rows: Query<(&AudioGalleryTextRow, &mut Text)>,
) {
    state.frames_open = state.frames_open.saturating_add(1);

    for (row, mut text) in &mut rows {
        match row {
            AudioGalleryTextRow::Parameters => {
                text.0 = audio_gallery_params_text(&state.params);
            }
            AudioGalleryTextRow::Instances => {
                text.0 = audio_gallery_instances_text(&state);
            }
            AudioGalleryTextRow::Status => {
                text.0 = audio_gallery_status_text(&state);
            }
        }
    }
}

pub(super) fn cleanup_audio_gallery(
    mut commands: Commands,
    mut audio_commands: MessageWriter<AudioCommand>,
) {
    audio_commands.write(AudioCommand::StopByScope(AudioScopeFadeCommand {
        scope: audio_gallery_scope(),
        fade_out_seconds: Some(0.1),
    }));
    commands.remove_resource::<AudioGalleryState>();
}

fn apply_audio_gallery_button(
    state: &mut AudioGalleryState,
    button: AudioGalleryButton,
) -> AudioGalleryActionOutcome {
    let mut outcome = AudioGalleryActionOutcome::default();

    match button {
        AudioGalleryButton::PlaySfx(sfx) => {
            let cue_id = sfx.cue_id();
            let cue_label = sfx.label();
            outcome.launches.push(AudioGalleryLaunchKind::Cue {
                cue_id: cue_id.clone(),
                slot: AudioGalleryInstanceSlot::Sfx,
            });
            outcome
                .commands
                .push(AudioCommand::PlayCue(gallery_cue_request(
                    cue_id,
                    state.params,
                    false,
                    Some(AudioBus::Sfx),
                )));
            outcome.status = format!("Requested SFX: {cue_label}.");
        }
        AudioGalleryButton::PlayLoop => {
            let cue_id = cue_id(AUDIO_GALLERY_RAIN_LOOP_CUE_ID);
            outcome.launches.push(AudioGalleryLaunchKind::Cue {
                cue_id: cue_id.clone(),
                slot: AudioGalleryInstanceSlot::Loop,
            });
            outcome
                .commands
                .push(AudioCommand::PlayCue(gallery_cue_request(
                    cue_id,
                    state.params,
                    true,
                    Some(AudioBus::Sfx),
                )));
            outcome.status = "Requested light rain loop.".to_string();
        }
        AudioGalleryButton::PauseLoop => {
            if let Some(instance_id) = state.loop_instance.instance_id {
                outcome
                    .commands
                    .push(AudioCommand::PauseInstance(AudioInstanceCommand::new(
                        instance_id,
                    )));
                state.mark_instance_paused(instance_id, true);
                outcome.status = format!("Pause requested for loop instance {instance_id}.");
            } else {
                outcome.status = "No loop instance is active yet.".to_string();
            }
        }
        AudioGalleryButton::ResumeLoop => {
            if let Some(instance_id) = state.loop_instance.instance_id {
                outcome
                    .commands
                    .push(AudioCommand::ResumeInstance(AudioInstanceCommand::new(
                        instance_id,
                    )));
                state.mark_instance_paused(instance_id, false);
                outcome.status = format!("Resume requested for loop instance {instance_id}.");
            } else {
                outcome.status = "No loop instance is active yet.".to_string();
            }
        }
        AudioGalleryButton::StopLoop => {
            if let Some(instance_id) = state.loop_instance.instance_id {
                outcome
                    .commands
                    .push(AudioCommand::StopInstance(AudioStopInstanceCommand::new(
                        instance_id,
                    )));
                outcome.status = format!("Stop requested for loop instance {instance_id}.");
            } else {
                outcome.status = "No loop instance is active yet.".to_string();
            }
        }
        AudioGalleryButton::FadeOutLoop => {
            if let Some(instance_id) = state.loop_instance.instance_id {
                outcome
                    .commands
                    .push(AudioCommand::StopInstance(AudioStopInstanceCommand {
                        instance_id,
                        fade_out_seconds: state.params.fade_out_seconds.or(Some(0.5)),
                    }));
                outcome.status = format!(
                    "Fade-out stop requested for loop instance {instance_id} ({}).",
                    format_seconds(state.params.fade_out_seconds.or(Some(0.5)))
                );
            } else {
                outcome.status = "No loop instance is active yet.".to_string();
            }
        }
        AudioGalleryButton::PlayMusic(music_clip) => {
            let clip_id = music_clip.clip_id();
            outcome.launches.push(AudioGalleryLaunchKind::Music {
                clip_id: clip_id.clone(),
                start_seconds: None,
            });
            outcome
                .commands
                .push(AudioCommand::PlayMusic(gallery_music_request(
                    clip_id,
                    state.params,
                    None,
                )));
            outcome.status = format!("Requested music: {}.", music_clip.label());
        }
        AudioGalleryButton::PlayMusicFromStart(music_clip) => {
            let clip_id = music_clip.clip_id();
            outcome.launches.push(AudioGalleryLaunchKind::Music {
                clip_id: clip_id.clone(),
                start_seconds: Some(DEFAULT_MUSIC_START_SECONDS),
            });
            outcome
                .commands
                .push(AudioCommand::PlayMusic(gallery_music_request(
                    clip_id,
                    state.params,
                    Some(DEFAULT_MUSIC_START_SECONDS),
                )));
            outcome.status = format!(
                "Requested music {} from {DEFAULT_MUSIC_START_SECONDS:.1}s.",
                music_clip.label()
            );
        }
        AudioGalleryButton::PauseMusic => {
            outcome.commands.push(AudioCommand::PauseMusic);
            if let Some(instance_id) = state.music_instance.instance_id {
                state.mark_instance_paused(instance_id, true);
                outcome.status = format!("Pause requested for music instance {instance_id}.");
            } else {
                outcome.status =
                    "Pause requested for music; no gallery music instance is recorded yet."
                        .to_string();
            }
        }
        AudioGalleryButton::ResumeMusic => {
            outcome.commands.push(AudioCommand::ResumeMusic);
            if let Some(instance_id) = state.music_instance.instance_id {
                state.mark_instance_paused(instance_id, false);
                outcome.status = format!("Resume requested for music instance {instance_id}.");
            } else {
                outcome.status =
                    "Resume requested for music; no gallery music instance is recorded yet."
                        .to_string();
            }
        }
        AudioGalleryButton::StopMusic => {
            outcome
                .commands
                .push(AudioCommand::StopMusic(AudioMusicFadeCommand::new()));
            outcome.status = if let Some(instance_id) = state.music_instance.instance_id {
                format!("Immediate stop requested for music instance {instance_id}.")
            } else {
                "Immediate music stop requested.".to_string()
            };
        }
        AudioGalleryButton::FadeOutMusic => {
            let fade_out_seconds = state.params.fade_out_seconds.or(Some(0.5));
            outcome
                .commands
                .push(AudioCommand::StopMusic(AudioMusicFadeCommand {
                    fade_out_seconds,
                }));
            outcome.status = if let Some(instance_id) = state.music_instance.instance_id {
                format!(
                    "Fade-out stop requested for music instance {instance_id} ({}).",
                    format_seconds(fade_out_seconds)
                )
            } else {
                format!(
                    "Fade-out music stop requested ({}).",
                    format_seconds(fade_out_seconds)
                )
            };
        }
        AudioGalleryButton::QueryMusicProgress => {
            if let Some(instance_id) = state.music_instance.instance_id {
                outcome.commands.push(AudioCommand::QueryInstanceProgress(
                    AudioInstanceCommand::new(instance_id),
                ));
                outcome.status =
                    format!("Progress query requested for music instance {instance_id}.");
            } else {
                outcome.status = "No gallery music instance is active yet.".to_string();
            }
        }
        AudioGalleryButton::CrossfadeMusic(music_clip, fade_preset) => {
            let clip_id = music_clip.clip_id();
            let fade_seconds = fade_preset.seconds();
            outcome.launches.push(AudioGalleryLaunchKind::Music {
                clip_id: clip_id.clone(),
                start_seconds: None,
            });
            outcome
                .commands
                .push(AudioCommand::CrossfadeMusic(AudioCrossfadeMusicRequest {
                    clip_id,
                    scope: audio_gallery_scope(),
                    volume: state.params.volume,
                    looped: true,
                    fade_seconds,
                }));
            outcome.status = format!(
                "Crossfade requested to {} ({}).",
                music_clip.label(),
                format_seconds(Some(fade_seconds))
            );
        }
        AudioGalleryButton::PlayClip => {
            let clip_id = clip_id(AUDIO_GALLERY_VOICE_CLIP_ID);
            outcome.launches.push(AudioGalleryLaunchKind::Clip {
                clip_id: clip_id.clone(),
                slot: AudioGalleryInstanceSlot::Clip,
            });
            outcome
                .commands
                .push(AudioCommand::PlayClip(gallery_clip_request(
                    clip_id,
                    state.params,
                    state.params.looped,
                    AudioBus::Sfx,
                    None,
                )));
            outcome.status = "Requested normal clip sample.".to_string();
        }
        AudioGalleryButton::PlayLong => {
            let clip_id = clip_id(AUDIO_GALLERY_MENU_MUSIC_CLIP_ID);
            outcome.launches.push(AudioGalleryLaunchKind::Clip {
                clip_id: clip_id.clone(),
                slot: AudioGalleryInstanceSlot::Long,
            });
            outcome
                .commands
                .push(AudioCommand::PlayClip(gallery_clip_request(
                    clip_id,
                    state.params,
                    state.params.looped,
                    AudioBus::Sfx,
                    None,
                )));
            outcome.status = "Requested long audio clip for seek/progress testing.".to_string();
        }
        AudioGalleryButton::PauseRecent => {
            if let Some(instance_id) = state.recent_standard_instance() {
                outcome
                    .commands
                    .push(AudioCommand::PauseInstance(AudioInstanceCommand::new(
                        instance_id,
                    )));
                state.mark_instance_paused(instance_id, true);
                outcome.status = format!("Pause requested for recent instance {instance_id}.");
            } else {
                outcome.status = "No recent standard instance is active yet.".to_string();
            }
        }
        AudioGalleryButton::ResumeRecent => {
            if let Some(instance_id) = state.recent_standard_instance() {
                outcome
                    .commands
                    .push(AudioCommand::ResumeInstance(AudioInstanceCommand::new(
                        instance_id,
                    )));
                state.mark_instance_paused(instance_id, false);
                outcome.status = format!("Resume requested for recent instance {instance_id}.");
            } else {
                outcome.status = "No recent standard instance is active yet.".to_string();
            }
        }
        AudioGalleryButton::StopRecent => {
            if let Some(instance_id) = state.recent_standard_instance() {
                outcome
                    .commands
                    .push(AudioCommand::StopInstance(AudioStopInstanceCommand {
                        instance_id,
                        fade_out_seconds: state.params.fade_out_seconds,
                    }));
                outcome.status = format!(
                    "Stop requested for recent instance {instance_id} (fade-out {}).",
                    format_seconds(state.params.fade_out_seconds)
                );
            } else {
                outcome.status = "No recent standard instance is active yet.".to_string();
            }
        }
        AudioGalleryButton::SeekLong => {
            if let Some(instance_id) = state.long_instance_id() {
                outcome
                    .commands
                    .push(AudioCommand::SeekInstance(AudioSeekInstanceCommand::new(
                        instance_id,
                        DEFAULT_LONG_SEEK_SECONDS,
                    )));
                outcome.status = format!(
                    "Seek requested for long instance {instance_id} to {DEFAULT_LONG_SEEK_SECONDS:.1}s. If the sink is not ready or seek is unsupported, the failure will appear here."
                );
            } else {
                outcome.status =
                    "No long audio instance is active yet; play long audio or rain loop first."
                        .to_string();
            }
        }
        AudioGalleryButton::QueryLongProgress => {
            if let Some(instance_id) = state.long_instance_id() {
                outcome.commands.push(AudioCommand::QueryInstanceProgress(
                    AudioInstanceCommand::new(instance_id),
                ));
                outcome.status = format!("Progress query requested for instance {instance_id}.");
            } else {
                outcome.status =
                    "No long audio instance is active yet; play long audio or rain loop first."
                        .to_string();
            }
        }
        AudioGalleryButton::Volume(preset) => {
            state.params.volume = preset.value();
            outcome.status = format!("Volume set to {}.", preset.label());
        }
        AudioGalleryButton::Pitch(preset) => {
            state.params.pitch = preset.value();
            outcome.status = format!("Pitch set to {}.", preset.label());
        }
        AudioGalleryButton::ToggleLooped => {
            state.params.looped = !state.params.looped;
            outcome.status = if state.params.looped {
                "Looped playback enabled for new normal/long clips.".to_string()
            } else {
                "Looped playback disabled for new normal/long clips.".to_string()
            };
        }
        AudioGalleryButton::FadeIn(preset) => {
            state.params.fade_in_seconds = preset.seconds();
            outcome.status = format!("Fade-in set to {}.", format_seconds(preset.seconds()));
        }
        AudioGalleryButton::FadeOut(preset) => {
            state.params.fade_out_seconds = preset.seconds();
            outcome.status = format!("Fade-out set to {}.", format_seconds(preset.seconds()));
        }
    }

    outcome
}

fn apply_audio_gallery_event(state: &mut AudioGalleryState, event: &AudioEvent) {
    match event {
        AudioEvent::CueStarted(started) => {
            let Some(slot) = take_pending_cue_slot(state, &started.cue_id).or_else(|| {
                gallery_scope_matches(&started.scope)
                    .then(|| dev_cue_slot(&started.cue_id).unwrap_or(AudioGalleryInstanceSlot::Sfx))
            }) else {
                return;
            };

            state.record_started(slot, started.instance_id, started.cue_id.to_string());
            state.status = format!(
                "Started cue {} as instance {} on {} bus.",
                started.cue_id, started.instance_id, started.bus
            );
        }
        AudioEvent::ClipStarted(started) => {
            let Some(slot) = take_pending_clip_slot(state, &started.clip_id).or_else(|| {
                gallery_scope_matches(&started.scope).then(|| {
                    dev_clip_slot(&started.clip_id).unwrap_or(AudioGalleryInstanceSlot::Clip)
                })
            }) else {
                return;
            };

            state.record_started(slot, started.instance_id, started.clip_id.to_string());
            state.status = format!(
                "Started clip {} as instance {} on {} bus.",
                started.clip_id, started.instance_id, started.bus
            );
        }
        AudioEvent::MusicChanged(changed) => {
            apply_audio_gallery_music_changed(state, changed);
        }
        AudioEvent::InstanceStopped(stopped) => {
            if state.clear_instance(stopped.instance_id) {
                state.status = format!(
                    "Instance {} stopped: {:?}.",
                    stopped.instance_id, stopped.reason
                );
            }
        }
        AudioEvent::LoadFailed(failed) => {
            if load_failure_is_gallery_owned(state, failed) {
                state.status = audio_gallery_load_failed_text(failed);
            }
        }
        AudioEvent::InstanceProgress(progress) => {
            if state.update_progress(progress) {
                state.status = format!(
                    "Instance {} progress: {:.2}s, paused={}, spatial={}.",
                    progress.instance_id,
                    progress.position_seconds,
                    yes_no(progress.paused),
                    yes_no(progress.spatial)
                );
            }
        }
        AudioEvent::InstanceControlFailed(failed) => {
            if instance_failure_is_gallery_owned(state, failed) {
                state.status = audio_gallery_control_failed_text(failed);
                if failed.reason == AudioInstanceControlFailureReason::MissingInstance {
                    state.clear_instance(failed.instance_id);
                }
            }
        }
        _ => {}
    }
}

#[derive(Default)]
struct AudioGalleryActionOutcome {
    commands: Vec<AudioCommand>,
    launches: Vec<AudioGalleryLaunchKind>,
    status: String,
}

fn gallery_cue_request(
    cue_id: AudioCueId,
    params: AudioGalleryPlaybackParams,
    force_looped: bool,
    bus: Option<AudioBus>,
) -> AudioCueRequest {
    AudioCueRequest {
        cue_id,
        scope: audio_gallery_scope(),
        bus,
        volume: params.volume,
        pitch: params.pitch,
        looped: force_looped || params.looped,
        fade_in_seconds: params.fade_in_seconds,
        start_seconds: None,
    }
}

fn gallery_clip_request(
    clip_id: AudioClipId,
    params: AudioGalleryPlaybackParams,
    looped: bool,
    bus: AudioBus,
    start_seconds: Option<f32>,
) -> AudioClipRequest {
    AudioClipRequest {
        clip_id,
        scope: audio_gallery_scope(),
        bus,
        volume: params.volume,
        pitch: params.pitch,
        looped,
        fade_in_seconds: params.fade_in_seconds,
        start_seconds,
    }
}

fn gallery_music_request(
    clip_id: AudioClipId,
    params: AudioGalleryPlaybackParams,
    start_seconds: Option<f32>,
) -> AudioMusicRequest {
    AudioMusicRequest {
        clip_id,
        scope: audio_gallery_scope(),
        volume: params.volume,
        looped: true,
        fade_in_seconds: params.fade_in_seconds,
        start_seconds,
    }
}

fn apply_audio_gallery_music_changed(state: &mut AudioGalleryState, changed: &AudioMusicChanged) {
    if !gallery_scope_matches(&changed.scope) {
        return;
    }

    let pending_start = take_pending_music_start(state, &changed.new_clip_id);
    if let Some(instance_id) = changed.new_instance_id {
        state.record_started(
            AudioGalleryInstanceSlot::Music,
            instance_id,
            changed.new_clip_id.to_string(),
        );
        state.music_instance.position_seconds = pending_start.flatten();
        state.status = format!(
            "Music changed to {} as instance {} (crossfade {}).",
            changed.new_clip_id,
            instance_id,
            format_seconds(changed.crossfade_seconds)
        );
    } else {
        state.music_instance.label = Some(changed.new_clip_id.to_string());
        state.music_instance.position_seconds = pending_start.flatten();
        state.status = format!(
            "Music changed to {} but no instance was reported.",
            changed.new_clip_id
        );
    }
}

fn take_pending_cue_slot(
    state: &mut AudioGalleryState,
    cue_id: &AudioCueId,
) -> Option<AudioGalleryInstanceSlot> {
    let index = state
        .pending_launches
        .iter()
        .position(|launch| matches!(launch, AudioGalleryLaunchKind::Cue { cue_id: pending, .. } if pending == cue_id))?;
    match state.pending_launches.remove(index) {
        AudioGalleryLaunchKind::Cue { slot, .. } => Some(slot),
        AudioGalleryLaunchKind::Clip { .. } | AudioGalleryLaunchKind::Music { .. } => None,
    }
}

fn take_pending_clip_slot(
    state: &mut AudioGalleryState,
    clip_id: &AudioClipId,
) -> Option<AudioGalleryInstanceSlot> {
    let index = state
        .pending_launches
        .iter()
        .position(|launch| matches!(launch, AudioGalleryLaunchKind::Clip { clip_id: pending, .. } if pending == clip_id))?;
    match state.pending_launches.remove(index) {
        AudioGalleryLaunchKind::Clip { slot, .. } => Some(slot),
        AudioGalleryLaunchKind::Cue { .. } | AudioGalleryLaunchKind::Music { .. } => None,
    }
}

fn take_pending_music_start(
    state: &mut AudioGalleryState,
    clip_id: &AudioClipId,
) -> Option<Option<f32>> {
    let index = state.pending_launches.iter().position(|launch| {
        matches!(launch, AudioGalleryLaunchKind::Music { clip_id: pending, .. } if pending == clip_id)
    })?;
    match state.pending_launches.remove(index) {
        AudioGalleryLaunchKind::Music { start_seconds, .. } => Some(start_seconds),
        AudioGalleryLaunchKind::Cue { .. } | AudioGalleryLaunchKind::Clip { .. } => None,
    }
}

fn load_failure_is_gallery_owned(state: &AudioGalleryState, failed: &AudioLoadFailed) -> bool {
    failed
        .cue_id
        .as_ref()
        .is_some_and(|cue_id| dev_cue_slot(cue_id).is_some())
        || failed
            .clip_id
            .as_ref()
            .is_some_and(|clip_id| dev_clip_slot(clip_id).is_some())
        || failed
            .group_id
            .as_ref()
            .is_some_and(|group_id| group_id.as_str() == "bank.audio_gallery")
        || state.pending_launches.iter().any(|launch| match launch {
            AudioGalleryLaunchKind::Cue { cue_id, .. } => failed.cue_id.as_ref() == Some(cue_id),
            AudioGalleryLaunchKind::Clip { clip_id, .. } => {
                failed.clip_id.as_ref() == Some(clip_id)
            }
            AudioGalleryLaunchKind::Music { clip_id, .. } => {
                failed.clip_id.as_ref() == Some(clip_id)
            }
        })
}

fn instance_failure_is_gallery_owned(
    state: &AudioGalleryState,
    failed: &AudioInstanceControlFailed,
) -> bool {
    state
        .records()
        .into_iter()
        .any(|record| record.instance_id == Some(failed.instance_id))
}

fn dev_cue_slot(cue_id: &AudioCueId) -> Option<AudioGalleryInstanceSlot> {
    match cue_id.as_str() {
        AUDIO_GALLERY_UI_NOTIFY_CUE_ID
        | AUDIO_GALLERY_FOOTSTEP_CUE_ID
        | AUDIO_GALLERY_SWORD_HIT_CUE_ID => Some(AudioGalleryInstanceSlot::Sfx),
        AUDIO_GALLERY_RAIN_LOOP_CUE_ID => Some(AudioGalleryInstanceSlot::Loop),
        AUDIO_GALLERY_CAR_HORN_CUE_ID => Some(AudioGalleryInstanceSlot::Spatial),
        value if value.starts_with("dev.audio.music.") => Some(AudioGalleryInstanceSlot::Music),
        _ => None,
    }
}

fn dev_clip_slot(clip_id: &AudioClipId) -> Option<AudioGalleryInstanceSlot> {
    match clip_id.as_str() {
        AUDIO_GALLERY_VOICE_CLIP_ID => Some(AudioGalleryInstanceSlot::Clip),
        value if value.starts_with("dev.audio.music.") => Some(AudioGalleryInstanceSlot::Music),
        value if value.starts_with("dev.audio.spatial.") => Some(AudioGalleryInstanceSlot::Spatial),
        value if value.starts_with("dev.audio.") => Some(AudioGalleryInstanceSlot::Clip),
        _ => None,
    }
}

fn gallery_scope_matches(scope: &AudioScope) -> bool {
    scope == &audio_gallery_scope()
}

fn audio_gallery_scope() -> AudioScope {
    AudioScope::scene(AUDIO_GALLERY_SCOPE_ID).expect("audio gallery scope id must be valid")
}

fn audio_gallery_params_text(params: &AudioGalleryPlaybackParams) -> String {
    format!(
        "volume {:.0}% | pitch {:.0}% | looped {} | fade in {} | fade out {}",
        params.volume * 100.0,
        params.pitch * 100.0,
        yes_no(params.looped),
        format_seconds(params.fade_in_seconds),
        format_seconds(params.fade_out_seconds)
    )
}

fn audio_gallery_instances_text(state: &AudioGalleryState) -> String {
    format!(
        "sfx {} | loop {} | music {} | clip {} | long {}",
        record_text(&state.last_sfx),
        record_text(&state.loop_instance),
        record_text(&state.music_instance),
        record_text(&state.clip_instance),
        record_text(&state.long_instance)
    )
}

fn audio_gallery_status_text(state: &AudioGalleryState) -> String {
    format!("{} Frames open: {}", state.status, state.frames_open)
}

fn record_text(record: &AudioGalleryInstanceRecord) -> String {
    let Some(instance_id) = record.instance_id else {
        return "none".to_string();
    };

    let label = record.label.as_deref().unwrap_or("unknown");
    let paused = if record.paused { ", paused" } else { "" };
    let progress = record
        .position_seconds
        .map(|seconds| format!(", {:.2}s", seconds))
        .unwrap_or_default();
    format!("#{instance_id} {label}{paused}{progress}")
}

fn audio_gallery_load_failed_text(failed: &AudioLoadFailed) -> String {
    let id = failed
        .cue_id
        .as_ref()
        .map(|id| format!("cue {id}"))
        .or_else(|| failed.clip_id.as_ref().map(|id| format!("clip {id}")))
        .or_else(|| failed.group_id.as_ref().map(|id| format!("group {id}")))
        .unwrap_or_else(|| "gallery audio".to_string());
    format!("Load failed for {id}: {}", failed.message)
}

fn audio_gallery_control_failed_text(failed: &AudioInstanceControlFailed) -> String {
    format!(
        "{} failed for instance {}: {} ({:?}).",
        control_action_label(failed.action),
        failed.instance_id,
        readable_control_failure_reason(failed.reason),
        failed.reason
    )
}

fn control_action_label(action: AudioInstanceControlAction) -> &'static str {
    match action {
        AudioInstanceControlAction::Pause => "Pause",
        AudioInstanceControlAction::Resume => "Resume",
        AudioInstanceControlAction::Seek => "Seek",
        AudioInstanceControlAction::QueryProgress => "Progress query",
    }
}

fn readable_control_failure_reason(reason: AudioInstanceControlFailureReason) -> &'static str {
    match reason {
        AudioInstanceControlFailureReason::MissingInstance => "the instance is no longer active",
        AudioInstanceControlFailureReason::StoppedInstance => "the instance is already stopping",
        AudioInstanceControlFailureReason::SinkNotReady => "the audio sink is not ready yet",
        AudioInstanceControlFailureReason::SeekUnsupported => {
            "this source or platform does not support seek"
        }
        AudioInstanceControlFailureReason::InvalidPosition => "the seek position is invalid",
    }
}

fn format_seconds(seconds: Option<f32>) -> String {
    seconds
        .filter(|seconds| *seconds > 0.0)
        .map(|seconds| format!("{seconds:.1}s"))
        .unwrap_or_else(|| "off".to_string())
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

impl AudioGalleryState {
    fn records(&self) -> [&AudioGalleryInstanceRecord; 6] {
        [
            &self.last_sfx,
            &self.loop_instance,
            &self.clip_instance,
            &self.long_instance,
            &self.music_instance,
            &self.spatial_instance,
        ]
    }
}

impl AudioGallerySfxCue {
    fn cue_id(self) -> AudioCueId {
        cue_id(match self {
            Self::Notify => AUDIO_GALLERY_UI_NOTIFY_CUE_ID,
            Self::Footstep => AUDIO_GALLERY_FOOTSTEP_CUE_ID,
            Self::SwordHit => AUDIO_GALLERY_SWORD_HIT_CUE_ID,
        })
    }

    fn label(self) -> &'static str {
        match self {
            Self::Notify => "notify",
            Self::Footstep => "footstep",
            Self::SwordHit => "sword hit",
        }
    }
}

impl AudioGalleryMusicClip {
    fn clip_id(self) -> AudioClipId {
        clip_id(match self {
            Self::Menu => AUDIO_GALLERY_MENU_MUSIC_CLIP_ID,
            Self::Stealth => AUDIO_GALLERY_STEALTH_MUSIC_CLIP_ID,
        })
    }

    fn label(self) -> &'static str {
        match self {
            Self::Menu => "menu_loop",
            Self::Stealth => "stealth_bass_loop",
        }
    }
}

impl AudioGalleryMusicFadePreset {
    fn seconds(self) -> f32 {
        match self {
            Self::Instant => 0.0,
            Self::Smooth => DEFAULT_MUSIC_CROSSFADE_SECONDS,
        }
    }
}

impl AudioGalleryVolumePreset {
    fn value(self) -> f32 {
        match self {
            Self::Soft => 0.5,
            Self::Normal => 1.0,
            Self::Loud => 1.4,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Soft => "50%",
            Self::Normal => "100%",
            Self::Loud => "140%",
        }
    }
}

impl AudioGalleryPitchPreset {
    fn value(self) -> f32 {
        match self {
            Self::Low => 0.8,
            Self::Normal => 1.0,
            Self::High => 1.2,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Low => "80%",
            Self::Normal => "100%",
            Self::High => "120%",
        }
    }
}

impl AudioGalleryFadePreset {
    fn seconds(self) -> Option<f32> {
        match self {
            Self::Off => None,
            Self::Short => Some(0.5),
            Self::Long => Some(2.0),
        }
    }
}

impl fmt::Display for AudioGalleryInstanceSlot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Sfx => "sfx",
            Self::Loop => "loop",
            Self::Clip => "clip",
            Self::Long => "long",
            Self::Music => "music",
            Self::Spatial => "spatial",
        })
    }
}

fn spawn_gallery_button(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    button: AudioGalleryButton,
    key: &'static str,
    fallback: &'static str,
) {
    parent.spawn((
        secondary_action_button_key(theme, metrics, fonts, i18n, key, fallback),
        button,
    ));
}

fn spawn_looped_toggle(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    looped: bool,
) {
    let mut toggle = if looped {
        parent.spawn(toggle_on_key(
            theme,
            fonts,
            i18n,
            "audio_gallery.params.looped",
            "Looped",
        ))
    } else {
        parent.spawn(toggle_key(
            theme,
            fonts,
            i18n,
            "audio_gallery.params.looped",
            "Looped",
        ))
    };
    toggle.insert(AudioGalleryButton::ToggleLooped);
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

fn audio_gallery_grid(
    metrics: &UiMetrics,
    width_class: UiWidthClass,
    columns: UiResponsiveGridColumns,
) -> impl Bundle {
    ui_responsive_grid(metrics, width_class, columns)
}

fn audio_gallery_button_columns() -> UiResponsiveGridColumns {
    UiResponsiveGridColumns::new(1, 2, 4)
}

fn audio_gallery_parameter_columns() -> UiResponsiveGridColumns {
    UiResponsiveGridColumns::new(1, 3, 4)
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

fn cue_id(value: &str) -> AudioCueId {
    AudioCueId::try_from(value).expect("audio gallery cue id must be valid")
}

fn clip_id(value: &str) -> AudioClipId {
    AudioClipId::try_from(value).expect("audio gallery clip id must be valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::audio::prelude::{
        AudioClipStarted, AudioCueStarted, AudioInstanceStopped, AudioLoadFailed, AudioStopReason,
        DEFAULT_UI_CLICK_CUE_ID,
    };
    use crate::framework::ui::widgets::{UiButtonEvent, UiButtonEventKind};
    use bevy::ecs::message::MessageCursor;

    fn read_audio_commands(app: &App) -> Vec<AudioCommand> {
        let messages = app.world().resource::<Messages<AudioCommand>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    #[test]
    fn sfx_buttons_map_to_dev_cue_commands_with_current_params() {
        let mut state = AudioGalleryState::new();
        state.params.volume = 0.5;
        state.params.pitch = 1.2;
        state.params.fade_in_seconds = Some(0.5);

        let outcome = apply_audio_gallery_button(
            &mut state,
            AudioGalleryButton::PlaySfx(AudioGallerySfxCue::SwordHit),
        );

        assert_eq!(
            outcome.commands,
            vec![AudioCommand::PlayCue(AudioCueRequest {
                cue_id: cue_id(AUDIO_GALLERY_SWORD_HIT_CUE_ID),
                scope: audio_gallery_scope(),
                bus: Some(AudioBus::Sfx),
                volume: 0.5,
                pitch: 1.2,
                looped: false,
                fade_in_seconds: Some(0.5),
                start_seconds: None,
            })]
        );
        assert_eq!(
            outcome.launches,
            vec![AudioGalleryLaunchKind::Cue {
                cue_id: cue_id(AUDIO_GALLERY_SWORD_HIT_CUE_ID),
                slot: AudioGalleryInstanceSlot::Sfx,
            }]
        );
    }

    #[test]
    fn loop_controls_map_to_instance_commands() {
        let mut state = AudioGalleryState::new();
        let instance_id = AudioInstanceId::from_raw(55);
        state.record_started(
            AudioGalleryInstanceSlot::Loop,
            instance_id,
            AUDIO_GALLERY_RAIN_LOOP_CUE_ID.to_string(),
        );
        state.params.fade_out_seconds = Some(2.0);

        let pause = apply_audio_gallery_button(&mut state, AudioGalleryButton::PauseLoop);
        let resume = apply_audio_gallery_button(&mut state, AudioGalleryButton::ResumeLoop);
        let fade = apply_audio_gallery_button(&mut state, AudioGalleryButton::FadeOutLoop);

        assert_eq!(
            pause.commands,
            vec![AudioCommand::PauseInstance(AudioInstanceCommand::new(
                instance_id,
            ))]
        );
        assert_eq!(
            resume.commands,
            vec![AudioCommand::ResumeInstance(AudioInstanceCommand::new(
                instance_id,
            ))]
        );
        assert_eq!(
            fade.commands,
            vec![AudioCommand::StopInstance(AudioStopInstanceCommand {
                instance_id,
                fade_out_seconds: Some(2.0),
            })]
        );
    }

    #[test]
    fn music_play_buttons_map_to_music_commands() {
        let mut state = AudioGalleryState::new();
        state.params.volume = 0.5;
        state.params.fade_in_seconds = Some(0.5);

        let menu = apply_audio_gallery_button(
            &mut state,
            AudioGalleryButton::PlayMusic(AudioGalleryMusicClip::Menu),
        );
        let stealth = apply_audio_gallery_button(
            &mut state,
            AudioGalleryButton::PlayMusic(AudioGalleryMusicClip::Stealth),
        );
        let start = apply_audio_gallery_button(
            &mut state,
            AudioGalleryButton::PlayMusicFromStart(AudioGalleryMusicClip::Menu),
        );

        assert_eq!(
            menu.commands,
            vec![AudioCommand::PlayMusic(AudioMusicRequest {
                clip_id: clip_id(AUDIO_GALLERY_MENU_MUSIC_CLIP_ID),
                scope: audio_gallery_scope(),
                volume: 0.5,
                looped: true,
                fade_in_seconds: Some(0.5),
                start_seconds: None,
            })]
        );
        assert_eq!(
            stealth.commands,
            vec![AudioCommand::PlayMusic(AudioMusicRequest {
                clip_id: clip_id(AUDIO_GALLERY_STEALTH_MUSIC_CLIP_ID),
                scope: audio_gallery_scope(),
                volume: 0.5,
                looped: true,
                fade_in_seconds: Some(0.5),
                start_seconds: None,
            })]
        );
        assert_eq!(
            start.commands,
            vec![AudioCommand::PlayMusic(AudioMusicRequest {
                clip_id: clip_id(AUDIO_GALLERY_MENU_MUSIC_CLIP_ID),
                scope: audio_gallery_scope(),
                volume: 0.5,
                looped: true,
                fade_in_seconds: Some(0.5),
                start_seconds: Some(DEFAULT_MUSIC_START_SECONDS),
            })]
        );
        assert_eq!(
            start.launches,
            vec![AudioGalleryLaunchKind::Music {
                clip_id: clip_id(AUDIO_GALLERY_MENU_MUSIC_CLIP_ID),
                start_seconds: Some(DEFAULT_MUSIC_START_SECONDS),
            }]
        );
    }

    #[test]
    fn music_controls_map_to_music_commands_with_stop_and_crossfade_fades() {
        let mut state = AudioGalleryState::new();
        let instance_id = AudioInstanceId::from_raw(66);
        state.record_started(
            AudioGalleryInstanceSlot::Music,
            instance_id,
            AUDIO_GALLERY_MENU_MUSIC_CLIP_ID.to_string(),
        );
        state.params.volume = 0.5;
        state.params.fade_out_seconds = Some(2.0);

        let pause = apply_audio_gallery_button(&mut state, AudioGalleryButton::PauseMusic);
        assert_eq!(pause.commands, vec![AudioCommand::PauseMusic]);
        assert!(state.music_instance.paused);

        let resume = apply_audio_gallery_button(&mut state, AudioGalleryButton::ResumeMusic);
        assert_eq!(resume.commands, vec![AudioCommand::ResumeMusic]);
        assert!(!state.music_instance.paused);

        let progress =
            apply_audio_gallery_button(&mut state, AudioGalleryButton::QueryMusicProgress);
        let stop = apply_audio_gallery_button(&mut state, AudioGalleryButton::StopMusic);
        let fade_stop = apply_audio_gallery_button(&mut state, AudioGalleryButton::FadeOutMusic);
        let crossfade_zero = apply_audio_gallery_button(
            &mut state,
            AudioGalleryButton::CrossfadeMusic(
                AudioGalleryMusicClip::Stealth,
                AudioGalleryMusicFadePreset::Instant,
            ),
        );
        let crossfade_smooth = apply_audio_gallery_button(
            &mut state,
            AudioGalleryButton::CrossfadeMusic(
                AudioGalleryMusicClip::Menu,
                AudioGalleryMusicFadePreset::Smooth,
            ),
        );

        assert_eq!(
            progress.commands,
            vec![AudioCommand::QueryInstanceProgress(
                AudioInstanceCommand::new(instance_id)
            )]
        );
        assert_eq!(
            stop.commands,
            vec![AudioCommand::StopMusic(AudioMusicFadeCommand::new())]
        );
        assert_eq!(
            fade_stop.commands,
            vec![AudioCommand::StopMusic(AudioMusicFadeCommand {
                fade_out_seconds: Some(2.0),
            })]
        );
        assert_eq!(
            crossfade_zero.commands,
            vec![AudioCommand::CrossfadeMusic(AudioCrossfadeMusicRequest {
                clip_id: clip_id(AUDIO_GALLERY_STEALTH_MUSIC_CLIP_ID),
                scope: audio_gallery_scope(),
                volume: 0.5,
                looped: true,
                fade_seconds: 0.0,
            })]
        );
        assert_eq!(
            crossfade_smooth.commands,
            vec![AudioCommand::CrossfadeMusic(AudioCrossfadeMusicRequest {
                clip_id: clip_id(AUDIO_GALLERY_MENU_MUSIC_CLIP_ID),
                scope: audio_gallery_scope(),
                volume: 0.5,
                looped: true,
                fade_seconds: DEFAULT_MUSIC_CROSSFADE_SECONDS,
            })]
        );
    }

    #[test]
    fn long_controls_seek_and_query_recent_long_instance() {
        let mut state = AudioGalleryState::new();
        let instance_id = AudioInstanceId::from_raw(77);
        state.record_started(
            AudioGalleryInstanceSlot::Long,
            instance_id,
            AUDIO_GALLERY_MENU_MUSIC_CLIP_ID.to_string(),
        );

        let seek = apply_audio_gallery_button(&mut state, AudioGalleryButton::SeekLong);
        let query = apply_audio_gallery_button(&mut state, AudioGalleryButton::QueryLongProgress);

        assert_eq!(
            seek.commands,
            vec![AudioCommand::SeekInstance(AudioSeekInstanceCommand::new(
                instance_id,
                DEFAULT_LONG_SEEK_SECONDS,
            ))]
        );
        assert_eq!(
            query.commands,
            vec![AudioCommand::QueryInstanceProgress(
                AudioInstanceCommand::new(instance_id)
            )]
        );
    }

    #[test]
    fn play_long_uses_music_loop_clip_as_plain_instance_for_seek_testing() {
        let mut state = AudioGalleryState::new();

        let outcome = apply_audio_gallery_button(&mut state, AudioGalleryButton::PlayLong);

        assert_eq!(
            outcome.commands,
            vec![AudioCommand::PlayClip(AudioClipRequest {
                clip_id: clip_id(AUDIO_GALLERY_MENU_MUSIC_CLIP_ID),
                scope: audio_gallery_scope(),
                bus: AudioBus::Sfx,
                volume: 1.0,
                pitch: 1.0,
                looped: false,
                fade_in_seconds: None,
                start_seconds: None,
            })]
        );
        assert_eq!(
            outcome.launches,
            vec![AudioGalleryLaunchKind::Clip {
                clip_id: clip_id(AUDIO_GALLERY_MENU_MUSIC_CLIP_ID),
                slot: AudioGalleryInstanceSlot::Long,
            }]
        );
    }

    #[test]
    fn parameter_buttons_update_future_requests() {
        let mut state = AudioGalleryState::new();

        apply_audio_gallery_button(
            &mut state,
            AudioGalleryButton::Volume(AudioGalleryVolumePreset::Soft),
        );
        apply_audio_gallery_button(
            &mut state,
            AudioGalleryButton::Pitch(AudioGalleryPitchPreset::High),
        );
        apply_audio_gallery_button(&mut state, AudioGalleryButton::ToggleLooped);
        apply_audio_gallery_button(
            &mut state,
            AudioGalleryButton::FadeIn(AudioGalleryFadePreset::Short),
        );
        apply_audio_gallery_button(
            &mut state,
            AudioGalleryButton::FadeOut(AudioGalleryFadePreset::Long),
        );

        let outcome = apply_audio_gallery_button(&mut state, AudioGalleryButton::PlayClip);

        assert_eq!(state.params.volume, 0.5);
        assert_eq!(state.params.pitch, 1.2);
        assert!(state.params.looped);
        assert_eq!(state.params.fade_in_seconds, Some(0.5));
        assert_eq!(state.params.fade_out_seconds, Some(2.0));
        assert_eq!(
            outcome.commands,
            vec![AudioCommand::PlayClip(AudioClipRequest {
                clip_id: clip_id(AUDIO_GALLERY_VOICE_CLIP_ID),
                scope: audio_gallery_scope(),
                bus: AudioBus::Sfx,
                volume: 0.5,
                pitch: 1.2,
                looped: true,
                fade_in_seconds: Some(0.5),
                start_seconds: None,
            })]
        );
    }

    #[test]
    fn button_event_system_writes_audio_commands_and_pending_launch() {
        let mut app = App::new();
        app.add_message::<UiButtonEvent>()
            .add_message::<AudioCommand>()
            .insert_resource(AudioGalleryState::new())
            .add_systems(Update, handle_audio_gallery_buttons);

        let button = app
            .world_mut()
            .spawn(AudioGalleryButton::PlaySfx(AudioGallerySfxCue::Notify))
            .id();
        app.world_mut().write_message(UiButtonEvent {
            entity: button,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.update();

        assert_eq!(
            read_audio_commands(&app),
            vec![AudioCommand::PlayCue(AudioCueRequest {
                cue_id: cue_id(AUDIO_GALLERY_UI_NOTIFY_CUE_ID),
                scope: audio_gallery_scope(),
                bus: Some(AudioBus::Sfx),
                volume: 1.0,
                pitch: 1.0,
                looped: false,
                fade_in_seconds: None,
                start_seconds: None,
            })]
        );
        assert_eq!(
            app.world().resource::<AudioGalleryState>().pending_launches,
            vec![AudioGalleryLaunchKind::Cue {
                cue_id: cue_id(AUDIO_GALLERY_UI_NOTIFY_CUE_ID),
                slot: AudioGalleryInstanceSlot::Sfx,
            }]
        );
    }

    #[test]
    fn button_event_system_writes_music_command_and_pending_launch() {
        let mut app = App::new();
        app.add_message::<UiButtonEvent>()
            .add_message::<AudioCommand>()
            .insert_resource(AudioGalleryState::new())
            .add_systems(Update, handle_audio_gallery_buttons);

        let button = app
            .world_mut()
            .spawn(AudioGalleryButton::PlayMusic(
                AudioGalleryMusicClip::Stealth,
            ))
            .id();
        app.world_mut().write_message(UiButtonEvent {
            entity: button,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.update();

        assert_eq!(
            read_audio_commands(&app),
            vec![AudioCommand::PlayMusic(AudioMusicRequest {
                clip_id: clip_id(AUDIO_GALLERY_STEALTH_MUSIC_CLIP_ID),
                scope: audio_gallery_scope(),
                volume: 1.0,
                looped: true,
                fade_in_seconds: None,
                start_seconds: None,
            })]
        );
        assert_eq!(
            app.world().resource::<AudioGalleryState>().pending_launches,
            vec![AudioGalleryLaunchKind::Music {
                clip_id: clip_id(AUDIO_GALLERY_STEALTH_MUSIC_CLIP_ID),
                start_seconds: None,
            }]
        );
    }

    #[test]
    fn started_event_filter_ignores_default_ui_click() {
        let mut state = AudioGalleryState::new();
        let ui_click = cue_id(DEFAULT_UI_CLICK_CUE_ID);

        apply_audio_gallery_event(
            &mut state,
            &AudioEvent::CueStarted(AudioCueStarted {
                cue_id: ui_click,
                clip_id: clip_id("ui.click_wood_01"),
                instance_id: AudioInstanceId::from_raw(1),
                scope: AudioScope::Ui,
                bus: AudioBus::Ui,
            }),
        );

        assert_eq!(state.last_sfx.instance_id, None);
        assert!(state.status.starts_with("Ready."));
    }

    #[test]
    fn pending_dev_cue_started_event_records_sfx_instance() {
        let mut state = AudioGalleryState::new();
        state.record_pending_launch(AudioGalleryLaunchKind::Cue {
            cue_id: cue_id(AUDIO_GALLERY_FOOTSTEP_CUE_ID),
            slot: AudioGalleryInstanceSlot::Sfx,
        });

        apply_audio_gallery_event(
            &mut state,
            &AudioEvent::CueStarted(AudioCueStarted {
                cue_id: cue_id(AUDIO_GALLERY_FOOTSTEP_CUE_ID),
                clip_id: clip_id("dev.audio.common.footstep_concrete_01"),
                instance_id: AudioInstanceId::from_raw(2),
                scope: audio_gallery_scope(),
                bus: AudioBus::Sfx,
            }),
        );

        assert_eq!(
            state.last_sfx.instance_id,
            Some(AudioInstanceId::from_raw(2))
        );
        assert!(state.pending_launches.is_empty());
    }

    #[test]
    fn dev_scope_clip_started_event_records_clip_even_without_pending_launch() {
        let mut state = AudioGalleryState::new();

        apply_audio_gallery_event(
            &mut state,
            &AudioEvent::ClipStarted(AudioClipStarted {
                clip_id: clip_id(AUDIO_GALLERY_VOICE_CLIP_ID),
                instance_id: AudioInstanceId::from_raw(3),
                scope: audio_gallery_scope(),
                bus: AudioBus::Sfx,
            }),
        );

        assert_eq!(
            state.clip_instance.instance_id,
            Some(AudioInstanceId::from_raw(3))
        );
    }

    #[test]
    fn music_changed_event_records_music_instance_and_start_progress() {
        let mut state = AudioGalleryState::new();
        state.record_pending_launch(AudioGalleryLaunchKind::Music {
            clip_id: clip_id(AUDIO_GALLERY_MENU_MUSIC_CLIP_ID),
            start_seconds: Some(DEFAULT_MUSIC_START_SECONDS),
        });

        apply_audio_gallery_event(
            &mut state,
            &AudioEvent::MusicChanged(AudioMusicChanged {
                previous_instance_id: None,
                previous_clip_id: None,
                new_instance_id: Some(AudioInstanceId::from_raw(8)),
                new_clip_id: clip_id(AUDIO_GALLERY_MENU_MUSIC_CLIP_ID),
                scope: audio_gallery_scope(),
                crossfade_seconds: None,
            }),
        );

        assert_eq!(
            state.music_instance.instance_id,
            Some(AudioInstanceId::from_raw(8))
        );
        assert_eq!(
            state.music_instance.label.as_deref(),
            Some(AUDIO_GALLERY_MENU_MUSIC_CLIP_ID)
        );
        assert_eq!(
            state.music_instance.position_seconds,
            Some(DEFAULT_MUSIC_START_SECONDS)
        );
        assert!(state.pending_launches.is_empty());
        assert!(audio_gallery_instances_text(&state).contains("music #8"));
        assert!(audio_gallery_instances_text(&state).contains("12.00s"));
    }

    #[test]
    fn music_progress_and_stopped_events_update_music_record() {
        let mut state = AudioGalleryState::new();
        let instance_id = AudioInstanceId::from_raw(9);
        state.record_started(
            AudioGalleryInstanceSlot::Music,
            instance_id,
            AUDIO_GALLERY_STEALTH_MUSIC_CLIP_ID.to_string(),
        );

        apply_audio_gallery_event(
            &mut state,
            &AudioEvent::InstanceProgress(AudioInstanceProgress {
                instance_id,
                clip_id: clip_id(AUDIO_GALLERY_STEALTH_MUSIC_CLIP_ID),
                cue_id: None,
                scope: audio_gallery_scope(),
                bus: AudioBus::Music,
                position_seconds: 18.5,
                paused: true,
                spatial: false,
            }),
        );
        assert_eq!(state.music_instance.position_seconds, Some(18.5));
        assert!(state.music_instance.paused);

        apply_audio_gallery_event(
            &mut state,
            &AudioEvent::InstanceStopped(AudioInstanceStopped {
                instance_id,
                clip_id: Some(clip_id(AUDIO_GALLERY_STEALTH_MUSIC_CLIP_ID)),
                cue_id: None,
                scope: audio_gallery_scope(),
                bus: AudioBus::Music,
                reason: AudioStopReason::Stopped,
            }),
        );
        assert_eq!(state.music_instance.instance_id, None);
        assert!(!state.music_instance.paused);
        assert_eq!(state.music_instance.position_seconds, None);
    }

    #[test]
    fn stop_progress_and_failure_events_update_gallery_state() {
        let mut state = AudioGalleryState::new();
        let instance_id = AudioInstanceId::from_raw(4);
        state.record_started(
            AudioGalleryInstanceSlot::Long,
            instance_id,
            AUDIO_GALLERY_MENU_MUSIC_CLIP_ID.to_string(),
        );

        apply_audio_gallery_event(
            &mut state,
            &AudioEvent::InstanceProgress(AudioInstanceProgress {
                instance_id,
                clip_id: clip_id(AUDIO_GALLERY_MENU_MUSIC_CLIP_ID),
                cue_id: None,
                scope: audio_gallery_scope(),
                bus: AudioBus::Sfx,
                position_seconds: 5.25,
                paused: true,
                spatial: false,
            }),
        );
        assert_eq!(state.long_instance.position_seconds, Some(5.25));
        assert!(state.long_instance.paused);

        apply_audio_gallery_event(
            &mut state,
            &AudioEvent::InstanceControlFailed(AudioInstanceControlFailed {
                instance_id,
                action: AudioInstanceControlAction::Seek,
                reason: AudioInstanceControlFailureReason::SeekUnsupported,
                message: "try_seek failed".to_string(),
            }),
        );
        assert!(state.status.contains("does not support seek"));
        assert_eq!(state.long_instance.instance_id, Some(instance_id));

        apply_audio_gallery_event(
            &mut state,
            &AudioEvent::InstanceStopped(AudioInstanceStopped {
                instance_id,
                clip_id: Some(clip_id(AUDIO_GALLERY_MENU_MUSIC_CLIP_ID)),
                cue_id: None,
                scope: audio_gallery_scope(),
                bus: AudioBus::Sfx,
                reason: AudioStopReason::Stopped,
            }),
        );
        assert_eq!(state.long_instance.instance_id, None);
        assert!(state.status.contains("stopped"));
    }

    #[test]
    fn missing_instance_failure_clears_stale_record() {
        let mut state = AudioGalleryState::new();
        let instance_id = AudioInstanceId::from_raw(5);
        state.record_started(
            AudioGalleryInstanceSlot::Clip,
            instance_id,
            AUDIO_GALLERY_VOICE_CLIP_ID.to_string(),
        );

        apply_audio_gallery_event(
            &mut state,
            &AudioEvent::InstanceControlFailed(AudioInstanceControlFailed {
                instance_id,
                action: AudioInstanceControlAction::QueryProgress,
                reason: AudioInstanceControlFailureReason::MissingInstance,
                message: "audio instance is not active".to_string(),
            }),
        );

        assert_eq!(state.clip_instance.instance_id, None);
        assert!(state.status.contains("no longer active"));
    }

    #[test]
    fn load_failed_for_dev_clip_updates_status() {
        let mut state = AudioGalleryState::new();

        apply_audio_gallery_event(
            &mut state,
            &AudioEvent::LoadFailed(AudioLoadFailed {
                clip_id: Some(clip_id(AUDIO_GALLERY_VOICE_CLIP_ID)),
                cue_id: None,
                group_id: None,
                asset_path: Some("audio/voice/en_us_una_hs_lo_01.wav".to_string()),
                message: "load failed".to_string(),
            }),
        );

        assert!(state.status.contains("Load failed"));
        assert!(state.status.contains(AUDIO_GALLERY_VOICE_CLIP_ID));
    }

    #[test]
    fn cleanup_sends_stop_by_gallery_scope_and_removes_state() {
        let mut app = App::new();
        app.add_message::<AudioCommand>()
            .insert_resource(AudioGalleryState::new())
            .add_systems(Update, cleanup_audio_gallery);

        app.update();

        assert!(!app.world().contains_resource::<AudioGalleryState>());
        assert_eq!(
            read_audio_commands(&app),
            vec![AudioCommand::StopByScope(AudioScopeFadeCommand {
                scope: audio_gallery_scope(),
                fade_out_seconds: Some(0.1),
            })]
        );
    }
}
