use super::host::*;

use std::{collections::BTreeMap, str::FromStr};

use bevy::{ecs::message::MessageCursor, prelude::*, state::app::StatesPlugin};

use crate::framework::{
    audio::prelude::{
        AudioBus, AudioBusMutedCommand, AudioBusState, AudioBusVolumeCommand, AudioCommand,
        AudioMixer,
    },
    ui::{
        core::{UiMetrics, UiViewport, binding::UiBindingValues, focus::UiFocusState},
        document::{
            UiActionDispatch, UiActionId, UiActionValue, UiBindingPath, UiBindingScope,
            UiBindingValue, UiDocument, UiDocumentId, UiDocumentInputMode, UiDocumentPlatform,
            UiDocumentPreviewPlugin, UiDocumentRuntime, UiDocumentRuntimePlugin, UiNode, UiNodeId,
            UiPageState, UiRegisteredActionKind, UiSafeAreaClass, UiTargetProfile,
            parse_approved_document_registration,
        },
        style::{UiFontAssets, UiTheme},
        widgets::controls::UiSlider,
    },
};
use crate::game::{
    declarative_screen::{DeclarativeScreenHostCommand, DeclarativeScreenHostPlugin},
    navigation::{AppUiMode, GameRouteCommand},
    ui_ids::{OWNER_AUDIO_SETTINGS, OWNER_MAIN_WORLD_SETTINGS_PANEL},
};

#[test]
fn audio_settings_document_is_valid_and_uses_only_existing_business_controls() {
    let validation = UiDocument::validate_json(AUDIO_SETTINGS_DOCUMENT_SOURCE);
    assert!(
        validation.report.valid,
        "{:#?}",
        validation.report.diagnostics
    );
    let document = validation.validated().unwrap().document();
    let mut sliders = 0;
    let mut toggles = 0;
    visit_nodes(&document.root, &mut |node| match node {
        UiNode::Slider { .. } => sliders += 1,
        UiNode::Toggle { .. } => toggles += 1,
        UiNode::Stepper { .. }
        | UiNode::Segmented { .. }
        | UiNode::Select { .. }
        | UiNode::Tab { .. } => panic!("unsupported settings control in Audio Settings"),
        _ => {}
    });

    assert_eq!(sliders, 5);
    assert_eq!(toggles, 1);
    assert!(document.bindings.values().all(|binding| {
        binding.scope == UiBindingScope::Owner
            && binding.default.is_none()
            && binding.missing
                == crate::framework::ui::document::UiBindingMissingBehavior::UseConsumerFallback
    }));
    assert!(!AUDIO_SETTINGS_DOCUMENT_SOURCE.contains("save"));
    assert!(!AUDIO_SETTINGS_DOCUMENT_SOURCE.contains("dirty"));
    assert!(!AUDIO_SETTINGS_DOCUMENT_SOURCE.contains("restore_default"));
}

#[test]
fn audio_settings_promotion_registration_matches_fixed_host_contract() {
    const REGISTRATION_SOURCE: &str = include_str!(
        "../../../../../assets/ui/documents/approved/audio_settings/audio_settings.promotion.v1.json"
    );
    let contract = AudioSettingsHostContract::default();
    let host = audio_settings_declarative_screen_host(&contract);
    let registration = parse_approved_document_registration(REGISTRATION_SOURCE).unwrap();
    let audit = registration
        .audit_report(AUDIO_SETTINGS_DOCUMENT_SOURCE)
        .unwrap();

    assert_eq!(host.document_id.as_str(), AUDIO_SETTINGS_DOCUMENT_ID);
    assert_eq!(host.mode, Some(AppUiMode::AudioSettings));
    assert_eq!(host.owner, OWNER_AUDIO_SETTINGS);
    assert_eq!(host.route, "audio_settings");
    assert_eq!(host.action_allowlist.len(), 3);
    assert_eq!(host.binding_schema.len(), 12);
    assert_eq!(registration.owner(), OWNER_AUDIO_SETTINGS.as_str());
    assert_eq!(registration.route(), "audio_settings");
    assert_eq!(audit.actions.len(), 3);
    assert_eq!(audit.bindings.len(), 12);
}

#[test]
fn main_world_audio_settings_reuses_the_schema_as_a_floating_scene_panel() {
    const REGISTRATION_SOURCE: &str = include_str!(
        "../../../../../assets/ui/documents/approved/audio_settings/main_world_audio_settings.promotion.v1.json"
    );
    let contract = AudioSettingsHostContract::default();
    let host = main_world_audio_settings_declarative_screen_host(&contract);
    let registration = parse_approved_document_registration(REGISTRATION_SOURCE).unwrap();

    assert_eq!(
        host.document_id.as_str(),
        MAIN_WORLD_AUDIO_SETTINGS_DOCUMENT_ID
    );
    assert_eq!(host.mode, None);
    assert_eq!(host.owner, OWNER_MAIN_WORLD_SETTINGS_PANEL);
    assert_eq!(host.route, MAIN_WORLD_SETTINGS_ROUTE);
    assert_eq!(
        host.panel,
        crate::framework::ui::document::UiDocumentPanel::Floating
    );
    assert_eq!(
        host.layer,
        crate::framework::ui::document::UiDocumentLayer::Floating
    );
    assert_eq!(host.binding_schema.len(), contract.bindings.len());
    assert_eq!(
        registration.owner(),
        OWNER_MAIN_WORLD_SETTINGS_PANEL.as_str()
    );
    assert_eq!(registration.route(), MAIN_WORLD_SETTINGS_ROUTE);
    assert_eq!(
        registration.panel(),
        crate::framework::ui::document::UiDocumentPanel::Floating
    );
}

#[test]
fn main_world_audio_settings_stays_scrollable_and_actionable_in_audit_profiles() {
    let validated =
        UiDocument::parse_and_validate_json(MAIN_WORLD_AUDIO_SETTINGS_DOCUMENT_SOURCE).unwrap();
    for profile in main_world_audit_profiles() {
        let effective = validated
            .effective_document(&profile, &UiPageState::initial())
            .unwrap();
        for node in [
            "audio_settings.scroll",
            "audio_settings.return_lobby",
            "audio_settings.master.muted",
            "audio_settings.volume.master",
            "audio_settings.volume.music",
            "audio_settings.volume.sfx",
            "audio_settings.volume.ui",
            "audio_settings.volume.battle",
        ] {
            assert!(
                find_document_node(&effective.document.root, node).is_some(),
                "{node}"
            );
        }
    }
}

#[test]
fn main_world_audio_settings_short_landscape_uses_its_compact_scroll_layout() {
    let profile = main_world_short_landscape_stress_profile();
    let effective = UiDocument::parse_and_validate_json(MAIN_WORLD_AUDIO_SETTINGS_DOCUMENT_SOURCE)
        .unwrap()
        .effective_document(&profile, &UiPageState::initial())
        .unwrap();

    assert!(
        effective
            .applied_overrides
            .iter()
            .any(|item| item.source_id == "short_landscape")
    );
}

#[test]
fn audio_settings_actions_are_closed_to_exact_sources_and_typed_params() {
    let descriptors = audio_settings_action_descriptors();
    assert_eq!(descriptors.len(), 3);
    let volume = descriptors
        .iter()
        .find(|descriptor| descriptor.id.as_str() == ACTION_SET_VOLUME)
        .unwrap();
    assert_eq!(volume.sources.len(), 5);
    assert_eq!(volume.params.len(), 2);
    assert!(volume.params.contains_key("bus"));
    assert!(volume.params.contains_key("value"));

    let muted = descriptors
        .iter()
        .find(|descriptor| descriptor.id.as_str() == ACTION_SET_MASTER_MUTED)
        .unwrap();
    assert_eq!(muted.sources.len(), 1);
    assert_eq!(muted.params.len(), 1);

    let return_lobby = descriptors
        .iter()
        .find(|descriptor| descriptor.id.as_str() == ACTION_RETURN_LOBBY)
        .unwrap();
    assert_eq!(return_lobby.sources.len(), 1);
    assert!(return_lobby.params.is_empty());
}

#[test]
fn volume_action_revalidates_source_bus_and_clamps_forged_values() {
    let mut app = audio_action_test_app(true);
    app.world_mut().write_message(audio_dispatch(
        ACTION_SET_VOLUME,
        "audio_settings.volume.music",
        BTreeMap::from([
            ("bus".to_owned(), UiActionValue::Enum("music".to_owned())),
            ("value".to_owned(), UiActionValue::Number(140.0)),
        ]),
    ));
    app.update();
    assert_eq!(
        read_messages::<AudioCommand>(&app),
        vec![AudioCommand::SetBusVolume(AudioBusVolumeCommand::new(
            AudioBus::Music,
            1.0,
        ))]
    );

    for (source, bus, value) in [
        ("audio_settings.volume.music", "battle", 25.0),
        ("audio_settings.volume.unknown", "music", 25.0),
        ("audio_settings.volume.music", "music", f64::NAN),
    ] {
        let mut rejected = audio_action_test_app(true);
        rejected.world_mut().write_message(audio_dispatch(
            ACTION_SET_VOLUME,
            source,
            BTreeMap::from([
                ("bus".to_owned(), UiActionValue::Enum(bus.to_owned())),
                ("value".to_owned(), UiActionValue::Number(value)),
            ]),
        ));
        rejected.update();
        assert!(read_messages::<AudioCommand>(&rejected).is_empty());
    }
}

#[test]
fn unavailable_mixer_rejects_setting_actions_but_keeps_navigation_available() {
    let mut app = audio_action_test_app(false);
    app.world_mut().write_message(audio_dispatch(
        ACTION_SET_MASTER_MUTED,
        "audio_settings.master.muted",
        BTreeMap::from([("muted".to_owned(), UiActionValue::Bool(true))]),
    ));
    app.world_mut().write_message(audio_dispatch(
        ACTION_RETURN_LOBBY,
        "audio_settings.return_lobby",
        BTreeMap::new(),
    ));
    app.update();

    assert!(read_messages::<AudioCommand>(&app).is_empty());
    assert!(
        read_messages::<GameRouteCommand>(&app)
            .iter()
            .any(|command| { matches!(command, GameRouteCommand::ChangeMode(AppUiMode::Lobby)) })
    );
}

#[test]
fn main_world_close_action_closes_only_the_scene_panel() {
    let mut app = audio_action_test_app(true);
    app.world_mut().write_message(main_world_audio_dispatch(
        ACTION_MAIN_WORLD_CLOSE,
        "audio_settings.return_lobby",
        BTreeMap::new(),
    ));
    app.update();

    assert!(read_messages::<GameRouteCommand>(&app).is_empty());
    assert!(matches!(
        read_messages::<DeclarativeScreenHostCommand>(&app).as_slice(),
        [DeclarativeScreenHostCommand::CloseRoute { route }]
            if route == MAIN_WORLD_SETTINGS_ROUTE
    ));
}

#[test]
fn master_mute_action_uses_the_explicit_control_value() {
    let mut app = audio_action_test_app(true);
    app.world_mut().write_message(audio_dispatch(
        ACTION_SET_MASTER_MUTED,
        "audio_settings.master.muted",
        BTreeMap::from([("muted".to_owned(), UiActionValue::Bool(true))]),
    ));
    app.update();

    assert_eq!(
        read_messages::<AudioCommand>(&app),
        vec![AudioCommand::SetBusMuted(AudioBusMutedCommand::new(
            AudioBus::Master,
            true,
        ))]
    );
}

#[test]
fn mixer_external_changes_are_clamped_and_synchronized_as_authoritative_values() {
    let mut mixer = AudioMixer::default();
    mixer.set_bus_volume(AudioBus::Music, 0.37);
    mixer.set_bus_muted(AudioBus::Master, true);
    mixer.buses.insert(
        AudioBus::Battle,
        AudioBusState {
            volume: f32::NAN,
            muted: false,
            paused: false,
        },
    );
    let mut app = audio_binding_test_app(Some(mixer));
    app.update();

    assert_eq!(
        binding_value(&app, "audio_settings.volume.music"),
        UiBindingValue::Number(37.0)
    );
    assert_eq!(
        binding_value(&app, "audio_settings.volume.battle"),
        UiBindingValue::Number(0.0)
    );
    assert_eq!(
        binding_value(&app, "audio_settings.master.muted"),
        UiBindingValue::Bool(true)
    );
    assert_eq!(
        binding_value(&app, "audio_settings.view_state"),
        UiBindingValue::Enum("ready".to_owned())
    );

    app.world_mut()
        .resource_mut::<AudioMixer>()
        .set_bus_volume(AudioBus::Music, 0.82);
    app.update();
    assert_eq!(
        binding_value(&app, "audio_settings.volume.music"),
        UiBindingValue::Number(82.0)
    );
}

#[test]
fn missing_mixer_produces_disabled_unavailable_error_bindings() {
    let mut app = audio_binding_test_app(None);
    app.update();
    assert_eq!(
        binding_value(&app, "audio_settings.controls.disabled"),
        UiBindingValue::Bool(true)
    );
    assert_eq!(
        binding_value(&app, "audio_settings.view_state"),
        UiBindingValue::Enum("unavailable".to_owned())
    );
    assert_eq!(
        binding_value(&app, "audio_settings.error.visibility"),
        UiBindingValue::Visibility(crate::framework::ui::document::UiBindingVisibility::Visible)
    );
}

#[test]
fn audio_settings_reload_keeps_focus_scroll_and_reapplies_authoritative_value() {
    let mut app = audio_runtime_test_app();
    app.world_mut()
        .resource_mut::<NextState<AppUiMode>>()
        .set(AppUiMode::AudioSettings);
    update_frames(&mut app, 8);

    let (scroll, toggle, slider) = runtime_nodes(&app);
    app.world_mut().get_mut::<ScrollPosition>(scroll).unwrap().0 = Vec2::new(0.0, 61.0);
    app.world_mut()
        .resource_mut::<UiFocusState>()
        .focused_entity = Some(toggle);
    app.world_mut().get_mut::<UiSlider>(slider).unwrap().value = 73.0;

    let mut changed: serde_json::Value =
        serde_json::from_str(AUDIO_SETTINGS_DOCUMENT_SOURCE).unwrap();
    changed
        .pointer_mut("/root/children/0/component/children/0/children")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .swap(2, 3);
    app.world_mut()
        .write_message(DeclarativeScreenHostCommand::ReloadRouteSource {
            route: "audio_settings".to_owned(),
            source_json: serde_json::to_string(&changed).unwrap(),
        });
    update_frames(&mut app, 8);

    let (new_scroll, new_toggle, new_slider) = runtime_nodes(&app);
    assert_eq!(
        app.world().get::<ScrollPosition>(new_scroll).unwrap().0,
        Vec2::new(0.0, 61.0)
    );
    assert_eq!(
        app.world().resource::<UiFocusState>().focused_entity,
        Some(new_toggle)
    );
    assert_eq!(app.world().get::<UiSlider>(new_slider).unwrap().value, 41.0);
}

#[test]
fn audio_settings_scroll_content_remains_reachable() {
    let mut app = audio_runtime_test_app();
    app.world_mut()
        .resource_mut::<NextState<AppUiMode>>()
        .set(AppUiMode::AudioSettings);
    update_frames(&mut app, 8);

    let (scroll, _, _) = runtime_nodes(&app);
    assert_eq!(
        app.world().get::<Node>(scroll).unwrap().overflow.y,
        OverflowAxis::Scroll
    );
}

#[test]
fn cleanup_clears_page_focus() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<UiFocusState>()
        .add_systems(Update, cleanup_audio_settings_screen);
    let focused = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<UiFocusState>()
        .focused_entity = Some(focused);
    app.update();
    assert_eq!(app.world().resource::<UiFocusState>().focused_entity, None);
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
#[test]
fn audit_fixture_maps_four_profiles_to_external_boundary_disabled_and_error_states() {
    for (viewport, expected) in [
        (
            UiViewport {
                logical_width: 1280.0,
                logical_height: 720.0,
                device_scale: 1.0,
                ..default()
            },
            "external",
        ),
        (
            UiViewport {
                logical_width: 800.0,
                logical_height: 360.0,
                device_scale: 2.0,
                ..default()
            },
            "boundary",
        ),
        (
            UiViewport {
                logical_width: 800.0,
                logical_height: 360.0,
                device_scale: 3.0,
                ..default()
            },
            "disabled",
        ),
        (
            UiViewport {
                logical_width: 1280.0,
                logical_height: 800.0,
                device_scale: 2.0,
                ..default()
            },
            "unavailable",
        ),
    ] {
        let fixture = audio_settings_audit_fixture_for_viewport(&viewport);
        assert!(fixture.active);
        match expected {
            "external" => assert_eq!(fixture.volumes, Some([72.0, 41.0, 63.0, 58.0, 80.0])),
            "boundary" => {
                assert_eq!(fixture.volumes, Some([-10.0, 0.0, 50.0, 100.0, 140.0]));
                assert_eq!(fixture.master_muted, Some(true));
            }
            "disabled" => assert!(fixture.force_disabled),
            "unavailable" => assert!(fixture.unavailable),
            _ => unreachable!(),
        }
    }
}

fn visit_nodes(node: &UiNode, visitor: &mut impl FnMut(&UiNode)) {
    visitor(node);
    for child in node.children() {
        visit_nodes(child, visitor);
    }
}

fn audio_action_test_app(with_mixer: bool) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<UiActionDispatch>()
        .add_message::<AudioCommand>()
        .add_message::<GameRouteCommand>()
        .add_message::<DeclarativeScreenHostCommand>()
        .init_resource::<UiFocusState>()
        .add_systems(Update, handle_audio_settings_document_actions);
    if with_mixer {
        app.init_resource::<AudioMixer>();
    }
    app
}

fn audio_dispatch(
    action: &str,
    source: &str,
    params: BTreeMap<String, UiActionValue>,
) -> UiActionDispatch {
    UiActionDispatch {
        action: UiActionId::from_str(action).unwrap(),
        document_id: UiDocumentId::from_str(AUDIO_SETTINGS_DOCUMENT_ID).unwrap(),
        owner: OWNER_AUDIO_SETTINGS.as_str().to_owned(),
        source_node: UiNodeId::from_str(source).unwrap(),
        kind: UiRegisteredActionKind::BusinessCommand {
            target: action.to_owned(),
        },
        params,
    }
}

fn main_world_audio_dispatch(
    action: &str,
    source: &str,
    params: BTreeMap<String, UiActionValue>,
) -> UiActionDispatch {
    UiActionDispatch {
        action: UiActionId::from_str(action).unwrap(),
        document_id: UiDocumentId::from_str(MAIN_WORLD_AUDIO_SETTINGS_DOCUMENT_ID).unwrap(),
        owner: OWNER_MAIN_WORLD_SETTINGS_PANEL.as_str().to_owned(),
        source_node: UiNodeId::from_str(source).unwrap(),
        kind: UiRegisteredActionKind::BusinessCommand {
            target: action.to_owned(),
        },
        params,
    }
}

fn audio_binding_test_app(mixer: Option<AudioMixer>) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<UiBindingValues>()
        .init_resource::<AudioSettingsHostContract>()
        .add_systems(Update, sync_audio_settings_document_bindings);
    if let Some(mixer) = mixer {
        app.insert_resource(mixer);
    }
    app
}

fn binding_value(app: &App, path: &str) -> UiBindingValue {
    let path = UiBindingPath::from_str(path).unwrap();
    let declaration = app
        .world()
        .resource::<AudioSettingsHostContract>()
        .bindings
        .get(&path)
        .unwrap();
    app.world()
        .resource::<UiBindingValues>()
        .scoped_value(
            AUDIO_SETTINGS_DOCUMENT_ID,
            OWNER_AUDIO_SETTINGS.as_str(),
            &path,
            declaration,
        )
        .unwrap()
}

fn audio_runtime_test_app() -> App {
    let mut mixer = AudioMixer::default();
    mixer.set_bus_volume(AudioBus::Music, 0.41);
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin))
        .init_state::<AppUiMode>()
        .insert_resource(UiTheme::default())
        .insert_resource(UiMetrics::default())
        .insert_resource(UiFontAssets::test_registry())
        .init_resource::<UiFocusState>()
        .init_resource::<UiViewport>()
        .init_resource::<AudioSettingsHostContract>()
        .insert_resource(mixer)
        .add_plugins((
            UiDocumentRuntimePlugin,
            UiDocumentPreviewPlugin,
            DeclarativeScreenHostPlugin,
        ))
        .add_systems(Startup, register_audio_settings_contract)
        .add_systems(Update, sync_audio_settings_document_bindings);
    app
}

fn runtime_nodes(app: &App) -> (Entity, Entity, Entity) {
    let runtime = app.world().resource::<UiDocumentRuntime>();
    let instance = runtime
        .active_instance(
            OWNER_AUDIO_SETTINGS.as_str(),
            &UiDocumentId::from_str(AUDIO_SETTINGS_DOCUMENT_ID).unwrap(),
        )
        .expect("Audio Settings document should be active");
    let node = |id: &str| {
        runtime
            .node_entity(instance, &UiNodeId::from_str(id).unwrap())
            .unwrap()
    };
    (
        node("audio_settings.scroll"),
        node("audio_settings.master.muted"),
        node("audio_settings.volume.music"),
    )
}

fn main_world_audit_profiles() -> [UiTargetProfile; 4] {
    // UiTargetProfile stores runner logical geometry only. The two 800x360
    // phone captures differ by physical scale (2x and 3x) in run-ui-audit.
    [
        UiTargetProfile::new(
            1280.0,
            720.0,
            UiSafeAreaClass::None,
            UiDocumentInputMode::MouseKeyboard,
            UiDocumentPlatform::Windows,
        )
        .unwrap(),
        UiTargetProfile::new(
            800.0,
            360.0,
            UiSafeAreaClass::Inset,
            UiDocumentInputMode::Touch,
            UiDocumentPlatform::Android,
        )
        .unwrap(),
        UiTargetProfile::new(
            800.0,
            360.0,
            UiSafeAreaClass::Inset,
            UiDocumentInputMode::Touch,
            UiDocumentPlatform::Android,
        )
        .unwrap(),
        UiTargetProfile::new(
            1280.0,
            800.0,
            UiSafeAreaClass::Inset,
            UiDocumentInputMode::Touch,
            UiDocumentPlatform::Android,
        )
        .unwrap(),
    ]
}

fn main_world_short_landscape_stress_profile() -> UiTargetProfile {
    UiTargetProfile::new(
        800.0,
        360.0,
        UiSafeAreaClass::Inset,
        UiDocumentInputMode::Touch,
        UiDocumentPlatform::Android,
    )
    .unwrap()
}

fn find_document_node<'a>(node: &'a UiNode, id: &str) -> Option<&'a UiNode> {
    if node.id().as_str() == id {
        return Some(node);
    }
    node.children()
        .iter()
        .find_map(|child| find_document_node(child, id))
}

fn update_frames(app: &mut App, count: usize) {
    for _ in 0..count {
        app.update();
    }
}

fn read_messages<M>(app: &App) -> Vec<M>
where
    M: Message + Clone,
{
    let messages = app.world().resource::<Messages<M>>();
    let mut cursor = MessageCursor::default();
    cursor.read(messages).cloned().collect()
}
