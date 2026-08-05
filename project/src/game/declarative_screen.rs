use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

use bevy::{prelude::*, window::AppLifecycle};

use crate::{
    framework::ui::{
        audit::{
            UiAuditCaptureRecipe, UiAuditReadyCondition, UiAuditRecipe, UiAuditScreen,
            UiAuditScreenRecipe, UiAuditScreenRegistry,
        },
        core::{
            UiCurrentOwner, UiDocumentCloseTopRequest, UiOwnerId, UiPanelCommand, UiPanelSystems,
            binding::UiBindingValues, focus::UiFocusState,
        },
        document::{
            UiActionId, UiActionRegistry, UiBindingScope, UiBindingType, UiDocument,
            UiDocumentHostValidationContext, UiDocumentId, UiDocumentLayer,
            UiDocumentPreviewCommand, UiDocumentPreviewRegistration, UiDocumentPreviewSystems,
            UiDocumentReloadEvent, UiDocumentReloadStatus, UiDocumentRuntime,
            UiDocumentRuntimeCommand, UiDocumentSourcePath, UiDocumentSourceRoot, UiHostBindingKey,
            UiNodeId, UiPageState, UiTargetProfile, target_profile_from_viewport,
        },
    },
    game::{
        myserver::mail::MailClientState,
        navigation::AppUiMode,
        scenes::main_world_entry::{MainWorldEntryPhase, MainWorldEntryState},
        ui_ids::{
            OWNER_DECLARATIVE_DOCUMENT_ROUTE, OWNER_UI_APPROVED_BUSINESS_ACCEPTANCE,
            OWNER_UI_DOCUMENT_GALLERY, OWNER_UI_GENERATED_ACCEPTANCE,
        },
    },
};

const UI_DOCUMENT_GALLERY_SOURCE: &str =
    include_str!("../../assets/ui/documents/approved/gallery/declarative_gallery.v1.json");
const GENERATED_ACCEPTANCE_SOURCE: &str = include_str!(
    "../../assets/ui/documents/approved/generated_acceptance_fixture/document.v1.json"
);
const DECLARATIVE_PILOT_SOURCE: &str =
    include_str!("../../assets/ui/documents/approved/pilot/declarative_pilot.v1.json");
const APPROVED_BUSINESS_ACCEPTANCE_SOURCE: &str =
    include_str!("../../assets/ui/documents/approved/business_acceptance_fixture/document.v1.json");

const DEFAULT_AUDIT_PROFILES: [&str; 4] = [
    "desktop",
    "phone-landscape",
    "phone-1080p-landscape",
    "tablet-landscape",
];
const DOCUMENT_AUDIT_CAPTURES: &[UiAuditCaptureRecipe] = &[UiAuditCaptureRecipe::initial()];

#[derive(Clone, Debug)]
pub(in crate::game) struct DeclarativeScreenSource {
    pub source_path: UiDocumentSourcePath,
    pub source_json: String,
    pub watch: bool,
}

impl DeclarativeScreenSource {
    pub fn approved(relative_path: &str, source_json: impl Into<String>) -> Self {
        Self::new(
            UiDocumentSourceRoot::Approved,
            relative_path,
            source_json,
            false,
        )
    }

    #[allow(dead_code)]
    pub fn authoring(relative_path: &str, source_json: impl Into<String>) -> Self {
        Self::new(
            UiDocumentSourceRoot::Authoring,
            relative_path,
            source_json,
            true,
        )
    }

    /// Content-cache payloads use the same host contract as packaged and authoring documents.
    /// The cache client is added later; this source deliberately does not enable file watch.
    #[allow(dead_code)]
    pub fn content_cache(relative_path: &str, source_json: impl Into<String>) -> Self {
        Self::new(
            UiDocumentSourceRoot::ContentCache,
            relative_path,
            source_json,
            false,
        )
    }

    fn new(
        root: UiDocumentSourceRoot,
        relative_path: &str,
        source_json: impl Into<String>,
        watch: bool,
    ) -> Self {
        Self {
            source_path: UiDocumentSourcePath::new(root, relative_path)
                .expect("game-owned declarative document source paths must be safe"),
            source_json: source_json.into(),
            watch,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::game) enum DeclarativeScreenFailurePolicy {
    /// A transactional reload keeps the last committed instance whenever one exists.
    RetainPrevious,
    /// Retry the supplied packaged fallback before allowing a missing page to become visible.
    PackagedFallback,
    /// Render the supplied same-contract error document if no instance is available.
    #[allow(dead_code)]
    ControlledError,
}

#[derive(Clone, Debug)]
pub(in crate::game) struct DeclarativeScreenHost {
    pub document_id: UiDocumentId,
    pub route: &'static str,
    pub route_aliases: &'static [&'static str],
    pub mode: Option<AppUiMode>,
    pub owner: UiOwnerId,
    pub panel: crate::framework::ui::document::UiDocumentPanel,
    pub layer: UiDocumentLayer,
    pub initial_state: UiPageState,
    pub binding_schema: BTreeMap<UiHostBindingKey, UiBindingType>,
    pub action_allowlist: BTreeSet<UiActionId>,
    pub audit_profiles: Vec<String>,
    pub source: DeclarativeScreenSource,
    pub fallback_source: Option<DeclarativeScreenSource>,
    pub failure_policy: DeclarativeScreenFailurePolicy,
}

impl DeclarativeScreenHost {
    fn key(&self) -> DeclarativeScreenHostKey {
        DeclarativeScreenHostKey {
            document_id: self.document_id.clone(),
            owner: self.owner.as_str().to_owned(),
        }
    }

    fn matches_route(&self, value: &str) -> bool {
        self.route.eq_ignore_ascii_case(value.trim())
            || self
                .route_aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(value.trim()))
    }

    fn preview_registration(
        &self,
        source: &DeclarativeScreenSource,
        target_profile: UiTargetProfile,
    ) -> UiDocumentPreviewRegistration {
        UiDocumentPreviewRegistration {
            document_id: self.document_id.clone(),
            owner: self.owner.as_str().to_owned(),
            source_path: source.source_path.clone(),
            source_json: source.source_json.clone(),
            panel: self.panel,
            layer: self.layer,
            target_profile,
            page_state: self.initial_state.clone(),
            owner_alive: true,
            host_bindings: self.binding_schema.clone(),
            watch: source.watch,
            open_on_register: true,
            audit_profiles: self.audit_profiles.clone(),
            approval_audit: None,
        }
    }
}

#[derive(Clone, Debug, Resource)]
pub(in crate::game) struct DeclarativeScreenRegistry {
    hosts: Vec<DeclarativeScreenHost>,
}

impl DeclarativeScreenRegistry {
    pub fn register(&mut self, host: DeclarativeScreenHost) -> Result<(), &'static str> {
        if self
            .hosts
            .iter()
            .any(|candidate| candidate.route == host.route)
        {
            return Err("UI_DECLARATIVE_SCREEN_ROUTE_DUPLICATE");
        }
        if host.mode.is_some_and(|mode| {
            self.hosts
                .iter()
                .any(|candidate| candidate.mode == Some(mode))
        }) {
            return Err("UI_DECLARATIVE_SCREEN_MODE_DUPLICATE");
        }
        if self.hosts.iter().any(|candidate| {
            candidate.document_id == host.document_id && candidate.owner == host.owner
        }) {
            return Err("UI_DECLARATIVE_SCREEN_DOCUMENT_OWNER_DUPLICATE");
        }
        self.hosts.push(host);
        Ok(())
    }

    pub fn route(&self, value: &str) -> Option<&DeclarativeScreenHost> {
        self.hosts.iter().find(|host| host.matches_route(value))
    }

    fn mode(&self, mode: AppUiMode) -> Option<&DeclarativeScreenHost> {
        self.hosts.iter().find(|host| host.mode == Some(mode))
    }

    fn host_for_key(&self, key: &DeclarativeScreenHostKey) -> Option<&DeclarativeScreenHost> {
        self.hosts.iter().find(|host| host.key() == *key)
    }

    fn hosts(&self) -> impl Iterator<Item = &DeclarativeScreenHost> {
        self.hosts.iter()
    }
}

impl FromIterator<DeclarativeScreenHost> for DeclarativeScreenRegistry {
    fn from_iter<T: IntoIterator<Item = DeclarativeScreenHost>>(iter: T) -> Self {
        let mut registry = Self { hosts: Vec::new() };
        for host in iter {
            registry
                .register(host)
                .expect("declarative screen test registrations must be unique");
        }
        registry
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct DeclarativeScreenHostKey {
    document_id: UiDocumentId,
    owner: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::game) enum DeclarativeScreenFailureDecision {
    RetainedPrevious,
    LoadingPackagedFallback,
    LoadingControlledError,
    NoFallbackAvailable,
}

#[derive(Clone, Debug, Message, PartialEq)]
pub(in crate::game) enum DeclarativeScreenHostEvent {
    Opened {
        route: String,
        document_id: UiDocumentId,
        owner: String,
    },
    Closed {
        route: String,
        document_id: UiDocumentId,
        owner: String,
    },
    LoadFailed {
        code: String,
        cause: String,
        route: String,
        document_id: UiDocumentId,
        owner: String,
        decision: DeclarativeScreenFailureDecision,
    },
}

#[derive(Clone, Debug, Message)]
pub(in crate::game) enum DeclarativeScreenHostCommand {
    /// Opens a data-registered pure document route and takes over from the current UI owner only
    /// after the new document commits.
    OpenRoute { route: String },
    /// Opens an independently owned host. Fixed game modes use this lifecycle path.
    #[allow(dead_code)]
    OpenDetachedRoute { route: String },
    #[allow(dead_code)]
    CloseRoute { route: String },
    #[allow(dead_code)]
    ReloadRoute { route: String },
    /// Used by a verified future cache client and tests. The route selects the fixed host
    /// contract; source JSON never supplies a route, owner, binding, or action capability.
    #[allow(dead_code)]
    ReloadRouteSource { route: String, source_json: String },
}

#[derive(Clone, Debug, Default, Resource)]
struct DeclarativeScreenHostRuntime {
    open: BTreeMap<DeclarativeScreenHostKey, &'static str>,
    open_order: Vec<DeclarativeScreenHostKey>,
    fallback_attempted: BTreeSet<DeclarativeScreenHostKey>,
    pending_owner_switch: BTreeMap<DeclarativeScreenHostKey, Option<UiOwnerId>>,
    pending_replacements: BTreeMap<DeclarativeScreenHostKey, Vec<DeclarativeScreenHostKey>>,
    observed_mode: Option<AppUiMode>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
enum DeclarativeScreenHostSystems {
    Commands,
    Reloads,
}

pub(in crate::game) struct DeclarativeScreenHostPlugin;

impl Plugin for DeclarativeScreenHostPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DeclarativeScreenRegistry>()
            .init_resource::<DeclarativeScreenHostRuntime>()
            .init_resource::<UiCurrentOwner>()
            .add_message::<AppLifecycle>()
            .add_message::<UiPanelCommand>()
            .add_message::<UiDocumentCloseTopRequest>()
            .add_message::<DeclarativeScreenHostCommand>()
            .add_message::<DeclarativeScreenHostEvent>()
            .configure_sets(
                Update,
                DeclarativeScreenHostSystems::Commands.before(UiDocumentPreviewSystems::Commands),
            )
            .configure_sets(
                Update,
                DeclarativeScreenHostSystems::Reloads
                    .after(UiDocumentPreviewSystems::FinishReloads),
            )
            .add_systems(
                Update,
                (sync_mode_host, handle_host_commands)
                    .chain()
                    .in_set(DeclarativeScreenHostSystems::Commands),
            )
            .add_systems(
                Update,
                close_top_declarative_document
                    .after(UiPanelSystems::Commands)
                    .before(DeclarativeScreenHostSystems::Commands),
            )
            .add_systems(
                Update,
                (handle_document_reload_events, restore_hosts_after_resume)
                    .chain()
                    .in_set(DeclarativeScreenHostSystems::Reloads),
            );
    }
}

impl Default for DeclarativeScreenRegistry {
    fn default() -> Self {
        [
            DeclarativeScreenHost {
                document_id: UiDocumentId::from_str("gallery.declarative")
                    .expect("static document ID is valid"),
                route: "ui_document_gallery",
                route_aliases: &[
                    "ui_document_gallery",
                    "ui-document-gallery",
                    "document_gallery",
                    "document-gallery",
                    "declarative_gallery",
                ],
                mode: Some(AppUiMode::UiDocumentGallery),
                owner: OWNER_UI_DOCUMENT_GALLERY,
                panel: crate::framework::ui::document::UiDocumentPanel::Page,
                layer: UiDocumentLayer::Page,
                initial_state: UiPageState::initial(),
                binding_schema: BTreeMap::new(),
                action_allowlist: [
                    UiActionId::from_str("gallery.set_status").expect("static action ID is valid"),
                    UiActionId::from_str("gallery.control_changed")
                        .expect("static action ID is valid"),
                ]
                .into_iter()
                .collect(),
                audit_profiles: DEFAULT_AUDIT_PROFILES.map(str::to_owned).to_vec(),
                source: DeclarativeScreenSource::approved(
                    "gallery/declarative_gallery.v1.json",
                    UI_DOCUMENT_GALLERY_SOURCE,
                ),
                fallback_source: None,
                failure_policy: DeclarativeScreenFailurePolicy::RetainPrevious,
            },
            DeclarativeScreenHost {
                document_id: UiDocumentId::from_str("generated.acceptance_fixture")
                    .expect("static document ID is valid"),
                route: "ui_generated_acceptance",
                route_aliases: &[
                    "ui_generated_acceptance",
                    "ui-generated-acceptance",
                    "generated_acceptance",
                ],
                mode: Some(AppUiMode::UiGeneratedAcceptance),
                owner: OWNER_UI_GENERATED_ACCEPTANCE,
                panel: crate::framework::ui::document::UiDocumentPanel::Page,
                layer: UiDocumentLayer::Page,
                initial_state: UiPageState::initial(),
                binding_schema: BTreeMap::new(),
                action_allowlist: BTreeSet::new(),
                audit_profiles: DEFAULT_AUDIT_PROFILES.map(str::to_owned).to_vec(),
                source: DeclarativeScreenSource::approved(
                    "generated_acceptance_fixture/document.v1.json",
                    GENERATED_ACCEPTANCE_SOURCE,
                ),
                fallback_source: None,
                failure_policy: DeclarativeScreenFailurePolicy::RetainPrevious,
            },
            DeclarativeScreenHost {
                document_id: UiDocumentId::from_str("approved.business_acceptance")
                    .expect("static document ID is valid"),
                route: "ui_approved_business_acceptance",
                route_aliases: &[
                    "ui_approved_business_acceptance",
                    "ui-approved-business-acceptance",
                    "approved_business_acceptance",
                ],
                mode: None,
                owner: OWNER_UI_APPROVED_BUSINESS_ACCEPTANCE,
                panel: crate::framework::ui::document::UiDocumentPanel::Page,
                layer: UiDocumentLayer::Page,
                initial_state: UiPageState::initial(),
                binding_schema: BTreeMap::from([(
                    UiHostBindingKey::new(
                        UiBindingScope::Owner,
                        crate::framework::ui::document::UiBindingPath::from_str(
                            "acceptance.status",
                        )
                        .expect("static binding path is valid"),
                    ),
                    UiBindingType::String,
                )]),
                action_allowlist: [UiActionId::from_str("approved.acceptance_continue")
                    .expect("static action ID is valid")]
                .into_iter()
                .collect(),
                audit_profiles: DEFAULT_AUDIT_PROFILES.map(str::to_owned).to_vec(),
                source: DeclarativeScreenSource::approved(
                    "business_acceptance_fixture/document.v1.json",
                    APPROVED_BUSINESS_ACCEPTANCE_SOURCE,
                ),
                fallback_source: None,
                failure_policy: DeclarativeScreenFailurePolicy::RetainPrevious,
            },
            DeclarativeScreenHost {
                document_id: UiDocumentId::from_str("game.declarative_pilot")
                    .expect("static document ID is valid"),
                route: "ui_declarative_pilot",
                route_aliases: &[
                    "ui_declarative_pilot",
                    "ui-declarative-pilot",
                    "declarative_pilot",
                ],
                mode: None,
                owner: OWNER_DECLARATIVE_DOCUMENT_ROUTE,
                panel: crate::framework::ui::document::UiDocumentPanel::Page,
                layer: UiDocumentLayer::Page,
                initial_state: UiPageState::initial(),
                binding_schema: BTreeMap::new(),
                action_allowlist: BTreeSet::new(),
                audit_profiles: DEFAULT_AUDIT_PROFILES.map(str::to_owned).to_vec(),
                source: DeclarativeScreenSource::approved(
                    "pilot/declarative_pilot.v1.json",
                    DECLARATIVE_PILOT_SOURCE,
                ),
                fallback_source: Some(DeclarativeScreenSource::approved(
                    "pilot/declarative_pilot.v1.json",
                    DECLARATIVE_PILOT_SOURCE,
                )),
                failure_policy: DeclarativeScreenFailurePolicy::PackagedFallback,
            },
        ]
        .into_iter()
        .collect()
    }
}

fn sync_mode_host(
    state: Option<Res<State<AppUiMode>>>,
    main_world_entry: Option<Res<MainWorldEntryState>>,
    registry: Res<DeclarativeScreenRegistry>,
    actions: Res<UiActionRegistry>,
    runtime: Res<UiDocumentRuntime>,
    viewport: Option<Res<crate::framework::ui::core::UiViewport>>,
    mut host_runtime: ResMut<DeclarativeScreenHostRuntime>,
    mut preview_commands: MessageWriter<UiDocumentPreviewCommand>,
    mut runtime_commands: MessageWriter<UiDocumentRuntimeCommand>,
    mut host_events: MessageWriter<DeclarativeScreenHostEvent>,
) {
    let Some(state) = state else {
        return;
    };
    let current_mode = *state.get();
    let registered_current_host = registry.mode(current_mode).cloned();
    let current_host = mode_host_is_eligible(current_mode, main_world_entry.as_deref())
        .then_some(registered_current_host.clone())
        .flatten();
    if host_runtime.observed_mode == Some(current_mode)
        && current_host
            .as_ref()
            .is_some_and(|host| host_runtime.open.contains_key(&host.key()))
    {
        return;
    }

    if host_runtime.observed_mode != Some(current_mode)
        && let Some(previous_mode) = host_runtime.observed_mode
        && let Some(host) = registry.mode(previous_mode).cloned()
    {
        close_host(
            &host,
            &mut host_runtime,
            &mut preview_commands,
            &mut runtime_commands,
            &mut host_events,
        );
    } else if current_host.is_none()
        && let Some(host) = registered_current_host
    {
        close_host(
            &host,
            &mut host_runtime,
            &mut preview_commands,
            &mut runtime_commands,
            &mut host_events,
        );
    }
    if let Some(host) = current_host {
        let Some(viewport) = viewport.as_deref() else {
            return;
        };
        open_host(
            &host,
            false,
            None,
            &actions,
            &runtime,
            viewport,
            &mut host_runtime,
            &mut preview_commands,
            &mut host_events,
        );
    }
    host_runtime.observed_mode = Some(current_mode);
}

fn mode_host_is_eligible(mode: AppUiMode, main_world_entry: Option<&MainWorldEntryState>) -> bool {
    mode != AppUiMode::MainWorld
        || main_world_entry.is_some_and(|entry| entry.phase == MainWorldEntryPhase::Active)
}

#[allow(clippy::too_many_arguments)]
fn handle_host_commands(
    mut commands: MessageReader<DeclarativeScreenHostCommand>,
    registry: Res<DeclarativeScreenRegistry>,
    actions: Res<UiActionRegistry>,
    runtime: Res<UiDocumentRuntime>,
    current_owner: Res<UiCurrentOwner>,
    state: Option<Res<State<AppUiMode>>>,
    viewport: Option<Res<crate::framework::ui::core::UiViewport>>,
    mut host_runtime: ResMut<DeclarativeScreenHostRuntime>,
    mut preview_commands: MessageWriter<UiDocumentPreviewCommand>,
    mut runtime_commands: MessageWriter<UiDocumentRuntimeCommand>,
    mut host_events: MessageWriter<DeclarativeScreenHostEvent>,
) {
    let Some(viewport) = viewport.as_deref() else {
        return;
    };
    for command in commands.read().cloned() {
        match &command {
            DeclarativeScreenHostCommand::OpenRoute { route }
            | DeclarativeScreenHostCommand::OpenDetachedRoute { route } => {
                let take_ownership =
                    matches!(command, DeclarativeScreenHostCommand::OpenRoute { .. });
                let previous_owner = current_owner
                    .owner
                    .or_else(|| state.as_deref().map(|state| state.get().ui_owner()));
                let Some(host) = registry.route(&route).cloned() else {
                    host_events.write(DeclarativeScreenHostEvent::LoadFailed {
                        code: "UI_DECLARATIVE_SCREEN_ROUTE_UNKNOWN".to_owned(),
                        cause: "route is not registered by the game host".to_owned(),
                        route: route.clone(),
                        document_id: UiDocumentId::from_str("game.declarative_pilot")
                            .expect("static fallback ID is valid"),
                        owner: String::new(),
                        decision: DeclarativeScreenFailureDecision::NoFallbackAvailable,
                    });
                    continue;
                };
                open_host(
                    &host,
                    take_ownership,
                    previous_owner,
                    &actions,
                    &runtime,
                    viewport,
                    &mut host_runtime,
                    &mut preview_commands,
                    &mut host_events,
                );
            }
            DeclarativeScreenHostCommand::CloseRoute { route } => {
                let Some(host) = registry.route(&route).cloned() else {
                    continue;
                };
                close_host(
                    &host,
                    &mut host_runtime,
                    &mut preview_commands,
                    &mut runtime_commands,
                    &mut host_events,
                );
            }
            DeclarativeScreenHostCommand::ReloadRoute { route } => {
                let Some(host) = registry.route(&route) else {
                    continue;
                };
                let key = host.key();
                if host_runtime.open.contains_key(&key) {
                    preview_commands.write(UiDocumentPreviewCommand::Reload {
                        reload_id: next_reload_id(&key),
                        document_id: host.document_id.clone(),
                        owner: host.owner.as_str().to_owned(),
                    });
                }
            }
            DeclarativeScreenHostCommand::ReloadRouteSource { route, source_json } => {
                let Some(host) = registry.route(&route).cloned() else {
                    continue;
                };
                let key = host.key();
                if !host_runtime.open.contains_key(&key) {
                    continue;
                }
                if let Err(cause) = validate_host_source(&host, &source_json, &actions) {
                    report_host_failure(
                        &host,
                        cause,
                        runtime
                            .active_instance(host.owner.as_str(), &host.document_id)
                            .is_some(),
                        &mut host_runtime,
                        &mut preview_commands,
                        viewport,
                        &mut host_events,
                    );
                    continue;
                }
                preview_commands.write(UiDocumentPreviewCommand::ReloadSource {
                    reload_id: next_reload_id(&key),
                    document_id: host.document_id.clone(),
                    owner: host.owner.as_str().to_owned(),
                    source_json: source_json.clone(),
                });
            }
        }
    }
}

fn close_top_declarative_document(
    mut requests: MessageReader<UiDocumentCloseTopRequest>,
    registry: Res<DeclarativeScreenRegistry>,
    mut host_runtime: ResMut<DeclarativeScreenHostRuntime>,
    mut preview_commands: MessageWriter<UiDocumentPreviewCommand>,
    mut runtime_commands: MessageWriter<UiDocumentRuntimeCommand>,
    mut host_events: MessageWriter<DeclarativeScreenHostEvent>,
    mut mail: Option<ResMut<MailClientState>>,
    mut focus: Option<ResMut<UiFocusState>>,
) {
    for _ in requests.read() {
        let Some(key) = host_runtime
            .open_order
            .iter()
            .rev()
            .find(|key| {
                registry.host_for_key(key).is_some_and(|host| {
                    matches!(
                        host.panel,
                        crate::framework::ui::document::UiDocumentPanel::Floating
                            | crate::framework::ui::document::UiDocumentPanel::Modal
                            | crate::framework::ui::document::UiDocumentPanel::BlockingOverlay
                    )
                })
            })
            .cloned()
        else {
            continue;
        };
        let Some(host) = registry.host_for_key(&key).cloned() else {
            continue;
        };
        if host.document_id.as_str() == "game.main_world_mail"
            && mail.as_deref().is_some_and(MailClientState::detail_is_open)
        {
            if let Some(mail) = mail.as_deref_mut() {
                mail.dismiss_detail();
            }
            if let Some(focus) = focus.as_deref_mut() {
                focus.focused_entity = None;
            }
            continue;
        }
        close_host(
            &host,
            &mut host_runtime,
            &mut preview_commands,
            &mut runtime_commands,
            &mut host_events,
        );
        if let Some(focus) = focus.as_deref_mut() {
            focus.focused_entity = None;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn open_host(
    host: &DeclarativeScreenHost,
    take_ownership: bool,
    current_owner: Option<UiOwnerId>,
    actions: &UiActionRegistry,
    runtime: &UiDocumentRuntime,
    viewport: &crate::framework::ui::core::UiViewport,
    host_runtime: &mut DeclarativeScreenHostRuntime,
    preview_commands: &mut MessageWriter<UiDocumentPreviewCommand>,
    host_events: &mut MessageWriter<DeclarativeScreenHostEvent>,
) {
    let key = host.key();
    if host_runtime.open.contains_key(&key) {
        return;
    }
    if take_ownership {
        host_runtime.pending_owner_switch.insert(
            key.clone(),
            current_owner.filter(|owner| *owner != host.owner),
        );
    }
    if let Err(cause) = validate_host_source(host, &host.source.source_json, actions) {
        if host.fallback_source.is_some()
            && matches!(
                host.failure_policy,
                DeclarativeScreenFailurePolicy::PackagedFallback
                    | DeclarativeScreenFailurePolicy::ControlledError
            )
        {
            host_runtime.open.insert(key, host.route);
        } else {
            host_runtime.pending_owner_switch.remove(&key);
        }
        report_host_failure(
            host,
            cause,
            runtime
                .active_instance(host.owner.as_str(), &host.document_id)
                .is_some(),
            host_runtime,
            preview_commands,
            viewport,
            host_events,
        );
        return;
    }

    let replacements = host_runtime
        .open
        .keys()
        .filter(|candidate| candidate.owner == key.owner)
        .cloned()
        .collect::<Vec<_>>();
    if !replacements.is_empty() {
        host_runtime
            .pending_replacements
            .insert(key.clone(), replacements);
    }
    host_runtime.open.insert(key.clone(), host.route);
    host_runtime.open_order.retain(|open_key| open_key != &key);
    host_runtime.open_order.push(key.clone());
    preview_commands.write(UiDocumentPreviewCommand::Register(
        host.preview_registration(&host.source, target_profile_from_viewport(viewport)),
    ));

    // Ownership is delayed until commit for pure data routes, keeping the prior Rust page as a
    // visible fallback while validation and resource preflight run.
}

fn close_host(
    host: &DeclarativeScreenHost,
    host_runtime: &mut DeclarativeScreenHostRuntime,
    preview_commands: &mut MessageWriter<UiDocumentPreviewCommand>,
    runtime_commands: &mut MessageWriter<UiDocumentRuntimeCommand>,
    host_events: &mut MessageWriter<DeclarativeScreenHostEvent>,
) {
    let key = host.key();
    if host_runtime.open.remove(&key).is_none() {
        return;
    }
    host_runtime.open_order.retain(|open_key| open_key != &key);
    host_runtime.fallback_attempted.remove(&key);
    host_runtime.pending_owner_switch.remove(&key);
    host_runtime.pending_replacements.remove(&key);
    preview_commands.write(UiDocumentPreviewCommand::Unregister {
        document_id: host.document_id.clone(),
        owner: host.owner.as_str().to_owned(),
    });
    runtime_commands.write(UiDocumentRuntimeCommand::Close {
        owner: host.owner.as_str().to_owned(),
        document_id: host.document_id.clone(),
    });
    host_events.write(DeclarativeScreenHostEvent::Closed {
        route: host.route.to_owned(),
        document_id: host.document_id.clone(),
        owner: host.owner.as_str().to_owned(),
    });
}

fn validate_host_source(
    host: &DeclarativeScreenHost,
    source_json: &str,
    actions: &UiActionRegistry,
) -> Result<(), String> {
    let validation = UiDocument::validate_json(source_json);
    let Some(validated) = validation.validated() else {
        return Err(validation
            .report
            .diagnostics
            .first()
            .map(|diagnostic| diagnostic.code.clone())
            .unwrap_or_else(|| "UI_DOCUMENT_VALIDATION_FAILED".to_owned()));
    };
    if validated.document().document_id != host.document_id {
        return Err("UI_DECLARATIVE_SCREEN_DOCUMENT_ID_MISMATCH".to_owned());
    }
    let mut document_actions = BTreeMap::<UiActionId, BTreeSet<UiNodeId>>::new();
    collect_document_actions(&validated.document().root, &mut document_actions);
    if document_actions.keys().cloned().collect::<BTreeSet<_>>() != host.action_allowlist {
        return Err("UI_DECLARATIVE_SCREEN_ACTION_ALLOWLIST_MISMATCH".to_owned());
    }
    for (action, sources) in &document_actions {
        let Some(descriptor) = actions.descriptor(action) else {
            return Err("UI_DECLARATIVE_SCREEN_ACTION_UNREGISTERED".to_owned());
        };
        if descriptor.document_id != host.document_id
            || descriptor.owner != host.owner.as_str()
            || descriptor.sources != *sources
        {
            return Err("UI_DECLARATIVE_SCREEN_ACTION_CONTRACT_MISMATCH".to_owned());
        }
    }
    let document_bindings = validated
        .document()
        .bindings
        .iter()
        .filter(|(_, declaration)| {
            matches!(
                declaration.scope,
                UiBindingScope::Document | UiBindingScope::Owner
            )
        })
        .map(|(path, declaration)| {
            (
                UiHostBindingKey::new(declaration.scope, path.clone()),
                declaration.value_type.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    if document_bindings != host.binding_schema {
        return Err("UI_DECLARATIVE_SCREEN_BINDING_CONTRACT_MISMATCH".to_owned());
    }
    if let Some(error) = validated
        .validate_with_host(&UiDocumentHostValidationContext {
            owner: host.owner.as_str(),
            owner_alive: true,
            action_registry: actions,
            bindings: &host.binding_schema,
        })
        .into_iter()
        .next()
    {
        return Err(error.code.to_owned());
    }
    Ok(())
}

fn collect_document_actions(
    node: &crate::framework::ui::document::UiNode,
    actions: &mut BTreeMap<UiActionId, BTreeSet<UiNodeId>>,
) {
    for trigger in [
        crate::framework::ui::document::UiActionTrigger::Click,
        crate::framework::ui::document::UiActionTrigger::Change,
        crate::framework::ui::document::UiActionTrigger::Submit,
    ] {
        if let Some(action) = crate::framework::ui::document::node_action(node, trigger) {
            actions
                .entry(action.action.clone())
                .or_default()
                .insert(node.id().clone());
        }
    }
    for child in node.children() {
        collect_document_actions(child, actions);
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_document_reload_events(
    mut reload_events: MessageReader<UiDocumentReloadEvent>,
    registry: Res<DeclarativeScreenRegistry>,
    viewport: Option<Res<crate::framework::ui::core::UiViewport>>,
    mut host_runtime: ResMut<DeclarativeScreenHostRuntime>,
    mut current_owner: ResMut<UiCurrentOwner>,
    mut bindings: ResMut<UiBindingValues>,
    mut preview_commands: MessageWriter<UiDocumentPreviewCommand>,
    mut runtime_commands: MessageWriter<UiDocumentRuntimeCommand>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
    mut host_events: MessageWriter<DeclarativeScreenHostEvent>,
) {
    let Some(viewport) = viewport.as_deref() else {
        return;
    };
    for event in reload_events.read() {
        let report = &event.0;
        let key = DeclarativeScreenHostKey {
            document_id: report.document_id.clone(),
            owner: report.owner.clone(),
        };
        if !host_runtime.open.contains_key(&key) {
            continue;
        }
        let Some(host) = registry.host_for_key(&key).cloned() else {
            continue;
        };
        match report.status {
            UiDocumentReloadStatus::Committed => {
                host_runtime.fallback_attempted.remove(&key);
                if let Some(replaced) = host_runtime.pending_replacements.remove(&key) {
                    for replaced_key in replaced {
                        if let Some(replaced_host) = registry.host_for_key(&replaced_key).cloned() {
                            close_host(
                                &replaced_host,
                                &mut host_runtime,
                                &mut preview_commands,
                                &mut runtime_commands,
                                &mut host_events,
                            );
                        }
                    }
                }
                if let Some(previous_owner) = host_runtime.pending_owner_switch.remove(&key) {
                    if let Some(previous_owner) = previous_owner {
                        bindings.clear_owner(previous_owner.as_str());
                        panel_commands.write(UiPanelCommand::CloseAllForOwner(previous_owner));
                        runtime_commands.write(UiDocumentRuntimeCommand::SwitchOwner {
                            previous_owner: previous_owner.as_str().to_owned(),
                        });
                    }
                    current_owner.owner = Some(host.owner);
                }
                host_events.write(DeclarativeScreenHostEvent::Opened {
                    route: host.route.to_owned(),
                    document_id: host.document_id.clone(),
                    owner: host.owner.as_str().to_owned(),
                });
            }
            UiDocumentReloadStatus::Failed | UiDocumentReloadStatus::Cancelled => {
                let cause = report
                    .error
                    .as_ref()
                    .map(|error| error.code.clone())
                    .unwrap_or_else(|| "UI_DOCUMENT_RELOAD_FAILED".to_owned());
                report_host_failure(
                    &host,
                    cause,
                    report.current_instance.is_some(),
                    &mut host_runtime,
                    &mut preview_commands,
                    viewport,
                    &mut host_events,
                );
            }
            UiDocumentReloadStatus::Queued => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn report_host_failure(
    host: &DeclarativeScreenHost,
    cause: String,
    has_current_instance: bool,
    host_runtime: &mut DeclarativeScreenHostRuntime,
    preview_commands: &mut MessageWriter<UiDocumentPreviewCommand>,
    viewport: &crate::framework::ui::core::UiViewport,
    host_events: &mut MessageWriter<DeclarativeScreenHostEvent>,
) {
    let key = host.key();
    let decision = if has_current_instance {
        DeclarativeScreenFailureDecision::RetainedPrevious
    } else if host.fallback_source.is_some() && host_runtime.fallback_attempted.insert(key.clone())
    {
        match host.failure_policy {
            DeclarativeScreenFailurePolicy::PackagedFallback => {
                DeclarativeScreenFailureDecision::LoadingPackagedFallback
            }
            DeclarativeScreenFailurePolicy::ControlledError => {
                DeclarativeScreenFailureDecision::LoadingControlledError
            }
            DeclarativeScreenFailurePolicy::RetainPrevious => {
                DeclarativeScreenFailureDecision::NoFallbackAvailable
            }
        }
    } else {
        DeclarativeScreenFailureDecision::NoFallbackAvailable
    };
    warn!(
        route = host.route,
        document_id = host.document_id.as_str(),
        owner = host.owner.as_str(),
        cause,
        ?decision,
        "declarative screen load failed"
    );
    host_events.write(DeclarativeScreenHostEvent::LoadFailed {
        code: "UI_DECLARATIVE_SCREEN_LOAD_FAILED".to_owned(),
        cause,
        route: host.route.to_owned(),
        document_id: host.document_id.clone(),
        owner: host.owner.as_str().to_owned(),
        decision,
    });
    if matches!(
        decision,
        DeclarativeScreenFailureDecision::LoadingPackagedFallback
            | DeclarativeScreenFailureDecision::LoadingControlledError
    ) {
        let source = host
            .fallback_source
            .as_ref()
            .expect("fallback decision requires a source");
        preview_commands.write(UiDocumentPreviewCommand::Register(
            host.preview_registration(source, target_profile_from_viewport(viewport)),
        ));
    }
}

fn restore_hosts_after_resume(
    mut lifecycle_events: MessageReader<AppLifecycle>,
    registry: Res<DeclarativeScreenRegistry>,
    runtime: Res<UiDocumentRuntime>,
    host_runtime: Res<DeclarativeScreenHostRuntime>,
    mut preview_commands: MessageWriter<UiDocumentPreviewCommand>,
) {
    let resumed = lifecycle_events
        .read()
        .any(|event| matches!(event, AppLifecycle::WillResume | AppLifecycle::Running));
    if !resumed {
        return;
    }
    for key in host_runtime.open.keys() {
        if runtime
            .active_instance(&key.owner, &key.document_id)
            .is_some()
        {
            continue;
        }
        let Some(host) = registry.host_for_key(key) else {
            continue;
        };
        preview_commands.write(UiDocumentPreviewCommand::Reload {
            reload_id: next_reload_id(key),
            document_id: host.document_id.clone(),
            owner: host.owner.as_str().to_owned(),
        });
    }
}

fn next_reload_id(
    key: &DeclarativeScreenHostKey,
) -> crate::framework::ui::document::UiDocumentReloadId {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    crate::framework::ui::document::UiDocumentReloadId(hasher.finish())
}

pub(in crate::game) fn register_declarative_route_audit_entries(
    registry: &mut UiAuditScreenRegistry,
    hosts: &DeclarativeScreenRegistry,
) {
    for host in hosts.hosts().filter(|host| host.mode.is_none()) {
        registry.register_recipe(UiAuditScreenRecipe::new(
            UiAuditScreen::new(host.route, host.route_aliases, host.owner).with_recipe(
                UiAuditRecipe::new(DOCUMENT_AUDIT_CAPTURES)
                    .with_ready(UiAuditReadyCondition::OwnerDocument),
            ),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::ui::{
        core::{UiMetrics, focus::UiFocusState},
        document::{UiDocumentRuntimePlugin, UiDocumentRuntimeRoot},
        style::{UiFontAssets, UiTheme},
    };
    use bevy::{ecs::message::MessageCursor, state::app::StatesPlugin};

    fn test_document(document_id: &str, label: &str) -> String {
        format!(
            r#"{{"schema_version":1,"document_id":"{document_id}","root":{{"type":"text","id":"page.title","content":{{"literal":"{label}"}}}}}}"#
        )
    }

    fn test_host(
        route: &'static str,
        owner: UiOwnerId,
        document_id: &str,
    ) -> DeclarativeScreenHost {
        DeclarativeScreenHost {
            document_id: UiDocumentId::from_str(document_id).unwrap(),
            route,
            route_aliases: &[],
            mode: None,
            owner,
            panel: crate::framework::ui::document::UiDocumentPanel::Page,
            layer: UiDocumentLayer::Page,
            initial_state: UiPageState::initial(),
            binding_schema: BTreeMap::new(),
            action_allowlist: BTreeSet::new(),
            audit_profiles: DEFAULT_AUDIT_PROFILES.map(str::to_owned).to_vec(),
            source: DeclarativeScreenSource::content_cache(
                "host/test.json",
                test_document(document_id, route),
            ),
            fallback_source: None,
            failure_policy: DeclarativeScreenFailurePolicy::RetainPrevious,
        }
    }

    fn host_app(hosts: impl IntoIterator<Item = DeclarativeScreenHost>) -> App {
        let mut app = App::new();
        app.add_plugins(StatesPlugin)
            .init_state::<AppUiMode>()
            .insert_resource(UiTheme::default())
            .insert_resource(UiMetrics::default())
            .insert_resource(UiFontAssets::test_registry())
            .init_resource::<UiFocusState>()
            .init_resource::<crate::framework::ui::core::UiViewport>()
            .add_plugins((
                UiDocumentRuntimePlugin,
                crate::framework::ui::document::UiDocumentPreviewPlugin,
                DeclarativeScreenHostPlugin,
            ));
        app.insert_resource(hosts.into_iter().collect::<DeclarativeScreenRegistry>());
        app
    }

    fn update_until_idle(app: &mut App) {
        for _ in 0..4 {
            app.update();
        }
    }

    fn active_instance(
        app: &App,
        owner: UiOwnerId,
        document_id: &str,
    ) -> Option<crate::framework::ui::document::UiDocumentInstanceId> {
        app.world().resource::<UiDocumentRuntime>().active_instance(
            owner.as_str(),
            &UiDocumentId::from_str(document_id).unwrap(),
        )
    }

    #[test]
    fn mode_host_registered_after_initial_observation_still_opens() {
        let owner = UiOwnerId::new("host_late_owner");
        let mut app = host_app([]);
        app.update();

        let mut host = test_host("host_late", owner, "host.late");
        host.mode = Some(AppUiMode::Login);
        app.world_mut()
            .resource_mut::<DeclarativeScreenRegistry>()
            .register(host)
            .unwrap();
        update_until_idle(&mut app);

        assert!(active_instance(&app, owner, "host.late").is_some());
    }

    #[test]
    fn repeated_open_is_idempotent_and_reuses_runtime_panel_layer_roots() {
        let owner = UiOwnerId::new("host_repeat_owner");
        let mut app = host_app([test_host("host_repeat", owner, "host.repeat")]);
        for _ in 0..2 {
            app.world_mut()
                .write_message(DeclarativeScreenHostCommand::OpenDetachedRoute {
                    route: "host_repeat".to_owned(),
                });
        }
        update_until_idle(&mut app);

        let instance = active_instance(&app, owner, "host.repeat").unwrap();
        let runtime = app.world().resource::<UiDocumentRuntime>();
        let root = runtime.instance(instance).unwrap().root;
        let entity = app.world().entity(root);
        assert!(entity.contains::<UiDocumentRuntimeRoot>());
        assert!(entity.contains::<crate::framework::ui::core::UiPanelRoot>());
        assert!(entity.contains::<crate::framework::ui::core::UiLayerRoot>());
        assert_eq!(
            app.world()
                .resource::<DeclarativeScreenHostRuntime>()
                .open
                .len(),
            1
        );
    }

    #[test]
    fn same_owner_route_replace_keeps_previous_until_replacement_commits() {
        let owner = UiOwnerId::new("host_replace_owner");
        let mut app = host_app([
            test_host("host_first", owner, "host.first"),
            test_host("host_second", owner, "host.second"),
        ]);
        app.world_mut()
            .write_message(DeclarativeScreenHostCommand::OpenDetachedRoute {
                route: "host_first".to_owned(),
            });
        update_until_idle(&mut app);
        assert!(active_instance(&app, owner, "host.first").is_some());

        app.world_mut()
            .write_message(DeclarativeScreenHostCommand::OpenDetachedRoute {
                route: "host_second".to_owned(),
            });
        update_until_idle(&mut app);
        assert!(active_instance(&app, owner, "host.first").is_none());
        assert!(active_instance(&app, owner, "host.second").is_some());
    }

    #[test]
    fn owner_cleanup_isolated_between_detached_hosts_and_route_close() {
        let owner_a = UiOwnerId::new("host_owner_a");
        let owner_b = UiOwnerId::new("host_owner_b");
        let mut app = host_app([
            test_host("host_a", owner_a, "host.owner_a"),
            test_host("host_b", owner_b, "host.owner_b"),
        ]);
        for route in ["host_a", "host_b"] {
            app.world_mut()
                .write_message(DeclarativeScreenHostCommand::OpenDetachedRoute {
                    route: route.to_owned(),
                });
        }
        update_until_idle(&mut app);
        assert!(active_instance(&app, owner_a, "host.owner_a").is_some());
        assert!(active_instance(&app, owner_b, "host.owner_b").is_some());

        app.world_mut()
            .write_message(DeclarativeScreenHostCommand::CloseRoute {
                route: "host_a".to_owned(),
            });
        update_until_idle(&mut app);
        assert!(active_instance(&app, owner_a, "host.owner_a").is_none());
        assert!(active_instance(&app, owner_b, "host.owner_b").is_some());
    }

    #[test]
    fn committed_pure_route_switches_owner_and_reclaims_the_previous_owner_tree() {
        let owner_a = UiOwnerId::new("host_route_owner_a");
        let owner_b = UiOwnerId::new("host_route_owner_b");
        let mut app = host_app([
            test_host("host_route_a", owner_a, "host.route_a"),
            test_host("host_route_b", owner_b, "host.route_b"),
        ]);
        app.world_mut()
            .write_message(DeclarativeScreenHostCommand::OpenDetachedRoute {
                route: "host_route_a".to_owned(),
            });
        update_until_idle(&mut app);
        app.world_mut().resource_mut::<UiCurrentOwner>().owner = Some(owner_a);

        app.world_mut()
            .write_message(DeclarativeScreenHostCommand::OpenRoute {
                route: "host_route_b".to_owned(),
            });
        update_until_idle(&mut app);

        assert!(active_instance(&app, owner_a, "host.route_a").is_none());
        assert!(active_instance(&app, owner_b, "host.route_b").is_some());
        assert_eq!(
            app.world().resource::<UiCurrentOwner>().owner,
            Some(owner_b)
        );
    }

    #[test]
    fn failed_reload_retains_old_tree_and_initial_failure_uses_fallback() {
        let owner = UiOwnerId::new("host_failure_owner");
        let mut retained = test_host("host_failure", owner, "host.failure");
        retained.fallback_source = Some(DeclarativeScreenSource::content_cache(
            "host/fallback.json",
            test_document("host.failure", "fallback"),
        ));
        retained.failure_policy = DeclarativeScreenFailurePolicy::PackagedFallback;
        let mut app = host_app([retained]);
        app.world_mut()
            .write_message(DeclarativeScreenHostCommand::OpenDetachedRoute {
                route: "host_failure".to_owned(),
            });
        update_until_idle(&mut app);
        let before = active_instance(&app, owner, "host.failure").unwrap();
        app.world_mut()
            .write_message(DeclarativeScreenHostCommand::ReloadRouteSource {
                route: "host_failure".to_owned(),
                source_json: "{invalid".to_owned(),
            });
        app.update();

        let messages = app
            .world()
            .resource::<Messages<DeclarativeScreenHostEvent>>();
        let mut cursor = MessageCursor::default();
        assert!(cursor.read(messages).any(|event| matches!(
            event,
            DeclarativeScreenHostEvent::LoadFailed {
                decision: DeclarativeScreenFailureDecision::RetainedPrevious,
                ..
            }
        )));
        update_until_idle(&mut app);
        assert_eq!(active_instance(&app, owner, "host.failure"), Some(before));
    }

    #[test]
    fn initial_failure_activates_the_registered_fallback_without_a_blank_runtime() {
        let owner = UiOwnerId::new("host_initial_fallback_owner");
        let mut host = test_host("host_initial_fallback", owner, "host.initial_fallback");
        host.source = DeclarativeScreenSource::content_cache("host/invalid.json", "{invalid");
        host.fallback_source = Some(DeclarativeScreenSource::approved(
            "host/fallback.json",
            test_document("host.initial_fallback", "fallback"),
        ));
        host.failure_policy = DeclarativeScreenFailurePolicy::PackagedFallback;
        let mut app = host_app([host]);
        app.world_mut()
            .write_message(DeclarativeScreenHostCommand::OpenDetachedRoute {
                route: "host_initial_fallback".to_owned(),
            });
        app.update();
        let messages = app
            .world()
            .resource::<Messages<DeclarativeScreenHostEvent>>();
        let mut cursor = MessageCursor::default();
        assert!(cursor.read(messages).any(|event| matches!(
            event,
            DeclarativeScreenHostEvent::LoadFailed {
                decision: DeclarativeScreenFailureDecision::LoadingPackagedFallback,
                ..
            }
        )));
        update_until_idle(&mut app);
        assert!(active_instance(&app, owner, "host.initial_fallback").is_some());
    }

    #[test]
    fn fixed_mode_host_opens_on_enter_and_closes_on_exit() {
        let owner = UiOwnerId::new("host_mode_owner");
        let mut mode_host = test_host("host_mode", owner, "host.mode");
        mode_host.mode = Some(AppUiMode::Lobby);
        let mut app = host_app([mode_host]);
        app.update();
        app.world_mut()
            .resource_mut::<NextState<AppUiMode>>()
            .set(AppUiMode::Lobby);
        update_until_idle(&mut app);
        assert!(active_instance(&app, owner, "host.mode").is_some());

        app.world_mut()
            .resource_mut::<NextState<AppUiMode>>()
            .set(AppUiMode::Login);
        update_until_idle(&mut app);
        assert!(active_instance(&app, owner, "host.mode").is_none());
    }

    #[test]
    fn pilot_page_is_a_data_registered_route_with_reload_exit_and_audit_contract() {
        let registry = DeclarativeScreenRegistry::default();
        let pilot = registry
            .route("ui-declarative-pilot")
            .expect("pilot route should be data-registered");
        assert!(pilot.mode.is_none());
        assert!(pilot.action_allowlist.is_empty());
        assert!(pilot.binding_schema.is_empty());
        assert!(
            UiDocument::validate_json(&pilot.source.source_json)
                .report
                .valid
        );

        let mut audit_registry = UiAuditScreenRegistry::default();
        register_declarative_route_audit_entries(&mut audit_registry, &registry);
        assert_eq!(
            audit_registry
                .resolve("declarative_pilot")
                .expect("pilot should be auditable")
                .owner,
            OWNER_DECLARATIVE_DOCUMENT_ROUTE
        );

        let mut app = host_app(registry.hosts.clone());
        app.world_mut()
            .write_message(DeclarativeScreenHostCommand::OpenRoute {
                route: "ui_declarative_pilot".to_owned(),
            });
        update_until_idle(&mut app);
        assert!(
            active_instance(
                &app,
                OWNER_DECLARATIVE_DOCUMENT_ROUTE,
                "game.declarative_pilot"
            )
            .is_some()
        );

        app.world_mut()
            .write_message(DeclarativeScreenHostCommand::ReloadRoute {
                route: "ui_declarative_pilot".to_owned(),
            });
        update_until_idle(&mut app);
        app.world_mut()
            .write_message(DeclarativeScreenHostCommand::CloseRoute {
                route: "ui_declarative_pilot".to_owned(),
            });
        update_until_idle(&mut app);
        assert!(
            active_instance(
                &app,
                OWNER_DECLARATIVE_DOCUMENT_ROUTE,
                "game.declarative_pilot"
            )
            .is_none()
        );
    }

    #[test]
    fn approved_business_fixture_matches_the_game_owned_host_contract() {
        const REGISTRATION_SOURCE: &str = include_str!(
            "../../assets/ui/documents/approved/business_acceptance_fixture/promotion.v1.json"
        );
        let registry = DeclarativeScreenRegistry::default();
        let host = registry
            .route("ui_approved_business_acceptance")
            .expect("business acceptance route should be data-registered");
        let mut actions = UiActionRegistry::default();
        let action = UiActionId::from_str("approved.acceptance_continue").unwrap();
        actions
            .register(
                crate::framework::ui::document::UiActionDescriptor::new(
                    action.clone(),
                    host.document_id.clone(),
                    host.owner.as_str(),
                    crate::framework::ui::document::UiRegisteredActionKind::BusinessCommand {
                        target: "game.approved_business_acceptance".to_owned(),
                    },
                )
                .with_source(UiNodeId::from_str("acceptance.continue").unwrap()),
            )
            .unwrap();
        validate_host_source(host, &host.source.source_json, &actions).unwrap();

        let contract = crate::framework::ui::document::UiApprovedDocumentHostContract::new(
            crate::framework::ui::document::UI_APPROVED_DOCUMENT_HOST_CONTRACT_VERSION,
            host.document_id.clone(),
            host.owner.as_str(),
            host.route,
            host.panel,
            host.layer,
            host.initial_state.clone(),
            host.audit_profiles.clone(),
            host.binding_schema.clone(),
            BTreeMap::from([(
                action,
                [UiNodeId::from_str("acceptance.continue").unwrap()]
                    .into_iter()
                    .collect(),
            )]),
            BTreeSet::new(),
        )
        .unwrap();
        let registration = crate::framework::ui::document::parse_approved_document_registration(
            REGISTRATION_SOURCE,
        )
        .unwrap();
        let preview = registration
            .to_preview_registration_with_contract(
                host.source.source_json.clone(),
                target_profile_from_viewport(&crate::framework::ui::core::UiViewport::default()),
                Some(&contract),
            )
            .unwrap();
        assert_eq!(preview.host_bindings, host.binding_schema);
        assert_eq!(
            registration
                .audit_report(&host.source.source_json)
                .unwrap()
                .actions,
            ["approved.acceptance_continue"]
        );
    }

    #[test]
    fn resume_reloads_an_open_host_after_its_runtime_tree_was_reclaimed() {
        let owner = UiOwnerId::new("host_resume_owner");
        let mut app = host_app([test_host("host_resume", owner, "host.resume")]);
        app.world_mut()
            .write_message(DeclarativeScreenHostCommand::OpenDetachedRoute {
                route: "host_resume".to_owned(),
            });
        update_until_idle(&mut app);
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::CloseAllForOwner {
                owner: owner.as_str().to_owned(),
            });
        update_until_idle(&mut app);
        assert!(active_instance(&app, owner, "host.resume").is_none());

        app.world_mut().write_message(AppLifecycle::WillResume);
        update_until_idle(&mut app);
        assert!(active_instance(&app, owner, "host.resume").is_some());
    }
}
