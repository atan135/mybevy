use std::fmt;

use bevy::prelude::*;

use crate::framework::{
    audio::prelude::{
        AudioBankGroupState, AudioBankLoadStatus, AudioBankRuntime, AudioBus, AudioBusChange,
        AudioBusChanged, AudioBusMutedCommand, AudioBusPausedCommand, AudioBusState,
        AudioBusVolumeCommand, AudioClipId, AudioClipRequest, AudioCommand,
        AudioCrossfadeMusicRequest, AudioCueId, AudioCueRequest, AudioCueSkipped, AudioDebugConfig,
        AudioEvent, AudioGroupCommand, AudioGroupId, AudioInstanceCommand,
        AudioInstanceControlAction, AudioInstanceControlFailed, AudioInstanceControlFailureReason,
        AudioInstanceId, AudioInstanceProgress, AudioLoadFailed, AudioLoadProgress, AudioMixer,
        AudioMusicChanged, AudioMusicFadeCommand, AudioMusicRequest, AudioScope,
        AudioScopeFadeCommand, AudioSeekInstanceCommand, AudioSpatialAttenuation,
        AudioSpatialCueRequest, AudioSpatialListenerBinding, AudioSpatialListenerEntity,
        AudioSpatialSource, AudioStopInstanceCommand, BEVY_SPATIAL_AUDIO_LIMITS,
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
        AUDIO_GALLERY_BANK_GROUP_ID, AUDIO_GALLERY_CAR_HORN_CUE_ID, AUDIO_GALLERY_COOLDOWN_CUE_ID,
        AUDIO_GALLERY_DOG_BARK_CUE_ID, AUDIO_GALLERY_FOOTSTEP_CUE_ID,
        AUDIO_GALLERY_MAX_CONCURRENT_CUE_ID, AUDIO_GALLERY_MENU_MUSIC_CLIP_ID,
        AUDIO_GALLERY_MISSING_CLIP_ID, AUDIO_GALLERY_MISSING_CUE_ID,
        AUDIO_GALLERY_RAIN_LOOP_CUE_ID, AUDIO_GALLERY_RESIDENT_BANK_GROUP_ID,
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
const AUDIO_GALLERY_STATUS_VALUE_LIMIT: usize = 96;
const AUDIO_GALLERY_RECORD_LABEL_LIMIT: usize = 40;
const AUDIO_GALLERY_ASSET_LABEL_LIMIT: usize = 48;
const AUDIO_GALLERY_BUSES: [AudioBus; 5] = [
    AudioBus::Master,
    AudioBus::Music,
    AudioBus::Sfx,
    AudioBus::Ui,
    AudioBus::Battle,
];

#[derive(Clone, Copy, Debug, Component, Eq, PartialEq)]
pub(super) enum AudioGalleryTextRow {
    Parameters,
    Mixer,
    Loading,
    Diagnostics,
    Spatial,
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
    PlaySpatialFixed(AudioGallerySpatialFixedPosition),
    PlaySpatialFollowEmitter,
    MoveSpatialEmitter,
    MoveSpatialListener,
    SpatialAttenuation(AudioGallerySpatialAttenuationPreset),
    QuerySpatialProgress,
    StopSpatial,
    BusVolume(AudioBus, AudioGalleryBusVolumePreset),
    ToggleMasterMute,
    ToggleBusMute(AudioBus),
    PauseBus(AudioBus),
    ResumeBus(AudioBus),
    PreloadGalleryBank,
    UnloadGalleryBank,
    PlayCooldownRuleCue,
    PlayMaxConcurrentRuleCue,
    PlayMissingCue,
    PlayMissingClip,
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
pub(super) enum AudioGallerySpatialFixedPosition {
    Left,
    Right,
    Near,
    Far,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AudioGallerySpatialAttenuationPreset {
    Close,
    Wide,
    Steep,
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
pub(super) enum AudioGalleryBusVolumePreset {
    Low,
    Full,
}

#[derive(Clone, Copy, Debug, Component, Eq, PartialEq)]
pub(super) struct AudioGallerySpatialHelper {
    kind: AudioGallerySpatialHelperKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AudioGallerySpatialHelperKind {
    ListenerTarget,
    EmitterTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AudioGallerySpatialListenerPosition {
    Center,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AudioGallerySpatialEmitterPosition {
    Left,
    Right,
    Near,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AudioGallerySpatialSourceKind {
    Fixed(AudioGallerySpatialFixedPosition),
    FollowEmitter,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct AudioGallerySpatialDetails {
    source_kind: AudioGallerySpatialSourceKind,
    position: Vec3,
    listener_position: Vec3,
    distance: f32,
    attenuation: AudioSpatialAttenuation,
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
    spatial: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct AudioGalleryDiagnosticsState {
    last_started_cue: Option<String>,
    last_skipped_cue: Option<String>,
    last_load_failed: Option<String>,
    last_loading_progress: Option<AudioLoadProgress>,
}

#[derive(Debug, Resource)]
pub(super) struct AudioGalleryState {
    frames_open: u64,
    params: AudioGalleryPlaybackParams,
    spatial_listener_target: Option<Entity>,
    spatial_emitter_target: Option<Entity>,
    spatial_listener_position: AudioGallerySpatialListenerPosition,
    spatial_emitter_position: AudioGallerySpatialEmitterPosition,
    spatial_attenuation: AudioGallerySpatialAttenuationPreset,
    spatial_details: Option<AudioGallerySpatialDetails>,
    pending_launches: Vec<AudioGalleryLaunchKind>,
    last_sfx: AudioGalleryInstanceRecord,
    loop_instance: AudioGalleryInstanceRecord,
    clip_instance: AudioGalleryInstanceRecord,
    long_instance: AudioGalleryInstanceRecord,
    music_instance: AudioGalleryInstanceRecord,
    spatial_instance: AudioGalleryInstanceRecord,
    buses: [(AudioBus, AudioBusState); 5],
    diagnostics: AudioGalleryDiagnosticsState,
    status: String,
}

impl AudioGalleryState {
    fn new() -> Self {
        Self {
            frames_open: 0,
            params: AudioGalleryPlaybackParams::default(),
            spatial_listener_target: None,
            spatial_emitter_target: None,
            spatial_listener_position: AudioGallerySpatialListenerPosition::Center,
            spatial_emitter_position: AudioGallerySpatialEmitterPosition::Left,
            spatial_attenuation: AudioGallerySpatialAttenuationPreset::Wide,
            spatial_details: None,
            pending_launches: Vec::new(),
            last_sfx: AudioGalleryInstanceRecord::default(),
            loop_instance: AudioGalleryInstanceRecord::default(),
            clip_instance: AudioGalleryInstanceRecord::default(),
            long_instance: AudioGalleryInstanceRecord::default(),
            music_instance: AudioGalleryInstanceRecord::default(),
            spatial_instance: AudioGalleryInstanceRecord::default(),
            buses: default_audio_gallery_bus_states(),
            diagnostics: AudioGalleryDiagnosticsState::default(),
            status: "Ready. Audio debug capture is enabled for this page.".to_string(),
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
        record.spatial = slot == AudioGalleryInstanceSlot::Spatial;
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
                record.spatial = false;
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
        record.spatial = progress.spatial;
        true
    }

    fn update_spatial_details(
        &mut self,
        source_kind: AudioGallerySpatialSourceKind,
        position: Vec3,
    ) {
        let listener_position = self.spatial_listener_position.position();
        self.spatial_details = Some(AudioGallerySpatialDetails {
            source_kind,
            position,
            listener_position,
            distance: listener_position.distance(position),
            attenuation: self.spatial_attenuation.value(),
        });
    }

    fn refresh_spatial_distance(&mut self) {
        if let Some(mut details) = self.spatial_details {
            details.listener_position = self.spatial_listener_position.position();
            details.distance = details.listener_position.distance(details.position);
            details.attenuation = self.spatial_attenuation.value();
            self.spatial_details = Some(details);
        }
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

    fn bus_state(&self, bus: AudioBus) -> AudioBusState {
        self.buses
            .iter()
            .find_map(|(entry_bus, state)| (*entry_bus == bus).then_some(*state))
            .unwrap_or_default()
    }

    fn set_bus_state(&mut self, bus: AudioBus, state: AudioBusState) {
        if let Some((_, entry_state)) = self
            .buses
            .iter_mut()
            .find(|(entry_bus, _)| *entry_bus == bus)
        {
            *entry_state = state;
        }
    }

    fn update_bus_change(&mut self, changed: &AudioBusChanged) {
        let mut state = self.bus_state(changed.bus);
        match changed.change {
            AudioBusChange::Volume { current, .. } => state.volume = current,
            AudioBusChange::Muted { current, .. } => state.muted = current,
            AudioBusChange::Paused { current, .. } => state.paused = current,
        }
        self.set_bus_state(changed.bus, state);
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
    mixer: Option<Res<AudioMixer>>,
) {
    let theme = theme.into_inner();
    let metrics = metrics.into_inner();
    let viewport = viewport.into_inner();
    let fonts = fonts.into_inner();
    let i18n = i18n.into_inner();
    clear_color.0 = theme.colors.screen_background;
    setup_audio_gallery_state_and_spatial_helpers(&mut commands, mixer.as_deref());

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
                            "audio_gallery.mixer_loading.section",
                            "Mixer / Loading",
                        ));
                        panel.spawn((
                            metric_label(
                                theme,
                                fonts,
                                audio_gallery_mixer_text(&AudioGalleryState::new()),
                            ),
                            AudioGalleryTextRow::Mixer,
                        ));
                        panel.spawn((
                            metric_label(
                                theme,
                                fonts,
                                audio_gallery_loading_text(&AudioGalleryState::new(), None),
                            ),
                            AudioGalleryTextRow::Loading,
                        ));
                        panel
                            .spawn(audio_gallery_grid(
                                metrics,
                                viewport.width_class,
                                audio_gallery_button_columns(),
                            ))
                            .with_children(|buttons| {
                                for bus in AUDIO_GALLERY_BUSES {
                                    spawn_gallery_button(
                                        buttons,
                                        theme,
                                        metrics,
                                        fonts,
                                        i18n,
                                        AudioGalleryButton::BusVolume(
                                            bus,
                                            AudioGalleryBusVolumePreset::Low,
                                        ),
                                        bus_action_i18n_key(bus, AudioGalleryBusAction::VolumeLow),
                                        bus_action_fallback(bus, AudioGalleryBusAction::VolumeLow),
                                    );
                                    spawn_gallery_button(
                                        buttons,
                                        theme,
                                        metrics,
                                        fonts,
                                        i18n,
                                        AudioGalleryButton::BusVolume(
                                            bus,
                                            AudioGalleryBusVolumePreset::Full,
                                        ),
                                        bus_action_i18n_key(bus, AudioGalleryBusAction::VolumeFull),
                                        bus_action_fallback(bus, AudioGalleryBusAction::VolumeFull),
                                    );
                                }
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::ToggleMasterMute,
                                    "audio_gallery.bus.master_mute",
                                    "Master Mute",
                                );
                                for bus in [
                                    AudioBus::Music,
                                    AudioBus::Sfx,
                                    AudioBus::Ui,
                                    AudioBus::Battle,
                                ] {
                                    spawn_gallery_button(
                                        buttons,
                                        theme,
                                        metrics,
                                        fonts,
                                        i18n,
                                        AudioGalleryButton::ToggleBusMute(bus),
                                        bus_action_i18n_key(bus, AudioGalleryBusAction::Mute),
                                        bus_action_fallback(bus, AudioGalleryBusAction::Mute),
                                    );
                                    spawn_gallery_button(
                                        buttons,
                                        theme,
                                        metrics,
                                        fonts,
                                        i18n,
                                        AudioGalleryButton::PauseBus(bus),
                                        bus_action_i18n_key(bus, AudioGalleryBusAction::Pause),
                                        bus_action_fallback(bus, AudioGalleryBusAction::Pause),
                                    );
                                    spawn_gallery_button(
                                        buttons,
                                        theme,
                                        metrics,
                                        fonts,
                                        i18n,
                                        AudioGalleryButton::ResumeBus(bus),
                                        bus_action_i18n_key(bus, AudioGalleryBusAction::Resume),
                                        bus_action_fallback(bus, AudioGalleryBusAction::Resume),
                                    );
                                }
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::PreloadGalleryBank,
                                    "audio_gallery.bank.preload",
                                    "Preload Bank",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::UnloadGalleryBank,
                                    "audio_gallery.bank.unload",
                                    "Unload Bank",
                                );
                            });
                    });

                body.spawn(audio_gallery_panel(theme))
                    .with_children(|panel| {
                        panel.spawn(section_label(
                            theme,
                            fonts,
                            i18n,
                            "audio_gallery.rules.section",
                            "Rules / Stress",
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
                                    AudioGalleryButton::PlayCooldownRuleCue,
                                    "audio_gallery.rules.cooldown",
                                    "Cooldown Cue",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::PlayMaxConcurrentRuleCue,
                                    "audio_gallery.rules.max_concurrent",
                                    "Max Concurrent Cue",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::PlayMissingCue,
                                    "audio_gallery.failure.missing_cue",
                                    "Missing Cue",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::PlayMissingClip,
                                    "audio_gallery.failure.missing_clip",
                                    "Missing Clip",
                                );
                            });
                    });

                body.spawn(audio_gallery_panel(theme))
                    .with_children(|panel| {
                        panel.spawn(section_label(
                            theme,
                            fonts,
                            i18n,
                            "audio_gallery.spatial.section",
                            "Spatial",
                        ));
                        panel.spawn((
                            metric_label(
                                theme,
                                fonts,
                                audio_gallery_spatial_text(&AudioGalleryState::new()),
                            ),
                            AudioGalleryTextRow::Spatial,
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
                                    AudioGalleryButton::PlaySpatialFixed(
                                        AudioGallerySpatialFixedPosition::Left,
                                    ),
                                    "audio_gallery.spatial.left",
                                    "Play Left",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::PlaySpatialFixed(
                                        AudioGallerySpatialFixedPosition::Right,
                                    ),
                                    "audio_gallery.spatial.right",
                                    "Play Right",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::PlaySpatialFixed(
                                        AudioGallerySpatialFixedPosition::Near,
                                    ),
                                    "audio_gallery.spatial.near",
                                    "Play Near",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::PlaySpatialFixed(
                                        AudioGallerySpatialFixedPosition::Far,
                                    ),
                                    "audio_gallery.spatial.far",
                                    "Play Far",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::PlaySpatialFollowEmitter,
                                    "audio_gallery.spatial.follow",
                                    "Follow Emitter",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::MoveSpatialEmitter,
                                    "audio_gallery.spatial.move_emitter",
                                    "Move Emitter",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::MoveSpatialListener,
                                    "audio_gallery.spatial.move_listener",
                                    "Move Listener",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::SpatialAttenuation(
                                        AudioGallerySpatialAttenuationPreset::Close,
                                    ),
                                    "audio_gallery.spatial.atten_close",
                                    "Atten Close",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::SpatialAttenuation(
                                        AudioGallerySpatialAttenuationPreset::Wide,
                                    ),
                                    "audio_gallery.spatial.atten_wide",
                                    "Atten Wide",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::SpatialAttenuation(
                                        AudioGallerySpatialAttenuationPreset::Steep,
                                    ),
                                    "audio_gallery.spatial.atten_steep",
                                    "Atten Steep",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::QuerySpatialProgress,
                                    "audio_gallery.spatial.progress",
                                    "Spatial Progress",
                                );
                                spawn_gallery_button(
                                    buttons,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    AudioGalleryButton::StopSpatial,
                                    "audio_gallery.spatial.stop",
                                    "Stop Spatial",
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
                        panel.spawn((
                            metric_label(
                                theme,
                                fonts,
                                audio_gallery_diagnostics_text(&AudioGalleryState::new()),
                            ),
                            AudioGalleryTextRow::Diagnostics,
                        ));
                        panel
                            .spawn(audio_gallery_grid(
                                metrics,
                                viewport.width_class,
                                audio_gallery_button_columns(),
                            ))
                            .with_children(|buttons| {
                                buttons.spawn(secondary_route_button_key(
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    "nav.audio_monitor",
                                    "Open Audio Monitor",
                                    AppUiMode::AudioMonitor,
                                ));
                            });
                    });
            });
        });
}

pub(super) fn handle_audio_gallery_buttons(
    buttons: Query<&AudioGalleryButton>,
    mut button_events: MessageReader<UiButtonEvent>,
    mut state: ResMut<AudioGalleryState>,
    mut audio_commands: MessageWriter<AudioCommand>,
    mut helpers: Query<(
        &AudioGallerySpatialHelper,
        &mut Transform,
        Option<&mut GlobalTransform>,
    )>,
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
        apply_audio_gallery_spatial_helper_transforms(&state, &mut helpers);
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
    bank: Option<Res<AudioBankRuntime>>,
) {
    state.frames_open = state.frames_open.saturating_add(1);

    for (row, mut text) in &mut rows {
        match row {
            AudioGalleryTextRow::Parameters => {
                text.0 = audio_gallery_params_text(&state.params);
            }
            AudioGalleryTextRow::Mixer => {
                text.0 = audio_gallery_mixer_text(&state);
            }
            AudioGalleryTextRow::Loading => {
                text.0 = audio_gallery_loading_text(&state, bank.as_deref());
            }
            AudioGalleryTextRow::Diagnostics => {
                text.0 = audio_gallery_diagnostics_text(&state);
            }
            AudioGalleryTextRow::Spatial => {
                text.0 = audio_gallery_spatial_text(&state);
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
    mut bank: Option<ResMut<AudioBankRuntime>>,
    state: Option<Res<AudioGalleryState>>,
    listener_entity: Option<Res<AudioSpatialListenerEntity>>,
) {
    let gallery_bank_group_id = group_id(AUDIO_GALLERY_BANK_GROUP_ID);
    let unload_gallery_bank = bank
        .as_deref()
        .and_then(|bank| bank.groups.get(&gallery_bank_group_id))
        .is_some_and(|state| !state.resident());
    audio_commands.write(AudioCommand::StopByScope(AudioScopeFadeCommand {
        scope: audio_gallery_scope(),
        fade_out_seconds: None,
    }));
    if unload_gallery_bank {
        audio_commands.write(AudioCommand::UnloadGroup(AudioGroupCommand::new(
            gallery_bank_group_id.clone(),
        )));
    }
    if let Some(state) = state {
        if let Some(entity) = state.spatial_listener_target {
            commands.entity(entity).try_despawn();
        }
        if let Some(entity) = state.spatial_emitter_target {
            commands.entity(entity).try_despawn();
        }
    }
    if let Some(listener_entity) = listener_entity {
        commands.entity(listener_entity.0).try_despawn();
        commands.remove_resource::<AudioSpatialListenerEntity>();
    }
    commands.remove_resource::<AudioSpatialListenerBinding>();
    commands.remove_resource::<AudioGalleryState>();
    if let Some(bank) = bank.as_deref_mut() {
        bank.clear_transient_group_runtime(&gallery_bank_group_id);
    }
}

pub(super) fn enable_audio_gallery_debug(mut config: ResMut<AudioDebugConfig>) {
    config.enabled = true;
}

fn setup_audio_gallery_state_and_spatial_helpers(
    commands: &mut Commands,
    mixer: Option<&AudioMixer>,
) {
    let (listener_target, emitter_target) = spawn_audio_gallery_spatial_helpers(commands);
    let mut state = AudioGalleryState::new();
    if let Some(mixer) = mixer {
        state.buses = default_audio_gallery_bus_states_from_mixer(mixer);
    }
    state.spatial_listener_target = Some(listener_target);
    state.spatial_emitter_target = Some(emitter_target);
    commands.insert_resource(AudioSpatialListenerBinding::new(listener_target));
    commands.insert_resource(state);
}

#[cfg(test)]
fn setup_audio_gallery_state_and_spatial_helpers_system(mut commands: Commands) {
    setup_audio_gallery_state_and_spatial_helpers(&mut commands, None);
}

fn spawn_audio_gallery_spatial_helpers(commands: &mut Commands) -> (Entity, Entity) {
    let listener_target = commands
        .spawn((
            AudioGallerySpatialHelper {
                kind: AudioGallerySpatialHelperKind::ListenerTarget,
            },
            Transform::from_translation(AudioGallerySpatialListenerPosition::Center.position()),
            GlobalTransform::from_translation(
                AudioGallerySpatialListenerPosition::Center.position(),
            ),
            Name::new("AudioGallerySpatialListenerTarget"),
        ))
        .id();
    let emitter_target = commands
        .spawn((
            AudioGallerySpatialHelper {
                kind: AudioGallerySpatialHelperKind::EmitterTarget,
            },
            Transform::from_translation(AudioGallerySpatialEmitterPosition::Left.position()),
            GlobalTransform::from_translation(AudioGallerySpatialEmitterPosition::Left.position()),
            Name::new("AudioGallerySpatialEmitterTarget"),
        ))
        .id();

    (listener_target, emitter_target)
}

fn apply_audio_gallery_spatial_helper_transforms(
    state: &AudioGalleryState,
    helpers: &mut Query<(
        &AudioGallerySpatialHelper,
        &mut Transform,
        Option<&mut GlobalTransform>,
    )>,
) {
    for (helper, mut transform, global_transform) in helpers {
        let position = match helper.kind {
            AudioGallerySpatialHelperKind::ListenerTarget => {
                state.spatial_listener_position.position()
            }
            AudioGallerySpatialHelperKind::EmitterTarget => {
                state.spatial_emitter_position.position()
            }
        };
        *transform = Transform::from_translation(position);
        if let Some(mut global_transform) = global_transform {
            *global_transform = GlobalTransform::from_translation(position);
        }
    }
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
        AudioGalleryButton::PlaySpatialFixed(position) => {
            let cue_id = cue_id(AUDIO_GALLERY_CAR_HORN_CUE_ID);
            let source_position = position.position();
            state.update_spatial_details(
                AudioGallerySpatialSourceKind::Fixed(position),
                source_position,
            );
            outcome.launches.push(AudioGalleryLaunchKind::Cue {
                cue_id: cue_id.clone(),
                slot: AudioGalleryInstanceSlot::Spatial,
            });
            outcome
                .commands
                .push(AudioCommand::PlaySpatialCue(gallery_spatial_cue_request(
                    cue_id,
                    state.params,
                    AudioSpatialSource::fixed(Transform::from_translation(source_position)),
                    state.spatial_attenuation.value(),
                )));
            outcome.status = format!(
                "Requested fixed spatial cue at {} with {} attenuation. {}",
                position.label(),
                state.spatial_attenuation.label(),
                BEVY_SPATIAL_AUDIO_LIMITS
            );
        }
        AudioGalleryButton::PlaySpatialFollowEmitter => {
            let Some(emitter_target) = state.spatial_emitter_target else {
                outcome.status =
                    "Spatial emitter helper is missing; re-enter Audio Gallery.".to_string();
                return outcome;
            };
            let cue_id = cue_id(AUDIO_GALLERY_DOG_BARK_CUE_ID);
            let source_position = state.spatial_emitter_position.position();
            state.update_spatial_details(
                AudioGallerySpatialSourceKind::FollowEmitter,
                source_position,
            );
            outcome.launches.push(AudioGalleryLaunchKind::Cue {
                cue_id: cue_id.clone(),
                slot: AudioGalleryInstanceSlot::Spatial,
            });
            outcome
                .commands
                .push(AudioCommand::PlaySpatialCue(gallery_spatial_cue_request(
                    cue_id,
                    state.params,
                    AudioSpatialSource::follow_entity(emitter_target),
                    state.spatial_attenuation.value(),
                )));
            outcome.status = format!(
                "Requested follow-emitter spatial cue at {}. Move Emitter updates the target entity. {}",
                state.spatial_emitter_position.label(),
                BEVY_SPATIAL_AUDIO_LIMITS
            );
        }
        AudioGalleryButton::MoveSpatialEmitter => {
            state.spatial_emitter_position = state.spatial_emitter_position.next();
            if matches!(
                state.spatial_details.map(|details| details.source_kind),
                Some(AudioGallerySpatialSourceKind::FollowEmitter)
            ) {
                state.update_spatial_details(
                    AudioGallerySpatialSourceKind::FollowEmitter,
                    state.spatial_emitter_position.position(),
                );
            }
            outcome.status = format!(
                "Moved spatial emitter helper to {}.",
                state.spatial_emitter_position.label()
            );
        }
        AudioGalleryButton::MoveSpatialListener => {
            state.spatial_listener_position = state.spatial_listener_position.next();
            state.refresh_spatial_distance();
            outcome.status = format!(
                "Moved spatial listener helper to {}.",
                state.spatial_listener_position.label()
            );
        }
        AudioGalleryButton::SpatialAttenuation(preset) => {
            state.spatial_attenuation = preset;
            state.refresh_spatial_distance();
            outcome.status = format!(
                "Spatial attenuation set to {} (max {:.0}, rolloff {:.1}).",
                preset.label(),
                preset.value().max_distance,
                preset.value().rolloff_factor
            );
        }
        AudioGalleryButton::QuerySpatialProgress => {
            if let Some(instance_id) = state.spatial_instance.instance_id {
                outcome.commands.push(AudioCommand::QueryInstanceProgress(
                    AudioInstanceCommand::new(instance_id),
                ));
                outcome.status =
                    format!("Progress query requested for spatial instance {instance_id}.");
            } else {
                outcome.status = "No spatial instance is active yet.".to_string();
            }
        }
        AudioGalleryButton::StopSpatial => {
            if let Some(instance_id) = state.spatial_instance.instance_id {
                outcome
                    .commands
                    .push(AudioCommand::StopInstance(AudioStopInstanceCommand {
                        instance_id,
                        fade_out_seconds: state.params.fade_out_seconds,
                    }));
                outcome.status = format!(
                    "Stop requested for spatial instance {instance_id} (fade-out {}).",
                    format_seconds(state.params.fade_out_seconds)
                );
            } else {
                outcome.status = "No spatial instance is active yet.".to_string();
            }
        }
        AudioGalleryButton::BusVolume(bus, preset) => {
            let volume = preset.value();
            let mut state_for_bus = state.bus_state(bus);
            state_for_bus.volume = volume;
            state.set_bus_state(bus, state_for_bus);
            outcome
                .commands
                .push(AudioCommand::SetBusVolume(AudioBusVolumeCommand::new(
                    bus, volume,
                )));
            outcome.status = format!(
                "{} bus volume set to {}. Existing normal and spatial instances on that bus should update.",
                bus,
                preset.label()
            );
        }
        AudioGalleryButton::ToggleMasterMute => {
            let muted = !state.bus_state(AudioBus::Master).muted;
            let mut master = state.bus_state(AudioBus::Master);
            master.muted = muted;
            state.set_bus_state(AudioBus::Master, master);
            outcome
                .commands
                .push(AudioCommand::SetBusMuted(AudioBusMutedCommand::new(
                    AudioBus::Master,
                    muted,
                )));
            outcome.status = format!(
                "Master mute {}. Existing normal and spatial instances should update.",
                if muted { "enabled" } else { "disabled" }
            );
        }
        AudioGalleryButton::ToggleBusMute(bus) => {
            let muted = !state.bus_state(bus).muted;
            let mut bus_state = state.bus_state(bus);
            bus_state.muted = muted;
            state.set_bus_state(bus, bus_state);
            outcome
                .commands
                .push(AudioCommand::SetBusMuted(AudioBusMutedCommand::new(
                    bus, muted,
                )));
            outcome.status = format!(
                "{} bus mute {}. Existing normal and spatial instances on that bus should update.",
                bus,
                if muted { "enabled" } else { "disabled" }
            );
        }
        AudioGalleryButton::PauseBus(bus) => {
            let mut bus_state = state.bus_state(bus);
            bus_state.paused = true;
            state.set_bus_state(bus, bus_state);
            outcome
                .commands
                .push(AudioCommand::SetBusPaused(AudioBusPausedCommand::new(
                    bus, true,
                )));
            outcome.status = format!(
                "{} bus pause requested. Existing normal and spatial instances on that bus should pause.",
                bus
            );
        }
        AudioGalleryButton::ResumeBus(bus) => {
            let mut bus_state = state.bus_state(bus);
            bus_state.paused = false;
            state.set_bus_state(bus, bus_state);
            outcome
                .commands
                .push(AudioCommand::SetBusPaused(AudioBusPausedCommand::new(
                    bus, false,
                )));
            outcome.status = format!(
                "{} bus resume requested. Existing normal and spatial instances on that bus should resume.",
                bus
            );
        }
        AudioGalleryButton::PreloadGalleryBank => {
            let group_id = group_id(AUDIO_GALLERY_BANK_GROUP_ID);
            outcome
                .commands
                .push(AudioCommand::PreloadGroup(AudioGroupCommand::new(
                    group_id.clone(),
                )));
            outcome.status = format!("Preload requested for {group_id}.");
        }
        AudioGalleryButton::UnloadGalleryBank => {
            let group_id = group_id(AUDIO_GALLERY_BANK_GROUP_ID);
            outcome
                .commands
                .push(AudioCommand::UnloadGroup(AudioGroupCommand::new(
                    group_id.clone(),
                )));
            outcome.status =
                format!("Unload requested for {group_id}; active instances are not stopped.");
        }
        AudioGalleryButton::PlayCooldownRuleCue => {
            let cue_id = cue_id(AUDIO_GALLERY_COOLDOWN_CUE_ID);
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
                    Some(AudioBus::Ui),
                )));
            outcome.status =
                "Requested cooldown cue; click repeatedly to produce CueSkipped(Cooldown)."
                    .to_string();
        }
        AudioGalleryButton::PlayMaxConcurrentRuleCue => {
            let cue_id = cue_id(AUDIO_GALLERY_MAX_CONCURRENT_CUE_ID);
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
            outcome.status =
                "Requested max_concurrent cue; click rapidly to produce skip or replacement behavior."
                    .to_string();
        }
        AudioGalleryButton::PlayMissingCue => {
            let cue_id = cue_id(AUDIO_GALLERY_MISSING_CUE_ID);
            outcome.launches.push(AudioGalleryLaunchKind::Cue {
                cue_id: cue_id.clone(),
                slot: AudioGalleryInstanceSlot::Clip,
            });
            outcome
                .commands
                .push(AudioCommand::PlayCue(gallery_cue_request(
                    cue_id,
                    state.params,
                    false,
                    Some(AudioBus::Sfx),
                )));
            outcome.status =
                "Requested missing-asset cue; LoadFailed should appear here and in Audio Monitor."
                    .to_string();
        }
        AudioGalleryButton::PlayMissingClip => {
            let clip_id = clip_id(AUDIO_GALLERY_MISSING_CLIP_ID);
            outcome.launches.push(AudioGalleryLaunchKind::Clip {
                clip_id: clip_id.clone(),
                slot: AudioGalleryInstanceSlot::Clip,
            });
            outcome
                .commands
                .push(AudioCommand::PlayClip(gallery_clip_request(
                    clip_id,
                    state.params,
                    false,
                    AudioBus::Sfx,
                    None,
                )));
            outcome.status =
                "Requested missing clip directly; LoadFailed should appear here and in Audio Monitor."
                    .to_string();
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

            state.diagnostics.last_started_cue = Some(format!(
                "{} -> {} #{} {} {}",
                started.cue_id, started.clip_id, started.instance_id, started.bus, started.scope
            ));
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
            state.diagnostics.last_load_failed = Some(audio_gallery_load_failed_text(failed));
            if load_failure_is_gallery_owned(state, failed) {
                state.status = audio_gallery_load_failed_text(failed);
            }
        }
        AudioEvent::LoadProgress(progress) => {
            if progress.group_id.as_str() == AUDIO_GALLERY_BANK_GROUP_ID {
                state.diagnostics.last_loading_progress = Some(progress.clone());
                state.status = audio_gallery_load_progress_text(progress);
            }
        }
        AudioEvent::CueSkipped(skipped) => {
            state.diagnostics.last_skipped_cue = Some(audio_gallery_cue_skipped_text(skipped));
            if dev_cue_slot(&skipped.cue_id).is_some() {
                state.status = audio_gallery_cue_skipped_text(skipped);
            }
        }
        AudioEvent::BusChanged(changed) => {
            state.update_bus_change(changed);
            state.status = audio_gallery_bus_changed_text(changed);
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

fn gallery_spatial_cue_request(
    cue_id: AudioCueId,
    params: AudioGalleryPlaybackParams,
    source: AudioSpatialSource,
    attenuation: AudioSpatialAttenuation,
) -> AudioSpatialCueRequest {
    AudioSpatialCueRequest {
        cue_id,
        scope: audio_gallery_scope(),
        bus: Some(AudioBus::Sfx),
        volume: params.volume,
        pitch: params.pitch,
        looped: params.looped,
        fade_in_seconds: params.fade_in_seconds,
        start_seconds: None,
        source,
        attenuation,
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
            .is_some_and(|group_id| group_id.as_str() == AUDIO_GALLERY_BANK_GROUP_ID)
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
        | AUDIO_GALLERY_SWORD_HIT_CUE_ID
        | AUDIO_GALLERY_COOLDOWN_CUE_ID
        | AUDIO_GALLERY_MAX_CONCURRENT_CUE_ID => Some(AudioGalleryInstanceSlot::Sfx),
        AUDIO_GALLERY_RAIN_LOOP_CUE_ID => Some(AudioGalleryInstanceSlot::Loop),
        AUDIO_GALLERY_CAR_HORN_CUE_ID | AUDIO_GALLERY_DOG_BARK_CUE_ID => {
            Some(AudioGalleryInstanceSlot::Spatial)
        }
        AUDIO_GALLERY_MISSING_CUE_ID => Some(AudioGalleryInstanceSlot::Clip),
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

fn audio_gallery_mixer_text(state: &AudioGalleryState) -> String {
    AUDIO_GALLERY_BUSES
        .iter()
        .map(|bus| {
            let bus_state = state.bus_state(*bus);
            format!(
                "{} {:.0}% muted={} paused={}",
                bus,
                bus_state.volume * 100.0,
                yes_no(bus_state.muted),
                yes_no(bus_state.paused)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn audio_gallery_loading_text(
    state: &AudioGalleryState,
    bank: Option<&AudioBankRuntime>,
) -> String {
    let gallery_group = group_status_text(
        AUDIO_GALLERY_BANK_GROUP_ID,
        bank.and_then(|bank| bank.groups.get(&group_id(AUDIO_GALLERY_BANK_GROUP_ID))),
    );
    let resident_group = group_status_text(
        AUDIO_GALLERY_RESIDENT_BANK_GROUP_ID,
        bank.and_then(|bank| {
            bank.groups
                .get(&group_id(AUDIO_GALLERY_RESIDENT_BANK_GROUP_ID))
        }),
    );
    let progress = state
        .diagnostics
        .last_loading_progress
        .as_ref()
        .map(audio_gallery_load_progress_line)
        .unwrap_or_else(|| "last progress none".to_string());

    format!("{gallery_group}\n{resident_group}\n{progress}")
}

fn audio_gallery_diagnostics_text(state: &AudioGalleryState) -> String {
    format!(
        "started: {}\nskipped: {}\nfailed: {}\ndebug: enabled; open Audio Monitor for full snapshot",
        compact_status_value(
            state
                .diagnostics
                .last_started_cue
                .as_deref()
                .unwrap_or("none")
        ),
        compact_status_value(
            state
                .diagnostics
                .last_skipped_cue
                .as_deref()
                .unwrap_or("none")
        ),
        state
            .diagnostics
            .last_load_failed
            .as_deref()
            .unwrap_or("none"),
    )
}

fn audio_gallery_spatial_text(state: &AudioGalleryState) -> String {
    let attenuation = state.spatial_attenuation.value();
    let base = format!(
        "listener {} {} / emitter {} {}\nattenuation {} max {:.0} rolloff {:.1}",
        state.spatial_listener_position.label(),
        format_vec3(state.spatial_listener_position.position()),
        state.spatial_emitter_position.label(),
        format_vec3(state.spatial_emitter_position.position()),
        state.spatial_attenuation.label(),
        attenuation.max_distance,
        attenuation.rolloff_factor,
    );

    let Some(details) = state.spatial_details else {
        return format!(
            "{base}\nsource none\nboundary: Bevy stereo panning + distance attenuation only; no HRTF/reverb/occlusion/full 3D audio"
        );
    };

    format!(
        "{base}\ninstance {} / source {} / pos {} / distance {:.1} / spatial {}\nboundary: Bevy stereo panning + distance attenuation only; no HRTF/reverb/occlusion/full 3D audio",
        state
            .spatial_instance
            .instance_id
            .map(|instance_id| format!("#{instance_id}"))
            .unwrap_or_else(|| "pending/none".to_string()),
        details.source_kind.label(),
        format_vec3(details.position),
        details.distance,
        yes_no(state.spatial_instance.spatial),
    )
}

fn audio_gallery_instances_text(state: &AudioGalleryState) -> String {
    format!(
        "sfx {}\nloop {}\nmusic {}\nspatial {}\nclip {}\nlong {}",
        record_text(&state.last_sfx),
        record_text(&state.loop_instance),
        record_text(&state.music_instance),
        record_text(&state.spatial_instance),
        record_text(&state.clip_instance),
        record_text(&state.long_instance)
    )
}

fn audio_gallery_status_text(state: &AudioGalleryState) -> String {
    format!(
        "{}\nframes open: {}",
        compact_status_value(&state.status),
        state.frames_open
    )
}

fn record_text(record: &AudioGalleryInstanceRecord) -> String {
    let Some(instance_id) = record.instance_id else {
        return "none".to_string();
    };

    let label = compact_label(
        record.label.as_deref().unwrap_or("unknown"),
        AUDIO_GALLERY_RECORD_LABEL_LIMIT,
    );
    let paused = if record.paused { ", paused" } else { "" };
    let spatial = if record.spatial { ", spatial" } else { "" };
    let progress = record
        .position_seconds
        .map(|seconds| format!(", {:.2}s", seconds))
        .unwrap_or_default();
    format!("#{instance_id} {label}{paused}{spatial}{progress}")
}

fn audio_gallery_load_failed_text(failed: &AudioLoadFailed) -> String {
    let id = failed
        .cue_id
        .as_ref()
        .map(|id| format!("cue {id}"))
        .or_else(|| failed.clip_id.as_ref().map(|id| format!("clip {id}")))
        .or_else(|| failed.group_id.as_ref().map(|id| format!("group {id}")))
        .unwrap_or_else(|| "gallery audio".to_string());
    let asset = failed
        .asset_path
        .as_deref()
        .map(compact_asset_label)
        .map(|asset| format!(" ({asset})"))
        .unwrap_or_default();
    format!(
        "Load failed for {}{}: {}",
        compact_status_value(&id),
        asset,
        compact_label(&failed.message, 32)
    )
}

fn audio_gallery_load_progress_text(progress: &AudioLoadProgress) -> String {
    format!(
        "Loading progress: {}",
        audio_gallery_load_progress_line(progress)
    )
}

fn audio_gallery_load_progress_line(progress: &AudioLoadProgress) -> String {
    format!(
        "{} {}/{} loaded, {} failed, required {}/{} loaded, {} required failed{}",
        compact_status_value(progress.group_id.as_str()),
        progress.loaded,
        progress.total,
        progress.failed,
        progress.required_loaded,
        progress.required_total,
        progress.required_failed,
        progress
            .clip_id
            .as_ref()
            .map(|clip_id| format!(" ({})", compact_status_value(clip_id.as_str())))
            .unwrap_or_default()
    )
}

fn audio_gallery_cue_skipped_text(skipped: &AudioCueSkipped) -> String {
    format!(
        "Cue skipped: {} {:?} in {}.",
        skipped.cue_id, skipped.reason, skipped.scope
    )
}

fn audio_gallery_bus_changed_text(changed: &AudioBusChanged) -> String {
    match changed.change {
        AudioBusChange::Volume { previous, current } => format!(
            "{} bus volume changed from {:.0}% to {:.0}%.",
            changed.bus,
            previous * 100.0,
            current * 100.0
        ),
        AudioBusChange::Muted { previous, current } => format!(
            "{} bus muted changed from {} to {}.",
            changed.bus,
            yes_no(previous),
            yes_no(current)
        ),
        AudioBusChange::Paused { previous, current } => format!(
            "{} bus paused changed from {} to {}.",
            changed.bus,
            yes_no(previous),
            yes_no(current)
        ),
    }
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

fn format_vec3(value: Vec3) -> String {
    format!("({:.1}, {:.1}, {:.1})", value.x, value.y, value.z)
}

fn compact_status_value(value: &str) -> String {
    compact_label(
        &value.replace(['\r', '\n'], " "),
        AUDIO_GALLERY_STATUS_VALUE_LIMIT,
    )
}

fn compact_asset_label(path: &str) -> String {
    compact_label(
        path.rsplit(['/', '\\']).next().unwrap_or(path),
        AUDIO_GALLERY_ASSET_LABEL_LIMIT,
    )
}

fn compact_label(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }

    if limit <= 3 {
        return value.chars().take(limit).collect();
    }

    let keep = limit - 3;
    let tail = value
        .chars()
        .rev()
        .take(keep)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("...{tail}")
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

impl AudioGallerySpatialFixedPosition {
    fn position(self) -> Vec3 {
        match self {
            Self::Left => Vec3::new(-18.0, 0.0, 0.0),
            Self::Right => Vec3::new(18.0, 0.0, 0.0),
            Self::Near => Vec3::new(0.0, 0.0, 6.0),
            Self::Far => Vec3::new(0.0, 0.0, 48.0),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Near => "near",
            Self::Far => "far",
        }
    }
}

impl AudioGallerySpatialAttenuationPreset {
    fn value(self) -> AudioSpatialAttenuation {
        match self {
            Self::Close => AudioSpatialAttenuation::new(18.0, 1.0),
            Self::Wide => AudioSpatialAttenuation::new(64.0, 1.0),
            Self::Steep => AudioSpatialAttenuation::new(64.0, 2.5),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Close => "close",
            Self::Wide => "wide",
            Self::Steep => "steep",
        }
    }
}

impl AudioGallerySpatialListenerPosition {
    fn position(self) -> Vec3 {
        match self {
            Self::Center => Vec3::ZERO,
            Self::Left => Vec3::new(-12.0, 0.0, 0.0),
            Self::Right => Vec3::new(12.0, 0.0, 0.0),
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Center => Self::Left,
            Self::Left => Self::Right,
            Self::Right => Self::Center,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Center => "center",
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

impl AudioGallerySpatialEmitterPosition {
    fn position(self) -> Vec3 {
        match self {
            Self::Left => Vec3::new(-20.0, 0.0, 8.0),
            Self::Right => Vec3::new(20.0, 0.0, 8.0),
            Self::Near => Vec3::new(0.0, 0.0, 4.0),
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Near,
            Self::Near => Self::Left,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Near => "near",
        }
    }
}

impl AudioGallerySpatialSourceKind {
    fn label(self) -> &'static str {
        match self {
            Self::Fixed(position) => position.label(),
            Self::FollowEmitter => "follow_entity",
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

impl AudioGalleryBusVolumePreset {
    fn value(self) -> f32 {
        match self {
            Self::Low => 0.5,
            Self::Full => 1.0,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Low => "50%",
            Self::Full => "100%",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AudioGalleryBusAction {
    VolumeLow,
    VolumeFull,
    Mute,
    Pause,
    Resume,
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

fn bus_action_i18n_key(bus: AudioBus, action: AudioGalleryBusAction) -> &'static str {
    match (bus, action) {
        (AudioBus::Master, AudioGalleryBusAction::VolumeLow) => {
            "audio_gallery.bus.master_volume_low"
        }
        (AudioBus::Master, AudioGalleryBusAction::VolumeFull) => {
            "audio_gallery.bus.master_volume_full"
        }
        (AudioBus::Master, AudioGalleryBusAction::Mute) => "audio_gallery.bus.master_mute_toggle",
        (AudioBus::Master, AudioGalleryBusAction::Pause) => "audio_gallery.bus.master_pause",
        (AudioBus::Master, AudioGalleryBusAction::Resume) => "audio_gallery.bus.master_resume",
        (AudioBus::Music, AudioGalleryBusAction::VolumeLow) => "audio_gallery.bus.music_volume_low",
        (AudioBus::Music, AudioGalleryBusAction::VolumeFull) => {
            "audio_gallery.bus.music_volume_full"
        }
        (AudioBus::Music, AudioGalleryBusAction::Mute) => "audio_gallery.bus.music_mute",
        (AudioBus::Music, AudioGalleryBusAction::Pause) => "audio_gallery.bus.music_pause",
        (AudioBus::Music, AudioGalleryBusAction::Resume) => "audio_gallery.bus.music_resume",
        (AudioBus::Sfx, AudioGalleryBusAction::VolumeLow) => "audio_gallery.bus.sfx_volume_low",
        (AudioBus::Sfx, AudioGalleryBusAction::VolumeFull) => "audio_gallery.bus.sfx_volume_full",
        (AudioBus::Sfx, AudioGalleryBusAction::Mute) => "audio_gallery.bus.sfx_mute",
        (AudioBus::Sfx, AudioGalleryBusAction::Pause) => "audio_gallery.bus.sfx_pause",
        (AudioBus::Sfx, AudioGalleryBusAction::Resume) => "audio_gallery.bus.sfx_resume",
        (AudioBus::Ui, AudioGalleryBusAction::VolumeLow) => "audio_gallery.bus.ui_volume_low",
        (AudioBus::Ui, AudioGalleryBusAction::VolumeFull) => "audio_gallery.bus.ui_volume_full",
        (AudioBus::Ui, AudioGalleryBusAction::Mute) => "audio_gallery.bus.ui_mute",
        (AudioBus::Ui, AudioGalleryBusAction::Pause) => "audio_gallery.bus.ui_pause",
        (AudioBus::Ui, AudioGalleryBusAction::Resume) => "audio_gallery.bus.ui_resume",
        (AudioBus::Battle, AudioGalleryBusAction::VolumeLow) => {
            "audio_gallery.bus.battle_volume_low"
        }
        (AudioBus::Battle, AudioGalleryBusAction::VolumeFull) => {
            "audio_gallery.bus.battle_volume_full"
        }
        (AudioBus::Battle, AudioGalleryBusAction::Mute) => "audio_gallery.bus.battle_mute",
        (AudioBus::Battle, AudioGalleryBusAction::Pause) => "audio_gallery.bus.battle_pause",
        (AudioBus::Battle, AudioGalleryBusAction::Resume) => "audio_gallery.bus.battle_resume",
    }
}

fn bus_action_fallback(bus: AudioBus, action: AudioGalleryBusAction) -> &'static str {
    match (bus, action) {
        (AudioBus::Master, AudioGalleryBusAction::VolumeLow) => "Master 50%",
        (AudioBus::Master, AudioGalleryBusAction::VolumeFull) => "Master 100%",
        (AudioBus::Master, AudioGalleryBusAction::Mute) => "Master Mute",
        (AudioBus::Master, AudioGalleryBusAction::Pause) => "Master Pause",
        (AudioBus::Master, AudioGalleryBusAction::Resume) => "Master Resume",
        (AudioBus::Music, AudioGalleryBusAction::VolumeLow) => "Music 50%",
        (AudioBus::Music, AudioGalleryBusAction::VolumeFull) => "Music 100%",
        (AudioBus::Music, AudioGalleryBusAction::Mute) => "Music Mute",
        (AudioBus::Music, AudioGalleryBusAction::Pause) => "Music Pause",
        (AudioBus::Music, AudioGalleryBusAction::Resume) => "Music Resume",
        (AudioBus::Sfx, AudioGalleryBusAction::VolumeLow) => "Sfx 50%",
        (AudioBus::Sfx, AudioGalleryBusAction::VolumeFull) => "Sfx 100%",
        (AudioBus::Sfx, AudioGalleryBusAction::Mute) => "Sfx Mute",
        (AudioBus::Sfx, AudioGalleryBusAction::Pause) => "Sfx Pause",
        (AudioBus::Sfx, AudioGalleryBusAction::Resume) => "Sfx Resume",
        (AudioBus::Ui, AudioGalleryBusAction::VolumeLow) => "Ui 50%",
        (AudioBus::Ui, AudioGalleryBusAction::VolumeFull) => "Ui 100%",
        (AudioBus::Ui, AudioGalleryBusAction::Mute) => "Ui Mute",
        (AudioBus::Ui, AudioGalleryBusAction::Pause) => "Ui Pause",
        (AudioBus::Ui, AudioGalleryBusAction::Resume) => "Ui Resume",
        (AudioBus::Battle, AudioGalleryBusAction::VolumeLow) => "Battle 50%",
        (AudioBus::Battle, AudioGalleryBusAction::VolumeFull) => "Battle 100%",
        (AudioBus::Battle, AudioGalleryBusAction::Mute) => "Battle Mute",
        (AudioBus::Battle, AudioGalleryBusAction::Pause) => "Battle Pause",
        (AudioBus::Battle, AudioGalleryBusAction::Resume) => "Battle Resume",
    }
}

fn metric_label(theme: &UiTheme, fonts: &UiFontAssets, text: impl Into<String>) -> impl Bundle {
    (
        Node {
            width: percent(100),
            overflow: Overflow::clip(),
            ..default()
        },
        screen_label(
            theme,
            fonts,
            text,
            UiThemeTextStyleRole::Caption,
            UiThemeTextColorRole::Primary,
        ),
    )
}

fn cue_id(value: &str) -> AudioCueId {
    AudioCueId::try_from(value).expect("audio gallery cue id must be valid")
}

fn clip_id(value: &str) -> AudioClipId {
    AudioClipId::try_from(value).expect("audio gallery clip id must be valid")
}

fn group_id(value: &str) -> AudioGroupId {
    AudioGroupId::try_from(value).expect("audio gallery group id must be valid")
}

fn default_audio_gallery_bus_states() -> [(AudioBus, AudioBusState); 5] {
    AUDIO_GALLERY_BUSES.map(|bus| (bus, AudioBusState::default()))
}

fn default_audio_gallery_bus_states_from_mixer(
    mixer: &AudioMixer,
) -> [(AudioBus, AudioBusState); 5] {
    AUDIO_GALLERY_BUSES.map(|bus| (bus, mixer.bus_state(bus)))
}

fn group_status_text(group_id: &str, state: Option<&AudioBankGroupState>) -> String {
    let Some(state) = state else {
        return format!("{group_id}: not loaded");
    };

    if state.resident() {
        return format!(
            "{}: resident {} active={}",
            state.group_id,
            bank_load_status_label(state.load_status),
            state.active_instance_ids.len()
        );
    }

    if let Some(countdown) = state.idle_countdown_seconds {
        return format!(
            "{}: idle countdown {:.1}s active={}",
            state.group_id,
            countdown.max(0.0),
            state.active_instance_ids.len()
        );
    }

    format!(
        "{}: {} active={}",
        state.group_id,
        bank_load_status_label(state.load_status),
        state.active_instance_ids.len()
    )
}

fn bank_load_status_label(status: AudioBankLoadStatus) -> &'static str {
    match status {
        AudioBankLoadStatus::NotLoaded => "not loaded",
        AudioBankLoadStatus::Loading => "loading",
        AudioBankLoadStatus::Loaded => "loaded",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::audio::prelude::AudioPlugin;
    use crate::framework::audio::prelude::{
        AudioBankGroupConfig, AudioBusChanged, AudioCatalog, AudioClipStarted, AudioCueSkipReason,
        AudioCueStarted, AudioDebugState, AudioFadeState, AudioGroupClip, AudioGroupEntry,
        AudioInstanceState, AudioInstanceStopped, AudioLoadFailed, AudioLoadingState,
        AudioMetadata, AudioPlaybackInstance, AudioPlaybackState, AudioStopReason,
        DEFAULT_UI_CLICK_CUE_ID, audio_debug_snapshot,
    };
    use crate::framework::ui::widgets::{UiButtonEvent, UiButtonEventKind};
    use crate::framework::ui::{
        core::{UiInputMode, UiPanelRoot, UiSafeArea},
        i18n::{UiI18nPlugin, UiI18nText},
    };
    use bevy::ecs::message::MessageCursor;
    use bevy::ecs::system::RunSystemOnce;
    use std::collections::HashSet;

    fn read_audio_commands(app: &App) -> Vec<AudioCommand> {
        let messages = app.world().resource::<Messages<AudioCommand>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    fn register_test_banked_menu_music(app: &mut App) {
        let group_id = group_id(AUDIO_GALLERY_BANK_GROUP_ID);
        let clip_id = clip_id(AUDIO_GALLERY_MENU_MUSIC_CLIP_ID);
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(clip_id.clone(), "audio/music/menu_loop.wav");
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_group(
                group_id.clone(),
                AudioGroupEntry::from_clips([AudioGroupClip::required(clip_id)]),
            );
        app.world_mut()
            .resource_mut::<AudioBankRuntime>()
            .register_group_config(AudioBankGroupConfig::new(
                group_id,
                std::time::Duration::from_secs_f32(12.0),
            ));
    }

    fn insert_playback_instance(
        app: &mut App,
        instance_id: AudioInstanceId,
        clip_id: AudioClipId,
        scope: AudioScope,
        bus: AudioBus,
    ) {
        let entity = app
            .world_mut()
            .spawn(AudioPlaybackInstance { instance_id })
            .id();
        app.world_mut()
            .resource_mut::<AudioPlaybackState>()
            .instances
            .insert(
                instance_id,
                AudioInstanceState {
                    entity,
                    clip_id,
                    cue_id: None,
                    scope,
                    bus,
                    volume: 1.0,
                    priority: 0,
                    looped: true,
                    asset_path: "audio/music/menu_loop.wav".to_string(),
                    source: Handle::default(),
                    failed: false,
                    paused: false,
                    stopping: false,
                    fade: AudioFadeState::new(0.25, 1.0, 0.0, true),
                    spatial: false,
                    start_seconds: 0.0,
                    position_seconds: 0.0,
                    pending_seek_seconds: None,
                },
            );
    }

    fn phone_portrait_viewport() -> UiViewport {
        UiViewport::from_device_logical_size(
            1080.0 / 3.0,
            2400.0 / 3.0,
            UiInputMode::MouseTouch,
            UiSafeArea::default(),
        )
    }

    fn insert_audio_gallery_ui_test_resources(app: &mut App, viewport: UiViewport) {
        let theme = UiTheme::default();
        let metrics = UiMetrics::from_viewport_and_theme(&viewport, &theme);
        app.insert_resource(theme)
            .insert_resource(metrics)
            .insert_resource(viewport)
            .insert_resource(UiFontAssets {
                regular: Handle::<Font>::default(),
            })
            .insert_resource(ClearColor(Color::srgb(0.0, 0.0, 0.0)))
            .add_message::<AudioCommand>();
    }

    fn setup_audio_gallery_test_app(viewport: UiViewport) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, UiI18nPlugin));
        insert_audio_gallery_ui_test_resources(&mut app, viewport);
        app.world_mut()
            .run_system_once(setup_audio_gallery)
            .expect("audio gallery setup should run");
        app
    }

    fn audio_gallery_i18n_keys(app: &mut App) -> HashSet<String> {
        let mut query = app.world_mut().query::<&UiI18nText>();
        query
            .iter(app.world())
            .map(|text| text.key.clone())
            .collect()
    }

    #[test]
    fn audio_gallery_layout_spawns_stage8_sections_and_navigation_buttons_with_i18n() {
        let mut app = setup_audio_gallery_test_app(phone_portrait_viewport());
        let keys = audio_gallery_i18n_keys(&mut app);

        for key in [
            "audio_gallery.title",
            "audio_gallery.sfx.section",
            "audio_gallery.loop.section",
            "audio_gallery.music.section",
            "audio_gallery.spatial.section",
            "audio_gallery.mixer_loading.section",
            "audio_gallery.rules.section",
            "audio_gallery.status.section",
            "nav.audio_settings",
            "nav.audio_monitor",
            "nav.lobby",
        ] {
            assert!(keys.contains(key), "missing i18n key {key}");
        }

        let mut buttons = app.world_mut().query::<&AudioGalleryButton>();
        let buttons = buttons.iter(app.world()).copied().collect::<Vec<_>>();
        assert!(
            buttons
                .iter()
                .any(|button| matches!(button, AudioGalleryButton::PlaySfx(_)))
        );
        assert!(buttons.contains(&AudioGalleryButton::PlayLoop));
        assert!(
            buttons
                .iter()
                .any(|button| matches!(button, AudioGalleryButton::PlayMusic(_)))
        );
        assert!(
            buttons
                .iter()
                .any(|button| matches!(button, AudioGalleryButton::PlaySpatialFixed(_)))
        );
        assert!(buttons.contains(&AudioGalleryButton::PreloadGalleryBank));
        assert!(buttons.contains(&AudioGalleryButton::PlayCooldownRuleCue));
    }

    #[test]
    fn audio_gallery_compact_layout_uses_single_button_columns() {
        assert_eq!(
            audio_gallery_button_columns().for_width_class(UiWidthClass::Compact),
            1
        );
        assert_eq!(
            audio_gallery_parameter_columns().for_width_class(UiWidthClass::Compact),
            1
        );
        assert_eq!(phone_portrait_viewport().width_class, UiWidthClass::Compact);
    }

    #[test]
    fn audio_gallery_page_panel_is_owned_and_marked_for_route_cleanup() {
        let mut app = setup_audio_gallery_test_app(phone_portrait_viewport());
        let mut panels = app
            .world_mut()
            .query::<(&UiPanelRoot, Option<&DespawnOnExit<AppUiMode>>)>();
        let mut found = false;

        for (panel, cleanup) in panels.iter(app.world()) {
            if panel.id == PANEL_AUDIO_GALLERY {
                found = true;
                assert_eq!(panel.kind, UiPanelKind::Page);
                assert_eq!(panel.owner, Some(OWNER_AUDIO_GALLERY));
                let cleanup = cleanup.expect("Audio Gallery page should despawn on exit");
                assert_eq!(cleanup.0, AppUiMode::AudioGallery);
            }
        }

        assert!(found, "Audio Gallery panel root was not spawned");
    }

    #[test]
    fn long_audio_gallery_status_text_is_split_and_compacted() {
        let mut state = AudioGalleryState::new();
        let raw_path = "audio/dev_gallery/deeply/nested/with/a/very/very/very/long/path/that/should/not/render/in/full/missing_asset_with_a_long_file_name.wav";
        state.record_started(
            AudioGalleryInstanceSlot::Clip,
            AudioInstanceId::from_raw(123456),
            "dev.audio.clip.with.an.extremely.long.identifier.that.should.be.shortened.for.phone.portrait"
                .to_string(),
        );
        apply_audio_gallery_event(
            &mut state,
            &AudioEvent::LoadFailed(AudioLoadFailed {
                clip_id: Some(clip_id(AUDIO_GALLERY_MISSING_CLIP_ID)),
                cue_id: None,
                group_id: Some(group_id(AUDIO_GALLERY_BANK_GROUP_ID)),
                asset_path: Some(raw_path.to_string()),
                message: "failed after trying a deliberately verbose platform path".to_string(),
            }),
        );

        let status_text = audio_gallery_status_text(&state);
        let diagnostics_text = audio_gallery_diagnostics_text(&state);
        let instances_text = audio_gallery_instances_text(&state);

        assert!(status_text.contains('\n'));
        assert!(diagnostics_text.contains('\n'));
        assert!(instances_text.contains('\n'));
        assert!(!status_text.contains(raw_path));
        assert!(!diagnostics_text.contains(raw_path));
        assert!(!instances_text.contains(
            "dev.audio.clip.with.an.extremely.long.identifier.that.should.be.shortened.for.phone.portrait"
        ));
        assert!(diagnostics_text.contains("missing_asset_with_a_long_file_name.wav"));
        assert!(instances_text.contains("..."));
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
    fn setup_state_creates_spatial_listener_target_binding_and_helpers() {
        let mut app = App::new();
        app.world_mut()
            .run_system_once(setup_audio_gallery_state_and_spatial_helpers_system)
            .expect("setup system should run");

        let state = app.world().resource::<AudioGalleryState>();
        let listener_target = state
            .spatial_listener_target
            .expect("listener helper should be stored");
        let emitter_target = state
            .spatial_emitter_target
            .expect("emitter helper should be stored");
        assert_ne!(listener_target, emitter_target);
        assert_eq!(
            app.world().resource::<AudioSpatialListenerBinding>().target,
            listener_target
        );
        assert_eq!(
            app.world()
                .entity(listener_target)
                .get::<AudioGallerySpatialHelper>()
                .unwrap()
                .kind,
            AudioGallerySpatialHelperKind::ListenerTarget
        );
        assert_eq!(
            app.world()
                .entity(emitter_target)
                .get::<AudioGallerySpatialHelper>()
                .unwrap()
                .kind,
            AudioGallerySpatialHelperKind::EmitterTarget
        );
    }

    #[test]
    fn spatial_fixed_buttons_map_to_play_spatial_cue_commands_with_attenuation() {
        let mut state = AudioGalleryState::new();
        state.params.volume = 0.5;
        state.params.pitch = 1.2;
        apply_audio_gallery_button(
            &mut state,
            AudioGalleryButton::SpatialAttenuation(AudioGallerySpatialAttenuationPreset::Close),
        );

        let left = apply_audio_gallery_button(
            &mut state,
            AudioGalleryButton::PlaySpatialFixed(AudioGallerySpatialFixedPosition::Left),
        );
        let right = apply_audio_gallery_button(
            &mut state,
            AudioGalleryButton::PlaySpatialFixed(AudioGallerySpatialFixedPosition::Right),
        );
        let near = apply_audio_gallery_button(
            &mut state,
            AudioGalleryButton::PlaySpatialFixed(AudioGallerySpatialFixedPosition::Near),
        );
        let far = apply_audio_gallery_button(
            &mut state,
            AudioGalleryButton::PlaySpatialFixed(AudioGallerySpatialFixedPosition::Far),
        );

        assert_eq!(
            left.commands,
            vec![AudioCommand::PlaySpatialCue(AudioSpatialCueRequest {
                cue_id: cue_id(AUDIO_GALLERY_CAR_HORN_CUE_ID),
                scope: audio_gallery_scope(),
                bus: Some(AudioBus::Sfx),
                volume: 0.5,
                pitch: 1.2,
                looped: false,
                fade_in_seconds: None,
                start_seconds: None,
                source: AudioSpatialSource::fixed(Transform::from_translation(
                    AudioGallerySpatialFixedPosition::Left.position(),
                )),
                attenuation: AudioSpatialAttenuation::new(18.0, 1.0),
            })]
        );
        assert_eq!(
            left.launches,
            vec![AudioGalleryLaunchKind::Cue {
                cue_id: cue_id(AUDIO_GALLERY_CAR_HORN_CUE_ID),
                slot: AudioGalleryInstanceSlot::Spatial,
            }]
        );

        for (outcome, position) in [
            (right, AudioGallerySpatialFixedPosition::Right),
            (near, AudioGallerySpatialFixedPosition::Near),
            (far, AudioGallerySpatialFixedPosition::Far),
        ] {
            let [AudioCommand::PlaySpatialCue(request)] = outcome.commands.as_slice() else {
                panic!("expected PlaySpatialCue");
            };
            assert_eq!(
                request.source,
                AudioSpatialSource::fixed(Transform::from_translation(position.position()))
            );
            assert_eq!(request.attenuation, AudioSpatialAttenuation::new(18.0, 1.0));
        }
    }

    #[test]
    fn spatial_follow_emitter_and_move_buttons_update_state_and_command() {
        let mut app = App::new();
        app.add_message::<UiButtonEvent>()
            .add_message::<AudioCommand>();
        app.world_mut()
            .run_system_once(setup_audio_gallery_state_and_spatial_helpers_system)
            .expect("setup system should run");
        app.add_systems(Update, handle_audio_gallery_buttons);

        let emitter_target = app
            .world()
            .resource::<AudioGalleryState>()
            .spatial_emitter_target
            .unwrap();
        let move_emitter = app
            .world_mut()
            .spawn(AudioGalleryButton::MoveSpatialEmitter)
            .id();
        let follow = app
            .world_mut()
            .spawn(AudioGalleryButton::PlaySpatialFollowEmitter)
            .id();
        app.world_mut().write_message(UiButtonEvent {
            entity: move_emitter,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.world_mut().write_message(UiButtonEvent {
            entity: follow,
            kind: UiButtonEventKind::Click,
            button: None,
        });

        app.update();

        let state = app.world().resource::<AudioGalleryState>();
        assert_eq!(
            state.spatial_emitter_position,
            AudioGallerySpatialEmitterPosition::Right
        );
        assert_eq!(
            app.world()
                .entity(emitter_target)
                .get::<Transform>()
                .unwrap()
                .translation,
            AudioGallerySpatialEmitterPosition::Right.position()
        );
        assert_eq!(
            read_audio_commands(&app),
            vec![AudioCommand::PlaySpatialCue(AudioSpatialCueRequest {
                cue_id: cue_id(AUDIO_GALLERY_DOG_BARK_CUE_ID),
                scope: audio_gallery_scope(),
                bus: Some(AudioBus::Sfx),
                volume: 1.0,
                pitch: 1.0,
                looped: false,
                fade_in_seconds: None,
                start_seconds: None,
                source: AudioSpatialSource::follow_entity(emitter_target),
                attenuation: AudioSpatialAttenuation::new(64.0, 1.0),
            })]
        );
        assert_eq!(
            state.pending_launches,
            vec![AudioGalleryLaunchKind::Cue {
                cue_id: cue_id(AUDIO_GALLERY_DOG_BARK_CUE_ID),
                slot: AudioGalleryInstanceSlot::Spatial,
            }]
        );
    }

    #[test]
    fn moving_listener_updates_spatial_distance_text() {
        let mut state = AudioGalleryState::new();
        apply_audio_gallery_button(
            &mut state,
            AudioGalleryButton::PlaySpatialFixed(AudioGallerySpatialFixedPosition::Left),
        );

        let before = state.spatial_details.unwrap().distance;
        apply_audio_gallery_button(&mut state, AudioGalleryButton::MoveSpatialListener);
        let after = state.spatial_details.unwrap().distance;

        assert_ne!(before, after);
        assert_eq!(
            state.spatial_listener_position,
            AudioGallerySpatialListenerPosition::Left
        );
        assert!(audio_gallery_spatial_text(&state).contains("distance"));
        assert!(audio_gallery_spatial_text(&state).contains("no HRTF"));
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
        assert_eq!(state.diagnostics.last_started_cue, None);
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
        assert!(
            state
                .diagnostics
                .last_started_cue
                .as_deref()
                .unwrap()
                .contains(AUDIO_GALLERY_FOOTSTEP_CUE_ID)
        );
        assert!(
            state
                .diagnostics
                .last_started_cue
                .as_deref()
                .unwrap()
                .contains("#2")
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
    fn spatial_progress_and_stopped_events_update_spatial_record_and_text() {
        let mut state = AudioGalleryState::new();
        let instance_id = AudioInstanceId::from_raw(10);
        state.record_started(
            AudioGalleryInstanceSlot::Spatial,
            instance_id,
            AUDIO_GALLERY_CAR_HORN_CUE_ID.to_string(),
        );
        state.update_spatial_details(
            AudioGallerySpatialSourceKind::Fixed(AudioGallerySpatialFixedPosition::Far),
            AudioGallerySpatialFixedPosition::Far.position(),
        );

        apply_audio_gallery_event(
            &mut state,
            &AudioEvent::InstanceProgress(AudioInstanceProgress {
                instance_id,
                clip_id: clip_id("dev.audio.spatial.car_horn_taps"),
                cue_id: Some(cue_id(AUDIO_GALLERY_CAR_HORN_CUE_ID)),
                scope: audio_gallery_scope(),
                bus: AudioBus::Sfx,
                position_seconds: 2.5,
                paused: false,
                spatial: true,
            }),
        );

        assert_eq!(state.spatial_instance.position_seconds, Some(2.5));
        assert!(state.spatial_instance.spatial);
        let spatial_text = audio_gallery_spatial_text(&state);
        assert!(spatial_text.contains("instance #10"));
        assert!(spatial_text.contains("source far"));
        assert!(audio_gallery_instances_text(&state).contains("spatial #10"));

        apply_audio_gallery_event(
            &mut state,
            &AudioEvent::InstanceStopped(AudioInstanceStopped {
                instance_id,
                clip_id: Some(clip_id("dev.audio.spatial.car_horn_taps")),
                cue_id: Some(cue_id(AUDIO_GALLERY_CAR_HORN_CUE_ID)),
                scope: audio_gallery_scope(),
                bus: AudioBus::Sfx,
                reason: AudioStopReason::Stopped,
            }),
        );

        assert_eq!(state.spatial_instance.instance_id, None);
        assert!(!state.spatial_instance.spatial);
        assert!(state.status.contains("stopped"));
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
    fn bus_controls_map_to_volume_mute_pause_commands_and_update_display_state() {
        let mut state = AudioGalleryState::new();

        let master_volume = apply_audio_gallery_button(
            &mut state,
            AudioGalleryButton::BusVolume(AudioBus::Master, AudioGalleryBusVolumePreset::Low),
        );
        let music_volume = apply_audio_gallery_button(
            &mut state,
            AudioGalleryButton::BusVolume(AudioBus::Music, AudioGalleryBusVolumePreset::Full),
        );
        let master_mute =
            apply_audio_gallery_button(&mut state, AudioGalleryButton::ToggleMasterMute);
        let sfx_mute = apply_audio_gallery_button(
            &mut state,
            AudioGalleryButton::ToggleBusMute(AudioBus::Sfx),
        );
        let ui_pause =
            apply_audio_gallery_button(&mut state, AudioGalleryButton::PauseBus(AudioBus::Ui));
        let battle_resume =
            apply_audio_gallery_button(&mut state, AudioGalleryButton::ResumeBus(AudioBus::Battle));

        assert_eq!(
            master_volume.commands,
            vec![AudioCommand::SetBusVolume(AudioBusVolumeCommand::new(
                AudioBus::Master,
                0.5,
            ))]
        );
        assert_eq!(
            music_volume.commands,
            vec![AudioCommand::SetBusVolume(AudioBusVolumeCommand::new(
                AudioBus::Music,
                1.0,
            ))]
        );
        assert_eq!(
            master_mute.commands,
            vec![AudioCommand::SetBusMuted(AudioBusMutedCommand::new(
                AudioBus::Master,
                true,
            ))]
        );
        assert_eq!(
            sfx_mute.commands,
            vec![AudioCommand::SetBusMuted(AudioBusMutedCommand::new(
                AudioBus::Sfx,
                true,
            ))]
        );
        assert_eq!(
            ui_pause.commands,
            vec![AudioCommand::SetBusPaused(AudioBusPausedCommand::new(
                AudioBus::Ui,
                true,
            ))]
        );
        assert_eq!(
            battle_resume.commands,
            vec![AudioCommand::SetBusPaused(AudioBusPausedCommand::new(
                AudioBus::Battle,
                false,
            ))]
        );

        let mixer_text = audio_gallery_mixer_text(&state);
        assert!(mixer_text.contains("master 50% muted=yes paused=no"));
        assert!(mixer_text.contains("music 100% muted=no paused=no"));
        assert!(mixer_text.contains("sfx 100% muted=yes paused=no"));
        assert!(mixer_text.contains("ui 100% muted=no paused=yes"));
        assert!(mixer_text.contains("battle 100% muted=no paused=no"));
        assert!(ui_pause.status.contains("spatial instances"));
    }

    #[test]
    fn bus_changed_events_update_gallery_bus_display() {
        let mut state = AudioGalleryState::new();

        apply_audio_gallery_event(
            &mut state,
            &AudioEvent::BusChanged(AudioBusChanged {
                bus: AudioBus::Music,
                change: AudioBusChange::Volume {
                    previous: 1.0,
                    current: 0.25,
                },
            }),
        );
        apply_audio_gallery_event(
            &mut state,
            &AudioEvent::BusChanged(AudioBusChanged {
                bus: AudioBus::Sfx,
                change: AudioBusChange::Paused {
                    previous: false,
                    current: true,
                },
            }),
        );

        assert_eq!(state.bus_state(AudioBus::Music).volume, 0.25);
        assert!(state.bus_state(AudioBus::Sfx).paused);
        assert!(state.status.contains("sfx bus paused changed"));
        assert!(audio_gallery_mixer_text(&state).contains("music 25%"));
    }

    #[test]
    fn preload_unload_and_loading_events_update_bank_status() {
        let mut state = AudioGalleryState::new();

        let preload =
            apply_audio_gallery_button(&mut state, AudioGalleryButton::PreloadGalleryBank);
        let unload = apply_audio_gallery_button(&mut state, AudioGalleryButton::UnloadGalleryBank);

        assert_eq!(
            preload.commands,
            vec![AudioCommand::PreloadGroup(AudioGroupCommand::new(
                group_id(AUDIO_GALLERY_BANK_GROUP_ID)
            ))]
        );
        assert_eq!(
            unload.commands,
            vec![AudioCommand::UnloadGroup(AudioGroupCommand::new(group_id(
                AUDIO_GALLERY_BANK_GROUP_ID
            )))]
        );

        let progress = AudioLoadProgress {
            group_id: group_id(AUDIO_GALLERY_BANK_GROUP_ID),
            loaded: 3,
            total: 10,
            failed: 1,
            required_loaded: 3,
            required_total: 9,
            required_failed: 0,
            clip_id: Some(clip_id(AUDIO_GALLERY_MISSING_CLIP_ID)),
            asset_path: Some("audio/dev_gallery/missing_asset.wav".to_string()),
        };
        apply_audio_gallery_event(&mut state, &AudioEvent::LoadProgress(progress.clone()));
        assert_eq!(state.diagnostics.last_loading_progress, Some(progress));
        assert!(state.status.contains("Loading progress"));

        apply_audio_gallery_event(
            &mut state,
            &AudioEvent::LoadFailed(AudioLoadFailed {
                clip_id: Some(clip_id(AUDIO_GALLERY_MISSING_CLIP_ID)),
                cue_id: None,
                group_id: Some(group_id(AUDIO_GALLERY_BANK_GROUP_ID)),
                asset_path: Some("audio/dev_gallery/missing_asset.wav".to_string()),
                message: "missing asset".to_string(),
            }),
        );
        assert!(
            state
                .diagnostics
                .last_load_failed
                .as_deref()
                .unwrap()
                .contains(AUDIO_GALLERY_MISSING_CLIP_ID)
        );

        let mut bank = AudioBankRuntime::default();
        bank.register_group_config(AudioBankGroupConfig::new(
            group_id(AUDIO_GALLERY_BANK_GROUP_ID),
            std::time::Duration::from_secs_f32(12.0),
        ));
        bank.register_group_config(AudioBankGroupConfig::new(
            group_id(AUDIO_GALLERY_RESIDENT_BANK_GROUP_ID),
            std::time::Duration::ZERO,
        ));
        {
            let group = bank
                .groups
                .get_mut(&group_id(AUDIO_GALLERY_BANK_GROUP_ID))
                .unwrap();
            group.load_status = AudioBankLoadStatus::Loaded;
            group.preload_requested = true;
            group.idle_countdown_seconds = Some(6.5);
        }
        {
            let group = bank
                .groups
                .get_mut(&group_id(AUDIO_GALLERY_RESIDENT_BANK_GROUP_ID))
                .unwrap();
            group.load_status = AudioBankLoadStatus::Loaded;
            group.preload_requested = true;
        }
        let loading_text = audio_gallery_loading_text(&state, Some(&bank));
        assert!(loading_text.contains("bank.audio_gallery: idle countdown 6.5s"));
        assert!(loading_text.contains("bank.audio_gallery.resident: resident loaded"));
        assert!(loading_text.contains("3/10 loaded"));
    }

    #[test]
    fn rules_and_failure_buttons_map_to_expected_cue_and_clip_commands() {
        let mut state = AudioGalleryState::new();

        let cooldown =
            apply_audio_gallery_button(&mut state, AudioGalleryButton::PlayCooldownRuleCue);
        let max_concurrent =
            apply_audio_gallery_button(&mut state, AudioGalleryButton::PlayMaxConcurrentRuleCue);
        let missing_cue =
            apply_audio_gallery_button(&mut state, AudioGalleryButton::PlayMissingCue);
        let missing_clip =
            apply_audio_gallery_button(&mut state, AudioGalleryButton::PlayMissingClip);

        assert_eq!(
            cooldown.commands,
            vec![AudioCommand::PlayCue(AudioCueRequest {
                cue_id: cue_id(AUDIO_GALLERY_COOLDOWN_CUE_ID),
                scope: audio_gallery_scope(),
                bus: Some(AudioBus::Ui),
                volume: 1.0,
                pitch: 1.0,
                looped: false,
                fade_in_seconds: None,
                start_seconds: None,
            })]
        );
        assert_eq!(
            max_concurrent.commands,
            vec![AudioCommand::PlayCue(AudioCueRequest {
                cue_id: cue_id(AUDIO_GALLERY_MAX_CONCURRENT_CUE_ID),
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
            missing_cue.commands,
            vec![AudioCommand::PlayCue(AudioCueRequest {
                cue_id: cue_id(AUDIO_GALLERY_MISSING_CUE_ID),
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
            missing_clip.commands,
            vec![AudioCommand::PlayClip(AudioClipRequest {
                clip_id: clip_id(AUDIO_GALLERY_MISSING_CLIP_ID),
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
            cooldown.launches,
            vec![AudioGalleryLaunchKind::Cue {
                cue_id: cue_id(AUDIO_GALLERY_COOLDOWN_CUE_ID),
                slot: AudioGalleryInstanceSlot::Sfx,
            }]
        );
    }

    #[test]
    fn skipped_and_failure_events_update_diagnostics_text() {
        let mut state = AudioGalleryState::new();

        apply_audio_gallery_event(
            &mut state,
            &AudioEvent::CueSkipped(AudioCueSkipped {
                cue_id: cue_id(AUDIO_GALLERY_COOLDOWN_CUE_ID),
                reason: AudioCueSkipReason::Cooldown,
                scope: AudioScope::Ui,
            }),
        );
        apply_audio_gallery_event(
            &mut state,
            &AudioEvent::CueSkipped(AudioCueSkipped {
                cue_id: cue_id(AUDIO_GALLERY_MAX_CONCURRENT_CUE_ID),
                reason: AudioCueSkipReason::MaxConcurrency,
                scope: audio_gallery_scope(),
            }),
        );
        apply_audio_gallery_event(
            &mut state,
            &AudioEvent::LoadFailed(AudioLoadFailed {
                clip_id: Some(clip_id(AUDIO_GALLERY_MISSING_CLIP_ID)),
                cue_id: Some(cue_id(AUDIO_GALLERY_MISSING_CUE_ID)),
                group_id: None,
                asset_path: Some("audio/dev_gallery/missing_asset.wav".to_string()),
                message: "missing asset".to_string(),
            }),
        );

        assert!(state.status.contains("Load failed"));
        let diagnostics = audio_gallery_diagnostics_text(&state);
        assert!(diagnostics.contains("MaxConcurrency"));
        assert!(diagnostics.contains(AUDIO_GALLERY_MISSING_CUE_ID));
    }

    #[test]
    fn enable_audio_gallery_debug_turns_on_debug_capture() {
        let mut app = App::new();
        app.insert_resource(AudioDebugConfig { enabled: false })
            .add_systems(Update, enable_audio_gallery_debug);

        app.update();

        assert!(app.world().resource::<AudioDebugConfig>().enabled);
    }

    #[test]
    fn cleanup_sends_gallery_scope_stop_bank_unload_and_removes_state() {
        let mut app = App::new();
        app.add_message::<AudioCommand>();
        app.init_resource::<AudioBankRuntime>();
        app.world_mut()
            .resource_mut::<AudioBankRuntime>()
            .register_group_config(AudioBankGroupConfig::new(
                group_id(AUDIO_GALLERY_BANK_GROUP_ID),
                std::time::Duration::from_secs_f32(12.0),
            ));
        app.world_mut()
            .run_system_once(setup_audio_gallery_state_and_spatial_helpers_system)
            .expect("setup system should run");
        let listener_target = app
            .world()
            .resource::<AudioGalleryState>()
            .spatial_listener_target
            .unwrap();
        let emitter_target = app
            .world()
            .resource::<AudioGalleryState>()
            .spatial_emitter_target
            .unwrap();
        let listener_proxy = app.world_mut().spawn_empty().id();
        app.insert_resource(AudioSpatialListenerEntity(listener_proxy));
        app.world_mut()
            .run_system_once(cleanup_audio_gallery)
            .expect("cleanup system should run");

        assert!(!app.world().contains_resource::<AudioGalleryState>());
        assert!(
            !app.world()
                .contains_resource::<AudioSpatialListenerBinding>()
        );
        assert!(
            !app.world()
                .contains_resource::<AudioSpatialListenerEntity>()
        );
        assert!(app.world().get_entity(listener_target).is_err());
        assert!(app.world().get_entity(emitter_target).is_err());
        assert!(app.world().get_entity(listener_proxy).is_err());
        assert_eq!(
            read_audio_commands(&app),
            vec![
                AudioCommand::StopByScope(AudioScopeFadeCommand {
                    scope: audio_gallery_scope(),
                    fade_out_seconds: None,
                }),
                AudioCommand::UnloadGroup(AudioGroupCommand::new(group_id(
                    AUDIO_GALLERY_BANK_GROUP_ID
                ))),
            ]
        );
    }

    #[test]
    fn cleanup_clears_transient_gallery_bank_runtime_without_unloading_resident_group() {
        let gallery_group_id = group_id(AUDIO_GALLERY_BANK_GROUP_ID);
        let resident_group_id = group_id(AUDIO_GALLERY_RESIDENT_BANK_GROUP_ID);
        let mut app = App::new();
        app.add_message::<AudioCommand>()
            .init_resource::<AudioBankRuntime>();
        app.world_mut()
            .resource_mut::<AudioBankRuntime>()
            .register_group_config(AudioBankGroupConfig::new(
                gallery_group_id.clone(),
                std::time::Duration::from_secs_f32(12.0),
            ));
        app.world_mut()
            .resource_mut::<AudioBankRuntime>()
            .register_group_config(AudioBankGroupConfig::new(
                resident_group_id.clone(),
                std::time::Duration::ZERO,
            ));
        {
            let mut bank = app.world_mut().resource_mut::<AudioBankRuntime>();
            let gallery = bank.groups.get_mut(&gallery_group_id).unwrap();
            gallery.preload_requested = true;
            gallery.load_status = AudioBankLoadStatus::Loaded;
            gallery
                .active_instance_ids
                .insert(AudioInstanceId::from_raw(1));
            gallery.idle_countdown_seconds = Some(6.0);
            let resident = bank.groups.get_mut(&resident_group_id).unwrap();
            resident.preload_requested = true;
            resident.load_status = AudioBankLoadStatus::Loaded;
            resident
                .active_instance_ids
                .insert(AudioInstanceId::from_raw(2));
            resident.idle_countdown_seconds = Some(6.0);
        }

        app.world_mut()
            .run_system_once(cleanup_audio_gallery)
            .expect("cleanup system should run");

        let bank = app.world().resource::<AudioBankRuntime>();
        let gallery = bank.groups.get(&gallery_group_id).unwrap();
        assert!(!gallery.preload_requested);
        assert_eq!(gallery.load_status, AudioBankLoadStatus::NotLoaded);
        assert!(gallery.active_instance_ids.is_empty());
        assert_eq!(gallery.idle_countdown_seconds, None);
        let resident = bank.groups.get(&resident_group_id).unwrap();
        assert!(resident.preload_requested);
        assert_eq!(resident.load_status, AudioBankLoadStatus::Loaded);
        assert_eq!(resident.active_instance_ids.len(), 1);
        assert_eq!(resident.idle_countdown_seconds, Some(6.0));
        assert_eq!(
            read_audio_commands(&app),
            vec![
                AudioCommand::StopByScope(AudioScopeFadeCommand {
                    scope: audio_gallery_scope(),
                    fade_out_seconds: None,
                }),
                AudioCommand::UnloadGroup(AudioGroupCommand::new(gallery_group_id)),
            ]
        );
    }

    #[test]
    fn rapid_repeated_play_pause_stop_paths_stay_bounded_and_do_not_panic() {
        let mut state = AudioGalleryState::new();
        let loop_instance = AudioInstanceId::from_raw(11);
        let music_instance = AudioInstanceId::from_raw(12);
        state.record_started(
            AudioGalleryInstanceSlot::Loop,
            loop_instance,
            AUDIO_GALLERY_RAIN_LOOP_CUE_ID.to_string(),
        );
        state.record_started(
            AudioGalleryInstanceSlot::Music,
            music_instance,
            AUDIO_GALLERY_MENU_MUSIC_CLIP_ID.to_string(),
        );

        let buttons = [
            AudioGalleryButton::PlaySfx(AudioGallerySfxCue::Notify),
            AudioGalleryButton::PlaySfx(AudioGallerySfxCue::Notify),
            AudioGalleryButton::PauseLoop,
            AudioGalleryButton::PauseLoop,
            AudioGalleryButton::ResumeLoop,
            AudioGalleryButton::StopLoop,
            AudioGalleryButton::StopLoop,
            AudioGalleryButton::PauseMusic,
            AudioGalleryButton::ResumeMusic,
            AudioGalleryButton::StopMusic,
            AudioGalleryButton::StopMusic,
        ];
        let mut total_commands = 0;
        let mut total_launches = 0;

        for button in buttons {
            let outcome = apply_audio_gallery_button(&mut state, button);
            total_commands += outcome.commands.len();
            total_launches += outcome.launches.len();
        }

        assert_eq!(total_commands, 11);
        assert_eq!(total_launches, 2);
        assert_eq!(state.pending_launches.len(), 0);
        assert!(state.loop_instance.instance_id.is_some());
        assert!(state.music_instance.instance_id.is_some());
    }

    #[test]
    fn lazy_unload_after_cleanup_allows_group_member_to_request_preload_again() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), AudioPlugin))
            .init_asset::<AudioSource>();
        app.update();
        register_test_banked_menu_music(&mut app);

        let clip_id = clip_id(AUDIO_GALLERY_MENU_MUSIC_CLIP_ID);
        app.world_mut()
            .write_message(AudioCommand::PlayMusic(AudioMusicRequest::new(
                clip_id.clone(),
            )));
        app.update();
        app.world_mut()
            .run_system_once(cleanup_audio_gallery)
            .expect("cleanup system should run");
        app.update();
        app.world_mut()
            .write_message(AudioCommand::PlayMusic(AudioMusicRequest::new(clip_id)));
        app.update();

        let commands = read_audio_commands(&app);
        let preload_count = commands
            .iter()
            .filter(|command| {
                matches!(
                    command,
                    AudioCommand::PreloadGroup(command)
                        if command.group_id.as_str() == AUDIO_GALLERY_BANK_GROUP_ID
                )
            })
            .count();
        assert_eq!(preload_count, 2);
        let gallery_bank = app
            .world()
            .resource::<AudioBankRuntime>()
            .groups
            .get(&group_id(AUDIO_GALLERY_BANK_GROUP_ID))
            .unwrap();
        assert!(gallery_bank.preload_requested);
        assert_eq!(gallery_bank.load_status, AudioBankLoadStatus::Loading);
        assert_eq!(gallery_bank.idle_countdown_seconds, None);
    }

    #[test]
    fn cleanup_processed_by_audio_plugin_leaves_no_gallery_scope_instances_for_monitor() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), AudioPlugin))
            .init_asset::<AudioSource>();
        app.update();
        app.world_mut().resource_mut::<AudioDebugConfig>().enabled = true;
        let gallery_instance = AudioInstanceId::from_raw(21);
        let global_instance = AudioInstanceId::from_raw(22);
        insert_playback_instance(
            &mut app,
            gallery_instance,
            clip_id(AUDIO_GALLERY_MENU_MUSIC_CLIP_ID),
            audio_gallery_scope(),
            AudioBus::Music,
        );
        insert_playback_instance(
            &mut app,
            global_instance,
            clip_id(DEFAULT_UI_CLICK_CUE_ID),
            AudioScope::Global,
            AudioBus::Ui,
        );

        app.world_mut()
            .run_system_once(cleanup_audio_gallery)
            .expect("cleanup system should run");
        app.update();

        let playback = app.world().resource::<AudioPlaybackState>();
        assert!(!playback.instances.contains_key(&gallery_instance));
        assert!(playback.instances.contains_key(&global_instance));
        let snapshot = audio_debug_snapshot(
            app.world().resource::<AudioDebugConfig>(),
            app.world().resource::<AudioDebugState>(),
            playback,
            app.world().resource::<AudioLoadingState>(),
            app.world().resource::<AudioMetadata>(),
        );
        assert!(
            snapshot
                .instance_details
                .iter()
                .all(|instance| instance.scope != audio_gallery_scope())
        );
    }
}
