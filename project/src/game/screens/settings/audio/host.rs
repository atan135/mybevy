use std::{collections::BTreeMap, str::FromStr};

use bevy::prelude::*;

#[cfg(all(debug_assertions, not(target_os = "android")))]
use crate::framework::ui::{audit::UiAuditConfig, core::UiViewport, document::UiDocumentRuntime};
use crate::framework::{
    audio::prelude::{
        AudioBus, AudioBusMutedCommand, AudioBusVolumeCommand, AudioCommand, AudioMixer,
    },
    ui::{
        core::{binding::UiBindingValues, focus::UiFocusState},
        document::{
            UiActionDescriptor, UiActionDispatch, UiActionId, UiActionParamSchema,
            UiActionParamType, UiActionRegistry, UiActionValue, UiBindingDeclaration,
            UiBindingMissingBehavior, UiBindingPath, UiBindingScope, UiBindingType, UiBindingValue,
            UiBindingVisibility, UiDocumentId, UiDocumentLayer, UiDocumentPanel, UiHostBindingKey,
            UiNodeId, UiPageState, UiRegisteredActionKind,
        },
    },
};
#[cfg(all(debug_assertions, not(target_os = "android")))]
use crate::game::ui_ids::SCROLL_AUDIO_SETTINGS_MAIN;
use crate::game::{
    declarative_screen::{
        DeclarativeScreenFailurePolicy, DeclarativeScreenHost, DeclarativeScreenHostCommand,
        DeclarativeScreenRegistry, DeclarativeScreenSource,
    },
    navigation::{AppUiMode, GameRouteCommand},
    ui_ids::{OWNER_AUDIO_SETTINGS, OWNER_MAIN_WORLD_SETTINGS_PANEL},
};

pub(super) const AUDIO_SETTINGS_DOCUMENT_ID: &str = "game.audio_settings";
pub(super) const AUDIO_SETTINGS_DOCUMENT_SOURCE_PATH: &str =
    "audio_settings/audio_settings.v1.json";
pub(super) const AUDIO_SETTINGS_DOCUMENT_SOURCE: &str = include_str!(
    "../../../../../assets/ui/documents/approved/audio_settings/audio_settings.v1.json"
);
pub(in crate::game) const MAIN_WORLD_AUDIO_SETTINGS_DOCUMENT_ID: &str =
    "game.main_world_audio_settings";
pub(in crate::game) const MAIN_WORLD_AUDIO_SETTINGS_DOCUMENT_SOURCE_PATH: &str =
    "audio_settings/main_world_audio_settings.v1.json";
pub(super) const MAIN_WORLD_AUDIO_SETTINGS_DOCUMENT_SOURCE: &str = include_str!(
    "../../../../../assets/ui/documents/approved/audio_settings/main_world_audio_settings.v1.json"
);

pub(super) const ACTION_SET_VOLUME: &str = "audio_settings.set_volume";
pub(super) const ACTION_SET_MASTER_MUTED: &str = "audio_settings.set_master_muted";
pub(super) const ACTION_RETURN_LOBBY: &str = "audio_settings.return_lobby";
pub(super) const ACTION_MAIN_WORLD_SET_VOLUME: &str = "main_world_audio_settings.set_volume";
pub(super) const ACTION_MAIN_WORLD_SET_MASTER_MUTED: &str =
    "main_world_audio_settings.set_master_muted";
pub(super) const ACTION_MAIN_WORLD_CLOSE: &str = "main_world_audio_settings.close";
pub(in crate::game) const MAIN_WORLD_SETTINGS_ROUTE: &str = "main_world_settings";

const RETURN_LOBBY_NODE: &str = "audio_settings.return_lobby";
const MASTER_MUTE_NODE: &str = "audio_settings.master.muted";
const VOLUME_MIN_PERCENT: f64 = 0.0;
const VOLUME_MAX_PERCENT: f64 = 100.0;

const VOLUME_SOURCES: [(&str, &str, AudioBus); 5] = [
    ("audio_settings.volume.master", "master", AudioBus::Master),
    ("audio_settings.volume.music", "music", AudioBus::Music),
    ("audio_settings.volume.sfx", "sfx", AudioBus::Sfx),
    ("audio_settings.volume.ui", "ui", AudioBus::Ui),
    ("audio_settings.volume.battle", "battle", AudioBus::Battle),
];

const VOLUME_BINDINGS: [(&str, AudioBus); 5] = [
    ("audio_settings.volume.master", AudioBus::Master),
    ("audio_settings.volume.music", AudioBus::Music),
    ("audio_settings.volume.sfx", AudioBus::Sfx),
    ("audio_settings.volume.ui", AudioBus::Ui),
    ("audio_settings.volume.battle", AudioBus::Battle),
];

#[derive(Resource)]
pub(in crate::game) struct AudioSettingsHostContract {
    pub bindings: BTreeMap<UiBindingPath, UiBindingDeclaration>,
}

impl Default for AudioSettingsHostContract {
    fn default() -> Self {
        Self {
            bindings: audio_settings_binding_schema(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct AudioSettingsSnapshot {
    available: bool,
    disabled: bool,
    volumes: [f64; 5],
    master_muted: bool,
}

impl AudioSettingsSnapshot {
    fn from_mixer(mixer: Option<&AudioMixer>) -> Self {
        let Some(mixer) = mixer else {
            return Self {
                available: false,
                disabled: true,
                volumes: [0.0; 5],
                master_muted: false,
            };
        };
        Self {
            available: true,
            disabled: false,
            volumes: VOLUME_BINDINGS
                .map(|(_, bus)| bus_volume_to_percent(mixer.bus_state(bus).volume) as f64),
            master_muted: mixer.bus_state(AudioBus::Master).muted,
        }
    }
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
#[derive(Clone, Copy, Debug, Default, Resource)]
pub(in crate::game) struct AudioSettingsAuditFixture {
    pub(super) active: bool,
    pub(super) unavailable: bool,
    pub(super) force_disabled: bool,
    pub(super) volumes: Option<[f64; 5]>,
    pub(super) master_muted: Option<bool>,
}

pub(super) fn audio_settings_binding_schema() -> BTreeMap<UiBindingPath, UiBindingDeclaration> {
    binding_schema([
        ("audio_settings.volume.master", UiBindingType::Number),
        ("audio_settings.volume.music", UiBindingType::Number),
        ("audio_settings.volume.sfx", UiBindingType::Number),
        ("audio_settings.volume.ui", UiBindingType::Number),
        ("audio_settings.volume.battle", UiBindingType::Number),
        ("audio_settings.master.muted", UiBindingType::Bool),
        ("audio_settings.controls.disabled", UiBindingType::Bool),
        (
            "audio_settings.view_state",
            UiBindingType::Enum {
                values: ["ready", "unavailable"].map(str::to_owned).to_vec(),
            },
        ),
        ("audio_settings.status", UiBindingType::String),
        ("audio_settings.error.title", UiBindingType::String),
        ("audio_settings.error.detail", UiBindingType::String),
        ("audio_settings.error.visibility", UiBindingType::Visibility),
    ])
}

fn binding_schema<const N: usize>(
    specs: [(&str, UiBindingType); N],
) -> BTreeMap<UiBindingPath, UiBindingDeclaration> {
    specs
        .into_iter()
        .map(|(path, value_type)| {
            (
                UiBindingPath::from_str(path)
                    .expect("Audio Settings binding paths are static and valid"),
                UiBindingDeclaration {
                    scope: UiBindingScope::Owner,
                    value_type,
                    default: None,
                    missing: UiBindingMissingBehavior::UseConsumerFallback,
                },
            )
        })
        .collect()
}

pub(in crate::game::screens::settings) fn register_audio_settings_contract(
    contract: Res<AudioSettingsHostContract>,
    mut actions: ResMut<UiActionRegistry>,
    mut screens: ResMut<DeclarativeScreenRegistry>,
) {
    for descriptor in audio_settings_action_descriptors()
        .into_iter()
        .chain(main_world_audio_settings_action_descriptors())
    {
        actions
            .register(descriptor)
            .expect("Audio Settings action registration must be valid and unique");
    }
    screens
        .register(audio_settings_declarative_screen_host(contract.as_ref()))
        .expect("Audio Settings declarative screen registration must be valid and unique");
    screens
        .register(main_world_audio_settings_declarative_screen_host(
            contract.as_ref(),
        ))
        .expect("Main World Audio Settings screen registration must be valid and unique");
}

pub(super) fn audio_settings_declarative_screen_host(
    contract: &AudioSettingsHostContract,
) -> DeclarativeScreenHost {
    let source = DeclarativeScreenSource::approved(
        AUDIO_SETTINGS_DOCUMENT_SOURCE_PATH,
        AUDIO_SETTINGS_DOCUMENT_SOURCE,
    );
    DeclarativeScreenHost {
        document_id: UiDocumentId::from_str(AUDIO_SETTINGS_DOCUMENT_ID)
            .expect("Audio Settings document ID is static and valid"),
        route: "audio_settings",
        route_aliases: &["audio_settings", "audio-settings", "audio", "settings"],
        mode: Some(AppUiMode::AudioSettings),
        owner: OWNER_AUDIO_SETTINGS,
        panel: UiDocumentPanel::Page,
        layer: UiDocumentLayer::Page,
        initial_state: UiPageState::initial(),
        binding_schema: contract
            .bindings
            .iter()
            .map(|(path, declaration)| {
                (
                    UiHostBindingKey::new(declaration.scope, path.clone()),
                    declaration.value_type.clone(),
                )
            })
            .collect(),
        action_allowlist: [
            ACTION_SET_VOLUME,
            ACTION_SET_MASTER_MUTED,
            ACTION_RETURN_LOBBY,
        ]
        .into_iter()
        .map(|action| {
            UiActionId::from_str(action).expect("Audio Settings action IDs are static and valid")
        })
        .collect(),
        audit_profiles: [
            "desktop",
            "phone-landscape",
            "phone-1080p-landscape",
            "tablet-landscape",
        ]
        .map(str::to_owned)
        .to_vec(),
        source: source.clone(),
        fallback_source: Some(source),
        failure_policy: DeclarativeScreenFailurePolicy::PackagedFallback,
    }
}

pub(in crate::game) fn main_world_audio_settings_declarative_screen_host(
    contract: &AudioSettingsHostContract,
) -> DeclarativeScreenHost {
    let source = DeclarativeScreenSource::approved(
        MAIN_WORLD_AUDIO_SETTINGS_DOCUMENT_SOURCE_PATH,
        MAIN_WORLD_AUDIO_SETTINGS_DOCUMENT_SOURCE,
    );
    DeclarativeScreenHost {
        document_id: UiDocumentId::from_str(MAIN_WORLD_AUDIO_SETTINGS_DOCUMENT_ID)
            .expect("Main World Audio Settings document ID is static and valid"),
        route: MAIN_WORLD_SETTINGS_ROUTE,
        route_aliases: &["main_world_settings", "main-world-settings"],
        mode: None,
        owner: OWNER_MAIN_WORLD_SETTINGS_PANEL,
        panel: UiDocumentPanel::Floating,
        layer: UiDocumentLayer::Floating,
        initial_state: UiPageState::initial(),
        binding_schema: host_binding_schema(contract),
        action_allowlist: [
            ACTION_MAIN_WORLD_SET_VOLUME,
            ACTION_MAIN_WORLD_SET_MASTER_MUTED,
            ACTION_MAIN_WORLD_CLOSE,
        ]
        .into_iter()
        .map(|action| UiActionId::from_str(action).expect("static action ID is valid"))
        .collect(),
        audit_profiles: [
            "desktop",
            "phone-landscape",
            "phone-1080p-landscape",
            "tablet-landscape",
        ]
        .map(str::to_owned)
        .to_vec(),
        source: source.clone(),
        fallback_source: Some(source),
        failure_policy: DeclarativeScreenFailurePolicy::PackagedFallback,
    }
}

fn host_binding_schema(
    contract: &AudioSettingsHostContract,
) -> BTreeMap<UiHostBindingKey, UiBindingType> {
    contract
        .bindings
        .iter()
        .map(|(path, declaration)| {
            (
                UiHostBindingKey::new(declaration.scope, path.clone()),
                declaration.value_type.clone(),
            )
        })
        .collect()
}

pub(super) fn audio_settings_action_descriptors() -> Vec<UiActionDescriptor> {
    let document_id = || UiDocumentId::from_str(AUDIO_SETTINGS_DOCUMENT_ID).unwrap();
    vec![
        UiActionDescriptor::new(
            UiActionId::from_str(ACTION_SET_VOLUME).unwrap(),
            document_id(),
            OWNER_AUDIO_SETTINGS.as_str(),
            business_command(ACTION_SET_VOLUME),
        )
        .with_sources(
            VOLUME_SOURCES
                .iter()
                .map(|(source, _, _)| UiNodeId::from_str(source).unwrap()),
        )
        .with_param(
            "bus",
            UiActionParamSchema::required(UiActionParamType::Enum {
                values: VOLUME_SOURCES
                    .iter()
                    .map(|(_, bus, _)| (*bus).to_owned())
                    .collect(),
            }),
        )
        .with_param(
            "value",
            UiActionParamSchema::required(UiActionParamType::Number {
                min: Some(VOLUME_MIN_PERCENT),
                max: Some(VOLUME_MAX_PERCENT),
            }),
        ),
        UiActionDescriptor::new(
            UiActionId::from_str(ACTION_SET_MASTER_MUTED).unwrap(),
            document_id(),
            OWNER_AUDIO_SETTINGS.as_str(),
            business_command(ACTION_SET_MASTER_MUTED),
        )
        .with_source(UiNodeId::from_str(MASTER_MUTE_NODE).unwrap())
        .with_param(
            "muted",
            UiActionParamSchema::required(UiActionParamType::Bool),
        ),
        UiActionDescriptor::new(
            UiActionId::from_str(ACTION_RETURN_LOBBY).unwrap(),
            document_id(),
            OWNER_AUDIO_SETTINGS.as_str(),
            business_command(ACTION_RETURN_LOBBY),
        )
        .with_source(UiNodeId::from_str(RETURN_LOBBY_NODE).unwrap()),
    ]
}

pub(super) fn main_world_audio_settings_action_descriptors() -> Vec<UiActionDescriptor> {
    let document_id = || UiDocumentId::from_str(MAIN_WORLD_AUDIO_SETTINGS_DOCUMENT_ID).unwrap();
    vec![
        UiActionDescriptor::new(
            UiActionId::from_str(ACTION_MAIN_WORLD_SET_VOLUME).unwrap(),
            document_id(),
            OWNER_MAIN_WORLD_SETTINGS_PANEL.as_str(),
            business_command(ACTION_MAIN_WORLD_SET_VOLUME),
        )
        .with_sources(
            VOLUME_SOURCES
                .iter()
                .map(|(source, _, _)| UiNodeId::from_str(source).unwrap()),
        )
        .with_param(
            "bus",
            UiActionParamSchema::required(UiActionParamType::Enum {
                values: VOLUME_SOURCES
                    .iter()
                    .map(|(_, bus, _)| (*bus).to_owned())
                    .collect(),
            }),
        )
        .with_param(
            "value",
            UiActionParamSchema::required(UiActionParamType::Number {
                min: Some(VOLUME_MIN_PERCENT),
                max: Some(VOLUME_MAX_PERCENT),
            }),
        ),
        UiActionDescriptor::new(
            UiActionId::from_str(ACTION_MAIN_WORLD_SET_MASTER_MUTED).unwrap(),
            document_id(),
            OWNER_MAIN_WORLD_SETTINGS_PANEL.as_str(),
            business_command(ACTION_MAIN_WORLD_SET_MASTER_MUTED),
        )
        .with_source(UiNodeId::from_str(MASTER_MUTE_NODE).unwrap())
        .with_param(
            "muted",
            UiActionParamSchema::required(UiActionParamType::Bool),
        ),
        UiActionDescriptor::new(
            UiActionId::from_str(ACTION_MAIN_WORLD_CLOSE).unwrap(),
            document_id(),
            OWNER_MAIN_WORLD_SETTINGS_PANEL.as_str(),
            business_command(ACTION_MAIN_WORLD_CLOSE),
        )
        .with_source(UiNodeId::from_str(RETURN_LOBBY_NODE).unwrap()),
    ]
}

fn business_command(target: &str) -> UiRegisteredActionKind {
    UiRegisteredActionKind::BusinessCommand {
        target: target.to_owned(),
    }
}

pub(in crate::game::screens::settings) fn handle_audio_settings_document_actions(
    mixer: Option<Res<AudioMixer>>,
    #[cfg(all(debug_assertions, not(target_os = "android")))] audit: Option<
        Res<AudioSettingsAuditFixture>,
    >,
    mut actions: MessageReader<UiActionDispatch>,
    mut audio_commands: MessageWriter<AudioCommand>,
    mut route_commands: MessageWriter<GameRouteCommand>,
    mut screen_commands: MessageWriter<DeclarativeScreenHostCommand>,
    mut focus: ResMut<UiFocusState>,
) {
    let available = mixer.is_some();
    let disabled = !available;
    #[cfg(all(debug_assertions, not(target_os = "android")))]
    let (available, disabled) = {
        let mut available = available;
        let mut disabled = disabled;
        if let Some(audit) = audit.as_deref().filter(|audit| audit.active) {
            available &= !audit.unavailable;
            disabled |= audit.force_disabled || audit.unavailable;
        }
        (available, disabled)
    };

    for action in actions.read() {
        let Some(surface) = audio_settings_action_surface(action) else {
            continue;
        };
        match (surface, action.action.as_str()) {
            (_, ACTION_SET_VOLUME | ACTION_MAIN_WORLD_SET_VOLUME) if available && !disabled => {
                let Some((bus, percent)) = volume_action_value(action) else {
                    continue;
                };
                audio_commands.write(AudioCommand::SetBusVolume(AudioBusVolumeCommand::new(
                    bus,
                    percent_to_bus_volume(percent),
                )));
            }
            (_, ACTION_SET_MASTER_MUTED | ACTION_MAIN_WORLD_SET_MASTER_MUTED)
                if available
                    && !disabled
                    && action.source_node.as_str() == MASTER_MUTE_NODE
                    && action.params.len() == 1 =>
            {
                let Some(UiActionValue::Bool(muted)) = action.params.get("muted") else {
                    continue;
                };
                audio_commands.write(AudioCommand::SetBusMuted(AudioBusMutedCommand::new(
                    AudioBus::Master,
                    *muted,
                )));
            }
            (AudioSettingsSurface::Lobby, ACTION_RETURN_LOBBY)
                if action.source_node.as_str() == RETURN_LOBBY_NODE && action.params.is_empty() =>
            {
                route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::Lobby));
            }
            (AudioSettingsSurface::MainWorld, ACTION_MAIN_WORLD_CLOSE)
                if action.source_node.as_str() == RETURN_LOBBY_NODE && action.params.is_empty() =>
            {
                focus.focused_entity = None;
                screen_commands.write(DeclarativeScreenHostCommand::CloseRoute {
                    route: MAIN_WORLD_SETTINGS_ROUTE.to_owned(),
                });
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AudioSettingsSurface {
    Lobby,
    MainWorld,
}

fn audio_settings_action_surface(action: &UiActionDispatch) -> Option<AudioSettingsSurface> {
    let matches_business_action = matches!(
        &action.kind,
        UiRegisteredActionKind::BusinessCommand { target }
            if target == action.action.as_str()
    );
    if !matches_business_action {
        return None;
    }
    match (
        action.document_id.as_str(),
        action.owner.as_str(),
        action.action.as_str(),
    ) {
        (
            AUDIO_SETTINGS_DOCUMENT_ID,
            owner,
            ACTION_SET_VOLUME | ACTION_SET_MASTER_MUTED | ACTION_RETURN_LOBBY,
        ) if owner == OWNER_AUDIO_SETTINGS.as_str() => Some(AudioSettingsSurface::Lobby),
        (
            MAIN_WORLD_AUDIO_SETTINGS_DOCUMENT_ID,
            owner,
            ACTION_MAIN_WORLD_SET_VOLUME
            | ACTION_MAIN_WORLD_SET_MASTER_MUTED
            | ACTION_MAIN_WORLD_CLOSE,
        ) if owner == OWNER_MAIN_WORLD_SETTINGS_PANEL.as_str() => {
            Some(AudioSettingsSurface::MainWorld)
        }
        _ => None,
    }
}

fn volume_action_value(action: &UiActionDispatch) -> Option<(AudioBus, f64)> {
    if action.params.len() != 2 {
        return None;
    }
    let source = action.source_node.as_str();
    let (expected_bus_name, bus) =
        VOLUME_SOURCES
            .iter()
            .find_map(|(expected_source, bus_name, bus)| {
                (*expected_source == source).then_some((*bus_name, *bus))
            })?;
    match (action.params.get("bus"), action.params.get("value")) {
        (Some(UiActionValue::Enum(bus_name)), Some(UiActionValue::Number(value)))
            if bus_name == expected_bus_name && value.is_finite() =>
        {
            Some((bus, value.clamp(VOLUME_MIN_PERCENT, VOLUME_MAX_PERCENT)))
        }
        _ => None,
    }
}

pub(in crate::game::screens::settings) fn sync_audio_settings_document_bindings(
    mixer: Option<Res<AudioMixer>>,
    contract: Res<AudioSettingsHostContract>,
    #[cfg(all(debug_assertions, not(target_os = "android")))] audit: Option<
        Res<AudioSettingsAuditFixture>,
    >,
    mut values: ResMut<UiBindingValues>,
) {
    sync_audio_settings_bindings_for_surface(
        mixer,
        &contract,
        #[cfg(all(debug_assertions, not(target_os = "android")))]
        audit.as_deref(),
        AUDIO_SETTINGS_DOCUMENT_ID,
        OWNER_AUDIO_SETTINGS.as_str(),
        &mut values,
    );
}

pub(in crate::game) fn sync_main_world_audio_settings_document_bindings(
    mixer: Option<Res<AudioMixer>>,
    contract: Res<AudioSettingsHostContract>,
    #[cfg(all(debug_assertions, not(target_os = "android")))] audit: Option<
        Res<AudioSettingsAuditFixture>,
    >,
    mut values: ResMut<UiBindingValues>,
) {
    sync_audio_settings_bindings_for_surface(
        mixer,
        &contract,
        #[cfg(all(debug_assertions, not(target_os = "android")))]
        audit.as_deref(),
        MAIN_WORLD_AUDIO_SETTINGS_DOCUMENT_ID,
        OWNER_MAIN_WORLD_SETTINGS_PANEL.as_str(),
        &mut values,
    );
}

fn sync_audio_settings_bindings_for_surface(
    mixer: Option<Res<AudioMixer>>,
    contract: &AudioSettingsHostContract,
    #[cfg(all(debug_assertions, not(target_os = "android")))] audit: Option<
        &AudioSettingsAuditFixture,
    >,
    document_id: &str,
    owner: &str,
    values: &mut UiBindingValues,
) {
    let snapshot = AudioSettingsSnapshot::from_mixer(mixer.as_deref());
    #[cfg(all(debug_assertions, not(target_os = "android")))]
    let snapshot = {
        let mut snapshot = snapshot;
        if let Some(audit) = audit.filter(|audit| audit.active) {
            if audit.unavailable {
                snapshot.available = false;
                snapshot.disabled = true;
            } else {
                snapshot.disabled |= audit.force_disabled;
            }
            if let Some(volumes) = audit.volumes {
                snapshot.volumes = volumes.map(clamp_percent);
            }
            if let Some(muted) = audit.master_muted {
                snapshot.master_muted = muted;
            }
        }
        snapshot
    };

    for (path, value) in VOLUME_BINDINGS
        .iter()
        .zip(snapshot.volumes)
        .map(|((path, _), value)| (*path, UiBindingValue::Number(clamp_percent(value))))
    {
        set_binding(contract, values, document_id, owner, path, value);
    }
    for (path, value) in [
        (
            "audio_settings.master.muted",
            UiBindingValue::Bool(snapshot.master_muted),
        ),
        (
            "audio_settings.controls.disabled",
            UiBindingValue::Bool(snapshot.disabled),
        ),
        (
            "audio_settings.view_state",
            UiBindingValue::Enum(
                if snapshot.available {
                    "ready"
                } else {
                    "unavailable"
                }
                .to_owned(),
            ),
        ),
        (
            "audio_settings.status",
            UiBindingValue::String(
                if snapshot.available {
                    "Changes apply immediately"
                } else {
                    "Audio settings unavailable"
                }
                .to_owned(),
            ),
        ),
        (
            "audio_settings.error.title",
            UiBindingValue::String(
                if snapshot.available {
                    ""
                } else {
                    "Audio controls unavailable"
                }
                .to_owned(),
            ),
        ),
        (
            "audio_settings.error.detail",
            UiBindingValue::String(
                if snapshot.available {
                    ""
                } else {
                    "The audio mixer is not available. Return to the lobby and try again."
                }
                .to_owned(),
            ),
        ),
        (
            "audio_settings.error.visibility",
            UiBindingValue::Visibility(if snapshot.available {
                UiBindingVisibility::Hidden
            } else {
                UiBindingVisibility::Visible
            }),
        ),
    ] {
        set_binding(contract, values, document_id, owner, path, value);
    }
}

fn set_binding(
    contract: &AudioSettingsHostContract,
    values: &mut UiBindingValues,
    document_id: &str,
    owner: &str,
    path: &str,
    value: UiBindingValue,
) {
    let path = UiBindingPath::from_str(path).unwrap();
    let declaration = contract
        .bindings
        .get(&path)
        .expect("Audio Settings binding schema contains every synchronized value");
    values.set_scoped(document_id, owner, &path, declaration, value);
}

pub(in crate::game::screens::settings) fn cleanup_audio_settings_screen(
    mut focus: ResMut<UiFocusState>,
) {
    focus.focused_entity = None;
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
pub(in crate::game::screens::settings) fn prepare_audio_settings_audit_fixture(
    audit_config: Res<UiAuditConfig>,
    viewport: Res<UiViewport>,
    mut fixture: ResMut<AudioSettingsAuditFixture>,
) {
    if !audit_config.targets_screen("audio_settings")
        || audit_config.stable_fixture_id() != Some("stage14_audio_settings")
    {
        return;
    }
    *fixture = audio_settings_audit_fixture_for_viewport(&viewport);
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
pub(in crate::game::screens::settings) fn mark_audio_settings_audit_scroll(
    runtime: Res<UiDocumentRuntime>,
    mut commands: Commands,
) {
    let document_id = UiDocumentId::from_str(AUDIO_SETTINGS_DOCUMENT_ID)
        .expect("Audio Settings document ID is static and valid");
    let Some(instance) = runtime.active_instance(OWNER_AUDIO_SETTINGS.as_str(), &document_id)
    else {
        return;
    };
    let scroll_id = UiNodeId::from_str("audio_settings.scroll")
        .expect("Audio Settings scroll node ID is static and valid");
    let Some(scroll) = runtime.node_entity(instance, &scroll_id) else {
        return;
    };
    commands.entity(scroll).insert(SCROLL_AUDIO_SETTINGS_MAIN);
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
pub(super) fn audio_settings_audit_fixture_for_viewport(
    viewport: &UiViewport,
) -> AudioSettingsAuditFixture {
    let mut fixture = AudioSettingsAuditFixture {
        active: true,
        ..default()
    };
    if viewport.logical_height >= 600.0 && viewport.device_scale > 1.0 {
        fixture.unavailable = true;
    } else if viewport.device_scale >= 2.5 {
        fixture.force_disabled = true;
        fixture.volumes = Some([95.0, 70.0, 45.0, 20.0, 5.0]);
    } else if viewport.logical_height < 600.0 {
        fixture.volumes = Some([-10.0, 0.0, 50.0, 100.0, 140.0]);
        fixture.master_muted = Some(true);
    } else {
        fixture.volumes = Some([72.0, 41.0, 63.0, 58.0, 80.0]);
    }
    fixture
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
pub(in crate::game::screens::settings) fn clear_audio_settings_audit_fixture(
    mut fixture: ResMut<AudioSettingsAuditFixture>,
) {
    *fixture = AudioSettingsAuditFixture::default();
}

fn clamp_percent(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(VOLUME_MIN_PERCENT, VOLUME_MAX_PERCENT)
    } else {
        VOLUME_MIN_PERCENT
    }
}

fn bus_volume_to_percent(volume: f32) -> f32 {
    if volume.is_finite() {
        (volume * VOLUME_MAX_PERCENT as f32)
            .clamp(VOLUME_MIN_PERCENT as f32, VOLUME_MAX_PERCENT as f32)
    } else {
        VOLUME_MIN_PERCENT as f32
    }
}

fn percent_to_bus_volume(percent: f64) -> f32 {
    (clamp_percent(percent) / VOLUME_MAX_PERCENT) as f32
}
