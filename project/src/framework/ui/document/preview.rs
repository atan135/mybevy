use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use bevy::prelude::*;
use serde::Serialize;
use serde_json::Value;

use super::{
    UiDocument, UiDocumentBuildState, UiDocumentFailureStage, UiDocumentId, UiDocumentInstanceId,
    UiDocumentLayer, UiDocumentOpenRequest, UiDocumentOpenSource, UiDocumentPanel,
    UiDocumentRequestId, UiDocumentRuntime, UiDocumentRuntimeCommand, UiDocumentRuntimeSystems,
    UiDocumentSourceOrigin, UiHostBindingKey, UiNode, UiNodeId, UiPageState, UiTargetProfile,
    UiValidationDiagnostic, ValidatedUiDocument,
};
use crate::framework::ui::{
    core::{UiInputMode, UiViewport, focus::UiFocusState},
    widgets::{
        FocusableButton, SelectedButton, UiControlFlags, UiDropdown, UiSlider, UiStepper,
        UiTextInputMaxChars, UiTextInputValue,
        controls::{
            UiCheckboxChecked, UiSegmentOption, UiSegmentOptionSelected, UiTab, UiTextInputCursor,
            UiTextInputNativeState, UiToggleOn, apply_native_text_input_state,
            ui_text_input_native_state_from_value,
        },
    },
};

const PREVIEW_REQUEST_ID_BASE: u64 = 1 << 63;
const UI_DOCUMENT_RELOAD_REPORT_VERSION: u32 = 1;
const UI_DOCUMENT_WATCH_ENV: &str = "MYBEVY_UI_DOCUMENT_WATCH";

/// Filesystem roots are closed and resolve relative to the Rust project root.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiDocumentSourceRoot {
    Approved,
    Fixture,
    Authoring,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct UiDocumentSourcePath {
    logical: String,
}

impl UiDocumentSourcePath {
    pub fn new(
        root: UiDocumentSourceRoot,
        relative: impl AsRef<str>,
    ) -> Result<Self, UiDocumentSourcePathError> {
        let relative = relative.as_ref();
        if !valid_relative_json_path(relative) {
            return Err(UiDocumentSourcePathError);
        }
        let prefix = match root {
            UiDocumentSourceRoot::Approved => "ui/documents/approved",
            UiDocumentSourceRoot::Fixture => "ui/documents/fixtures",
            UiDocumentSourceRoot::Authoring => "ui-documents/source",
        };
        Ok(Self {
            logical: format!("{prefix}/{relative}"),
        })
    }

    pub fn as_str(&self) -> &str {
        &self.logical
    }

    fn filesystem_location(&self) -> Option<(PathBuf, PathBuf)> {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for (prefix, root) in [
            (
                "ui/documents/approved/",
                project_root.join("assets/ui/documents/approved"),
            ),
            (
                "ui/documents/fixtures/",
                project_root.join("assets/ui/documents/fixtures"),
            ),
            (
                "ui-documents/source/",
                project_root.join("ui-documents/source"),
            ),
        ] {
            if let Some(relative) = self.logical.strip_prefix(prefix) {
                return Some((root.clone(), root.join(relative)));
            }
        }
        None
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiDocumentSourcePathError;

impl std::fmt::Display for UiDocumentSourcePathError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("document source must be a safe lowercase relative JSON path")
    }
}

impl std::error::Error for UiDocumentSourcePathError {}

fn valid_relative_json_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 240
        && value.is_ascii()
        && value.ends_with(".json")
        && !value.starts_with('/')
        && !value.contains(['\\', ':', '\0', '\n', '\r'])
        && value.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && segment.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'_' | b'-' | b'.')
                })
        })
}

#[derive(Clone, Debug)]
pub struct UiDocumentPreviewRegistration {
    pub document_id: UiDocumentId,
    pub owner: String,
    pub source_path: UiDocumentSourcePath,
    pub source_json: String,
    pub panel: UiDocumentPanel,
    pub layer: UiDocumentLayer,
    pub target_profile: UiTargetProfile,
    pub page_state: UiPageState,
    pub owner_alive: bool,
    pub host_bindings: BTreeMap<UiHostBindingKey, super::UiBindingType>,
    pub watch: bool,
    pub open_on_register: bool,
    pub audit_profiles: Vec<String>,
}

#[derive(Clone, Debug, Message)]
pub enum UiDocumentPreviewCommand {
    Register(UiDocumentPreviewRegistration),
    Unregister {
        document_id: UiDocumentId,
        owner: String,
    },
    Reload {
        reload_id: UiDocumentReloadId,
        document_id: UiDocumentId,
        owner: String,
    },
    ReloadSource {
        reload_id: UiDocumentReloadId,
        document_id: UiDocumentId,
        owner: String,
        source_json: String,
    },
    /// Rebuilds the registered document through the normal effective-document merge for an
    /// explicitly declared visible page state. This is a preview/audit command, not a business
    /// action binding.
    SetPageState {
        reload_id: UiDocumentReloadId,
        document_id: UiDocumentId,
        owner: String,
        page_state: UiPageState,
    },
    SetWatchEnabled(bool),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct UiDocumentReloadId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiDocumentReloadStatus {
    Queued,
    Committed,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiDocumentReloadStage {
    Source,
    Validation,
    HostValidation,
    ResourcePreflight,
    Commit,
    Cancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiDocumentDiffKind {
    NoChanges,
    InPlace,
    RebuildSubtrees,
    RebuildPage,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UiDocumentDiff {
    pub kind: UiDocumentDiffKind,
    pub in_place_nodes: Vec<UiNodeId>,
    pub rebuild_subtrees: Vec<UiNodeId>,
    pub page_reasons: Vec<String>,
}

impl UiDocumentDiff {
    fn initial_page() -> Self {
        Self {
            kind: UiDocumentDiffKind::RebuildPage,
            in_place_nodes: Vec::new(),
            rebuild_subtrees: Vec::new(),
            page_reasons: vec!["initial_open".to_owned()],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UiDocumentStateDecision {
    pub node_id: UiNodeId,
    pub state: String,
    pub preserved: bool,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UiDocumentReloadError {
    pub code: String,
    pub stage: UiDocumentReloadStage,
    pub document_path: Option<String>,
    pub node_id: Option<UiNodeId>,
    pub field_path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UiDocumentReloadReport {
    pub report_version: u32,
    pub reload_id: UiDocumentReloadId,
    pub request_id: Option<UiDocumentRequestId>,
    pub document_id: UiDocumentId,
    pub owner: String,
    pub source_path: String,
    pub status: UiDocumentReloadStatus,
    pub previous_instance: Option<UiDocumentInstanceId>,
    pub current_instance: Option<UiDocumentInstanceId>,
    pub diff: Option<UiDocumentDiff>,
    pub state_decisions: Vec<UiDocumentStateDecision>,
    pub error: Option<UiDocumentReloadError>,
}

#[derive(Clone, Debug, Message)]
pub struct UiDocumentReloadEvent(pub UiDocumentReloadReport);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UiDocumentAuditRecipeEntry {
    pub screen: String,
    pub document_id: UiDocumentId,
    pub owner: String,
    pub source_path: String,
    pub profiles: Vec<String>,
}

#[derive(Clone, Debug, Default, Resource)]
pub struct UiDocumentAuditRecipeRegistry {
    entries: BTreeMap<(UiDocumentId, String), UiDocumentAuditRecipeEntry>,
}

impl UiDocumentAuditRecipeRegistry {
    pub fn entry(
        &self,
        document_id: &UiDocumentId,
        owner: &str,
    ) -> Option<&UiDocumentAuditRecipeEntry> {
        self.entries.get(&(document_id.clone(), owner.to_owned()))
    }

    pub fn entries(&self) -> impl Iterator<Item = &UiDocumentAuditRecipeEntry> {
        self.entries.values()
    }
}

#[derive(Clone, Debug, Resource)]
pub struct UiDocumentPreviewConfig {
    watch_enabled: bool,
    watch_supported: bool,
}

impl Default for UiDocumentPreviewConfig {
    fn default() -> Self {
        let watch_supported = cfg!(all(debug_assertions, not(target_os = "android")));
        let requested = env::var(UI_DOCUMENT_WATCH_ENV)
            .ok()
            .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes"));
        Self {
            watch_enabled: watch_supported && requested,
            watch_supported,
        }
    }
}

impl UiDocumentPreviewConfig {
    pub fn watch_enabled(&self) -> bool {
        self.watch_enabled
    }

    pub fn watch_supported(&self) -> bool {
        self.watch_supported
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PreviewKey {
    document_id: UiDocumentId,
    owner: String,
}

#[derive(Clone, Debug)]
struct PreviewRegistrationState {
    registration: UiDocumentPreviewRegistration,
    watch_state: SourceWatchState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceSignature {
    modified: Option<SystemTime>,
    len: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SourceWatchState {
    Uninitialized,
    Ready(SourceSignature),
    Failed(&'static str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceWatchAction {
    Ignore,
    Reload,
    Report(&'static str),
}

const UI_DOCUMENT_SOURCE_READ_FAILED: &str = "UI_DOCUMENT_SOURCE_READ_FAILED";
const UI_DOCUMENT_SOURCE_OUTSIDE_ROOT: &str = "UI_DOCUMENT_SOURCE_OUTSIDE_ROOT";

fn canonical_path_is_within_root(root: &Path, candidate: &Path) -> bool {
    candidate != root && candidate.starts_with(root)
}

fn resolve_contained_source_path(
    anchor: &Path,
    root: &Path,
    candidate: &Path,
) -> Result<PathBuf, &'static str> {
    let anchor = fs::canonicalize(anchor).map_err(|_| UI_DOCUMENT_SOURCE_READ_FAILED)?;
    let root = fs::canonicalize(root).map_err(|_| UI_DOCUMENT_SOURCE_READ_FAILED)?;
    let candidate = fs::canonicalize(candidate).map_err(|_| UI_DOCUMENT_SOURCE_READ_FAILED)?;
    if !canonical_path_is_within_root(&anchor, &root)
        || !canonical_path_is_within_root(&root, &candidate)
    {
        return Err(UI_DOCUMENT_SOURCE_OUTSIDE_ROOT);
    }
    Ok(candidate)
}

fn inspect_document_source(
    source_path: &UiDocumentSourcePath,
) -> Result<(PathBuf, SourceSignature), &'static str> {
    let (root, candidate) = source_path
        .filesystem_location()
        .ok_or(UI_DOCUMENT_SOURCE_READ_FAILED)?;
    let resolved =
        resolve_contained_source_path(Path::new(env!("CARGO_MANIFEST_DIR")), &root, &candidate)?;
    let metadata = fs::metadata(&resolved).map_err(|_| UI_DOCUMENT_SOURCE_READ_FAILED)?;
    if !metadata.is_file() {
        return Err(UI_DOCUMENT_SOURCE_READ_FAILED);
    }
    Ok((
        resolved,
        SourceSignature {
            modified: metadata.modified().ok(),
            len: metadata.len(),
        },
    ))
}

fn source_watch_transition(
    previous: &SourceWatchState,
    observation: Result<SourceSignature, &'static str>,
) -> (SourceWatchState, SourceWatchAction) {
    match (previous, observation) {
        (SourceWatchState::Uninitialized, Ok(signature)) => (
            SourceWatchState::Ready(signature),
            SourceWatchAction::Ignore,
        ),
        (SourceWatchState::Uninitialized, Err(code)) => (
            SourceWatchState::Failed(code),
            SourceWatchAction::Report(code),
        ),
        (SourceWatchState::Ready(previous), Ok(current)) if *previous == current => {
            (SourceWatchState::Ready(current), SourceWatchAction::Ignore)
        }
        (SourceWatchState::Ready(_), Ok(current)) | (SourceWatchState::Failed(_), Ok(current)) => {
            (SourceWatchState::Ready(current), SourceWatchAction::Reload)
        }
        (SourceWatchState::Ready(_), Err(code)) => (
            SourceWatchState::Failed(code),
            SourceWatchAction::Report(code),
        ),
        (SourceWatchState::Failed(previous), Err(code)) if *previous == code => {
            (SourceWatchState::Failed(code), SourceWatchAction::Ignore)
        }
        (SourceWatchState::Failed(_), Err(code)) => (
            SourceWatchState::Failed(code),
            SourceWatchAction::Report(code),
        ),
    }
}

#[derive(Clone, Debug, Default, Resource)]
struct UiDocumentPreviewRegistry {
    registrations: BTreeMap<PreviewKey, PreviewRegistrationState>,
    pending: BTreeMap<UiDocumentRequestId, PendingReload>,
    next_request_id: u64,
    next_reload_id: u64,
}

impl UiDocumentPreviewRegistry {
    fn request_id(&mut self) -> UiDocumentRequestId {
        if self.next_request_id < PREVIEW_REQUEST_ID_BASE {
            self.next_request_id = PREVIEW_REQUEST_ID_BASE;
        }
        let id = UiDocumentRequestId(self.next_request_id);
        self.next_request_id = self.next_request_id.saturating_add(1);
        id
    }

    fn reload_id(&mut self) -> UiDocumentReloadId {
        self.next_reload_id = self.next_reload_id.saturating_add(1);
        UiDocumentReloadId(self.next_reload_id)
    }
}

#[derive(Clone, Debug)]
struct PendingReload {
    reload_id: UiDocumentReloadId,
    request_id: UiDocumentRequestId,
    key: PreviewKey,
    source_path: String,
    previous_instance: Option<UiDocumentInstanceId>,
    diff: UiDocumentDiff,
    snapshot: UiDocumentStateSnapshot,
}

#[derive(Clone, Debug, Default)]
struct UiDocumentStateSnapshot {
    focused_node: Option<UiNodeId>,
    nodes: BTreeMap<UiNodeId, Vec<UiDocumentNodeState>>,
}

#[derive(Clone, Debug)]
enum UiDocumentNodeState {
    TextInput {
        value: String,
        selection_start: usize,
        selection_end: usize,
    },
    Scroll {
        position: Vec2,
    },
    Slider {
        value: f32,
    },
    Stepper {
        value: i32,
    },
    Select {
        selected_value: Option<String>,
    },
    Checkbox {
        checked: bool,
    },
    Toggle {
        on: bool,
    },
    Segmented {
        selected_value: Option<String>,
    },
    Tab {
        value: String,
        selected: bool,
    },
    Unsupported {
        state: &'static str,
        reason: &'static str,
    },
}

pub struct UiDocumentPreviewPlugin;

impl Plugin for UiDocumentPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiDocumentPreviewConfig>()
            .init_resource::<UiDocumentPreviewRegistry>()
            .init_resource::<UiDocumentAuditRecipeRegistry>()
            .add_message::<UiDocumentPreviewCommand>()
            .add_message::<UiDocumentReloadEvent>()
            .add_systems(
                Update,
                (poll_document_watch, handle_preview_commands)
                    .chain()
                    .before(UiDocumentRuntimeSystems::Commands),
            )
            .add_systems(
                Update,
                finish_preview_reloads
                    .after(UiDocumentRuntimeSystems::Commit)
                    .before(UiDocumentRuntimeSystems::Reconcile),
            );
    }
}

fn poll_document_watch(
    config: Res<UiDocumentPreviewConfig>,
    runtime: Res<UiDocumentRuntime>,
    mut registry: ResMut<UiDocumentPreviewRegistry>,
    mut commands: MessageWriter<UiDocumentPreviewCommand>,
    mut reload_events: MessageWriter<UiDocumentReloadEvent>,
) {
    if !config.watch_enabled {
        return;
    }
    let keys = registry.registrations.keys().cloned().collect::<Vec<_>>();
    for key in keys {
        let outcome = {
            let Some(entry) = registry.registrations.get_mut(&key) else {
                continue;
            };
            if !entry.registration.watch {
                continue;
            }
            let logical_source_path = entry.registration.source_path.as_str().to_owned();
            let previous = entry.watch_state.clone();
            let inspection = inspect_document_source(&entry.registration.source_path);
            let observation = inspection
                .as_ref()
                .map(|(_, signature)| *signature)
                .map_err(|code| *code);
            let (next_state, action) = source_watch_transition(&previous, observation);
            match action {
                SourceWatchAction::Ignore => {
                    entry.watch_state = next_state;
                    None
                }
                SourceWatchAction::Report(code) => {
                    entry.watch_state = next_state;
                    Some(Err((code, logical_source_path)))
                }
                SourceWatchAction::Reload => {
                    let resolved = inspection
                        .as_ref()
                        .expect("reload transition requires a contained source")
                        .0
                        .clone();
                    match fs::read_to_string(resolved) {
                        Ok(source_json) => {
                            entry.watch_state = next_state;
                            Some(Ok(source_json))
                        }
                        Err(_) => {
                            entry.watch_state =
                                SourceWatchState::Failed(UI_DOCUMENT_SOURCE_READ_FAILED);
                            if previous == SourceWatchState::Failed(UI_DOCUMENT_SOURCE_READ_FAILED)
                            {
                                None
                            } else {
                                Some(Err((UI_DOCUMENT_SOURCE_READ_FAILED, logical_source_path)))
                            }
                        }
                    }
                }
            }
        };
        match outcome {
            Some(Ok(source_json)) => {
                let reload_id = registry.reload_id();
                commands.write(UiDocumentPreviewCommand::ReloadSource {
                    reload_id,
                    document_id: key.document_id.clone(),
                    owner: key.owner.clone(),
                    source_json,
                });
            }
            Some(Err((code, logical_source_path))) => emit_watch_source_failure(
                registry.reload_id(),
                &key,
                logical_source_path,
                code,
                &runtime,
                &mut reload_events,
            ),
            None => {}
        }
    }
}

fn emit_watch_source_failure(
    reload_id: UiDocumentReloadId,
    key: &PreviewKey,
    source_path: String,
    code: &'static str,
    runtime: &UiDocumentRuntime,
    events: &mut MessageWriter<UiDocumentReloadEvent>,
) {
    let current = runtime.active_instance(&key.owner, &key.document_id);
    events.write(UiDocumentReloadEvent(UiDocumentReloadReport {
        report_version: UI_DOCUMENT_RELOAD_REPORT_VERSION,
        reload_id,
        request_id: None,
        document_id: key.document_id.clone(),
        owner: key.owner.clone(),
        source_path,
        status: UiDocumentReloadStatus::Failed,
        previous_instance: current,
        current_instance: current,
        diff: None,
        state_decisions: Vec::new(),
        error: Some(UiDocumentReloadError {
            code: code.to_owned(),
            stage: UiDocumentReloadStage::Source,
            document_path: None,
            node_id: None,
            field_path: None,
        }),
    }));
}

#[allow(clippy::too_many_arguments)]
fn handle_preview_commands(
    mut incoming: MessageReader<UiDocumentPreviewCommand>,
    mut config: ResMut<UiDocumentPreviewConfig>,
    runtime: Res<UiDocumentRuntime>,
    focus: Option<Res<UiFocusState>>,
    parents: Query<&ChildOf>,
    states: Query<(
        Entity,
        &super::UiDocumentNodeMarker,
        Option<&UiTextInputValue>,
        Option<&UiTextInputCursor>,
        Option<&ScrollPosition>,
        Option<&UiSlider>,
        Option<&UiStepper>,
        Option<&UiDropdown>,
        Option<&UiTab>,
        Option<&UiControlFlags>,
        Has<UiCheckboxChecked>,
        Has<UiToggleOn>,
        Has<SelectedButton>,
    )>,
    segment_options: Query<(
        &UiSegmentOption,
        Has<UiSegmentOptionSelected>,
        Option<&UiControlFlags>,
        &ChildOf,
    )>,
    mut registry: ResMut<UiDocumentPreviewRegistry>,
    mut audit_recipes: ResMut<UiDocumentAuditRecipeRegistry>,
    mut runtime_commands: MessageWriter<UiDocumentRuntimeCommand>,
    mut reload_events: MessageWriter<UiDocumentReloadEvent>,
) {
    for command in incoming.read().cloned() {
        match command {
            UiDocumentPreviewCommand::Register(registration) => {
                let key = PreviewKey {
                    document_id: registration.document_id.clone(),
                    owner: registration.owner.clone(),
                };
                audit_recipes.entries.insert(
                    (registration.document_id.clone(), registration.owner.clone()),
                    UiDocumentAuditRecipeEntry {
                        screen: format!(
                            "document_{}",
                            registration.document_id.as_str().replace('.', "_")
                        ),
                        document_id: registration.document_id.clone(),
                        owner: registration.owner.clone(),
                        source_path: registration.source_path.as_str().to_owned(),
                        profiles: normalized_audit_profiles(&registration.audit_profiles),
                    },
                );
                let open = registration.open_on_register;
                let source_json = registration.source_json.clone();
                registry.registrations.insert(
                    key.clone(),
                    PreviewRegistrationState {
                        registration,
                        watch_state: SourceWatchState::Uninitialized,
                    },
                );
                if open {
                    let reload_id = registry.reload_id();
                    start_reload(
                        reload_id,
                        key,
                        source_json,
                        &runtime,
                        focus.as_deref(),
                        &parents,
                        &states,
                        &segment_options,
                        &mut registry,
                        &mut runtime_commands,
                        &mut reload_events,
                    );
                }
            }
            UiDocumentPreviewCommand::Unregister { document_id, owner } => {
                let key = PreviewKey { document_id, owner };
                registry.registrations.remove(&key);
                audit_recipes.entries.remove(&(key.document_id, key.owner));
            }
            UiDocumentPreviewCommand::Reload {
                reload_id,
                document_id,
                owner,
            } => {
                let key = PreviewKey { document_id, owner };
                let source_json = registry
                    .registrations
                    .get(&key)
                    .map(|entry| entry.registration.source_json.clone());
                if let Some(source_json) = source_json {
                    start_reload(
                        reload_id,
                        key,
                        source_json,
                        &runtime,
                        focus.as_deref(),
                        &parents,
                        &states,
                        &segment_options,
                        &mut registry,
                        &mut runtime_commands,
                        &mut reload_events,
                    );
                } else {
                    emit_unregistered_report(reload_id, key, &mut reload_events);
                }
            }
            UiDocumentPreviewCommand::ReloadSource {
                reload_id,
                document_id,
                owner,
                source_json,
            } => {
                let key = PreviewKey { document_id, owner };
                if let Some(entry) = registry.registrations.get_mut(&key) {
                    entry.registration.source_json = source_json.clone();
                    start_reload(
                        reload_id,
                        key,
                        source_json,
                        &runtime,
                        focus.as_deref(),
                        &parents,
                        &states,
                        &segment_options,
                        &mut registry,
                        &mut runtime_commands,
                        &mut reload_events,
                    );
                } else {
                    emit_unregistered_report(reload_id, key, &mut reload_events);
                }
            }
            UiDocumentPreviewCommand::SetPageState {
                reload_id,
                document_id,
                owner,
                page_state,
            } => {
                let key = PreviewKey { document_id, owner };
                let source_json = registry.registrations.get_mut(&key).map(|entry| {
                    entry.registration.page_state = page_state;
                    entry.registration.source_json.clone()
                });
                if let Some(source_json) = source_json {
                    start_reload(
                        reload_id,
                        key,
                        source_json,
                        &runtime,
                        focus.as_deref(),
                        &parents,
                        &states,
                        &segment_options,
                        &mut registry,
                        &mut runtime_commands,
                        &mut reload_events,
                    );
                } else {
                    emit_unregistered_report(reload_id, key, &mut reload_events);
                }
            }
            UiDocumentPreviewCommand::SetWatchEnabled(enabled) => {
                config.watch_enabled = enabled && config.watch_supported;
            }
        }
    }
}

fn normalized_audit_profiles(profiles: &[String]) -> Vec<String> {
    let mut result = profiles
        .iter()
        .filter(|profile| {
            matches!(
                profile.as_str(),
                "phone-small"
                    | "phone-portrait"
                    | "phone-1080p"
                    | "tablet-portrait"
                    | "tablet-landscape"
            )
        })
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if result.is_empty() {
        result = vec![
            "phone-small".to_owned(),
            "phone-portrait".to_owned(),
            "tablet-portrait".to_owned(),
            "tablet-landscape".to_owned(),
        ];
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn start_reload(
    reload_id: UiDocumentReloadId,
    key: PreviewKey,
    source_json: String,
    runtime: &UiDocumentRuntime,
    focus: Option<&UiFocusState>,
    parents: &Query<&ChildOf>,
    states: &Query<(
        Entity,
        &super::UiDocumentNodeMarker,
        Option<&UiTextInputValue>,
        Option<&UiTextInputCursor>,
        Option<&ScrollPosition>,
        Option<&UiSlider>,
        Option<&UiStepper>,
        Option<&UiDropdown>,
        Option<&UiTab>,
        Option<&UiControlFlags>,
        Has<UiCheckboxChecked>,
        Has<UiToggleOn>,
        Has<SelectedButton>,
    )>,
    segment_options: &Query<(
        &UiSegmentOption,
        Has<UiSegmentOptionSelected>,
        Option<&UiControlFlags>,
        &ChildOf,
    )>,
    registry: &mut UiDocumentPreviewRegistry,
    runtime_commands: &mut MessageWriter<UiDocumentRuntimeCommand>,
    reload_events: &mut MessageWriter<UiDocumentReloadEvent>,
) {
    let Some(entry) = registry.registrations.get(&key) else {
        emit_unregistered_report(reload_id, key, reload_events);
        return;
    };
    let registration = entry.registration.clone();
    let validation = UiDocument::validate_json(&source_json);
    let Some(validated) = validation.validated().cloned() else {
        let first = validation.report.diagnostics.first();
        reload_events.write(UiDocumentReloadEvent(UiDocumentReloadReport {
            report_version: UI_DOCUMENT_RELOAD_REPORT_VERSION,
            reload_id,
            request_id: None,
            document_id: key.document_id,
            owner: key.owner,
            source_path: registration.source_path.as_str().to_owned(),
            status: UiDocumentReloadStatus::Failed,
            previous_instance: runtime
                .active_instance(&registration.owner, &registration.document_id),
            current_instance: runtime
                .active_instance(&registration.owner, &registration.document_id),
            diff: None,
            state_decisions: Vec::new(),
            error: Some(validation_error(first)),
        }));
        return;
    };
    if validated.document().document_id != registration.document_id {
        reload_events.write(UiDocumentReloadEvent(UiDocumentReloadReport {
            report_version: UI_DOCUMENT_RELOAD_REPORT_VERSION,
            reload_id,
            request_id: None,
            document_id: key.document_id,
            owner: key.owner,
            source_path: registration.source_path.as_str().to_owned(),
            status: UiDocumentReloadStatus::Failed,
            previous_instance: runtime
                .active_instance(&registration.owner, &registration.document_id),
            current_instance: runtime
                .active_instance(&registration.owner, &registration.document_id),
            diff: None,
            state_decisions: Vec::new(),
            error: Some(UiDocumentReloadError {
                code: "UI_DOCUMENT_ID_MISMATCH".to_owned(),
                stage: UiDocumentReloadStage::Validation,
                document_path: Some("$".to_owned()),
                node_id: None,
                field_path: Some("$.document_id".to_owned()),
            }),
        }));
        return;
    }
    let effective_document = match validated
        .effective_document(&registration.target_profile, &registration.page_state)
    {
        Ok(effective) => effective.document,
        Err(error) => {
            reload_events.write(UiDocumentReloadEvent(UiDocumentReloadReport {
                report_version: UI_DOCUMENT_RELOAD_REPORT_VERSION,
                reload_id,
                request_id: None,
                document_id: key.document_id,
                owner: key.owner,
                source_path: registration.source_path.as_str().to_owned(),
                status: UiDocumentReloadStatus::Failed,
                previous_instance: runtime
                    .active_instance(&registration.owner, &registration.document_id),
                current_instance: runtime
                    .active_instance(&registration.owner, &registration.document_id),
                diff: None,
                state_decisions: Vec::new(),
                error: Some(UiDocumentReloadError {
                    code: error.code().to_owned(),
                    stage: UiDocumentReloadStage::Validation,
                    document_path: Some("$".to_owned()),
                    node_id: None,
                    field_path: None,
                }),
            }));
            return;
        }
    };
    let effective = match ValidatedUiDocument::new(effective_document) {
        Ok(effective) => effective,
        Err(error) => {
            reload_events.write(UiDocumentReloadEvent(UiDocumentReloadReport {
                report_version: UI_DOCUMENT_RELOAD_REPORT_VERSION,
                reload_id,
                request_id: None,
                document_id: key.document_id,
                owner: key.owner,
                source_path: registration.source_path.as_str().to_owned(),
                status: UiDocumentReloadStatus::Failed,
                previous_instance: runtime
                    .active_instance(&registration.owner, &registration.document_id),
                current_instance: runtime
                    .active_instance(&registration.owner, &registration.document_id),
                diff: None,
                state_decisions: Vec::new(),
                error: Some(UiDocumentReloadError {
                    code: error.code().to_owned(),
                    stage: UiDocumentReloadStage::Validation,
                    document_path: Some("$".to_owned()),
                    node_id: None,
                    field_path: None,
                }),
            }));
            return;
        }
    };
    let previous_instance = runtime.active_instance(&registration.owner, &registration.document_id);
    let diff = runtime
        .active_validated_document(&registration.owner, &registration.document_id)
        .map_or_else(UiDocumentDiff::initial_page, |old| {
            diff_ui_documents(old.document(), effective.document())
        });
    let snapshot = previous_instance.map_or_else(UiDocumentStateSnapshot::default, |instance_id| {
        snapshot_document_state(
            instance_id,
            runtime.active_validated_document_by_instance(instance_id),
            focus,
            parents,
            states,
            segment_options,
        )
    });
    let request_id = registry.request_id();
    registry.pending.insert(
        request_id,
        PendingReload {
            reload_id,
            request_id,
            key: key.clone(),
            source_path: registration.source_path.as_str().to_owned(),
            previous_instance,
            diff: diff.clone(),
            snapshot,
        },
    );
    runtime_commands.write(UiDocumentRuntimeCommand::Open(UiDocumentOpenRequest {
        request_id,
        document_id: registration.document_id,
        owner: registration.owner,
        source: UiDocumentOpenSource::Json(source_json),
        origin: UiDocumentSourceOrigin::Preview {
            source_path: registration.source_path.as_str().to_owned(),
        },
        panel: registration.panel,
        layer: registration.layer,
        target_profile: registration.target_profile,
        page_state: registration.page_state,
        owner_alive: registration.owner_alive,
        host_bindings: registration.host_bindings,
    }));
    reload_events.write(UiDocumentReloadEvent(UiDocumentReloadReport {
        report_version: UI_DOCUMENT_RELOAD_REPORT_VERSION,
        reload_id,
        request_id: Some(request_id),
        document_id: key.document_id,
        owner: key.owner,
        source_path: registration.source_path.as_str().to_owned(),
        status: UiDocumentReloadStatus::Queued,
        previous_instance,
        current_instance: previous_instance,
        diff: Some(diff),
        state_decisions: Vec::new(),
        error: None,
    }));
}

fn validation_error(first: Option<&UiValidationDiagnostic>) -> UiDocumentReloadError {
    UiDocumentReloadError {
        code: first
            .map(|diagnostic| diagnostic.code.clone())
            .unwrap_or_else(|| "UI_DOCUMENT_VALIDATION_FAILED".to_owned()),
        stage: UiDocumentReloadStage::Validation,
        document_path: first.map(|diagnostic| diagnostic.document_path.clone()),
        node_id: first.and_then(|diagnostic| diagnostic.node_id.clone()),
        field_path: first.map(|diagnostic| diagnostic.field_path.clone()),
    }
}

fn emit_unregistered_report(
    reload_id: UiDocumentReloadId,
    key: PreviewKey,
    events: &mut MessageWriter<UiDocumentReloadEvent>,
) {
    events.write(UiDocumentReloadEvent(UiDocumentReloadReport {
        report_version: UI_DOCUMENT_RELOAD_REPORT_VERSION,
        reload_id,
        request_id: None,
        document_id: key.document_id,
        owner: key.owner,
        source_path: String::new(),
        status: UiDocumentReloadStatus::Failed,
        previous_instance: None,
        current_instance: None,
        diff: None,
        state_decisions: Vec::new(),
        error: Some(UiDocumentReloadError {
            code: "UI_DOCUMENT_PREVIEW_NOT_REGISTERED".to_owned(),
            stage: UiDocumentReloadStage::Source,
            document_path: None,
            node_id: None,
            field_path: None,
        }),
    }));
}

fn snapshot_document_state(
    instance_id: UiDocumentInstanceId,
    document: Option<&ValidatedUiDocument>,
    focus: Option<&UiFocusState>,
    parents: &Query<&ChildOf>,
    states: &Query<(
        Entity,
        &super::UiDocumentNodeMarker,
        Option<&UiTextInputValue>,
        Option<&UiTextInputCursor>,
        Option<&ScrollPosition>,
        Option<&UiSlider>,
        Option<&UiStepper>,
        Option<&UiDropdown>,
        Option<&UiTab>,
        Option<&UiControlFlags>,
        Has<UiCheckboxChecked>,
        Has<UiToggleOn>,
        Has<SelectedButton>,
    )>,
    segment_options: &Query<(
        &UiSegmentOption,
        Has<UiSegmentOptionSelected>,
        Option<&UiControlFlags>,
        &ChildOf,
    )>,
) -> UiDocumentStateSnapshot {
    let focused_node = focus.and_then(|focus| {
        let mut current = focus.focused_entity?;
        loop {
            if let Ok((_, marker, ..)) = states.get(current)
                && marker.instance_id == instance_id
            {
                break Some(marker.node_id.clone());
            }
            let Ok(parent) = parents.get(current) else {
                break None;
            };
            current = parent.parent();
        }
    });
    let mut nodes = BTreeMap::new();
    for (
        entity,
        marker,
        text_input,
        text_cursor,
        scroll,
        slider,
        stepper,
        select,
        tab,
        flags,
        checkbox,
        toggle,
        selected_button,
    ) in states.iter()
    {
        if marker.instance_id != instance_id {
            continue;
        }
        let protocol_node =
            document.and_then(|document| find_node(&document.document().root, &marker.node_id));
        let mut saved = Vec::new();
        match protocol_node {
            Some(UiNode::TextInput { .. }) => {
                if let Some(value) = text_input {
                    let native = text_cursor.map_or_else(
                        || UiTextInputNativeState {
                            text: value.0.clone(),
                            selection_start: value.0.len(),
                            selection_end: value.0.len(),
                        },
                        |cursor| ui_text_input_native_state_from_value(&value.0, cursor),
                    );
                    saved.push(UiDocumentNodeState::TextInput {
                        value: native.text,
                        selection_start: native.selection_start,
                        selection_end: native.selection_end,
                    });
                    saved.push(UiDocumentNodeState::Unsupported {
                        state: "text_input_native_session",
                        reason: "ime_composition_and_native_keyboard_session_not_migrated",
                    });
                }
            }
            Some(UiNode::Scroll { .. }) => {
                if let Some(position) = scroll {
                    saved.push(UiDocumentNodeState::Scroll {
                        position: position.0,
                    });
                }
            }
            Some(UiNode::Slider { .. }) => {
                if let Some(slider) = slider {
                    saved.push(UiDocumentNodeState::Slider {
                        value: slider.value,
                    });
                }
            }
            Some(UiNode::Stepper { .. }) => {
                if let Some(stepper) = stepper {
                    saved.push(UiDocumentNodeState::Stepper {
                        value: stepper.value,
                    });
                }
            }
            Some(UiNode::Select { .. }) => {
                if let Some(select) = select {
                    saved.push(UiDocumentNodeState::Select {
                        selected_value: select.selected_option().map(|option| option.value.clone()),
                    });
                }
            }
            Some(UiNode::Checkbox { .. }) => {
                saved.push(UiDocumentNodeState::Checkbox {
                    checked: checkbox || flags.is_some_and(|flags| flags.selected),
                });
            }
            Some(UiNode::Toggle { .. }) => {
                saved.push(UiDocumentNodeState::Toggle {
                    on: toggle || flags.is_some_and(|flags| flags.selected),
                });
            }
            Some(UiNode::Segmented { .. }) => {
                let selected_value = segment_options
                    .iter()
                    .filter(|(_, selected, flags, parent)| {
                        parent.parent() == entity
                            && (*selected || flags.is_some_and(|flags| flags.selected))
                    })
                    .map(|(option, ..)| option.value.clone())
                    .min();
                saved.push(UiDocumentNodeState::Segmented { selected_value });
            }
            Some(UiNode::Tab { .. }) => {
                if let Some(tab) = tab {
                    saved.push(UiDocumentNodeState::Tab {
                        value: tab.value.clone(),
                        selected: selected_button || flags.is_some_and(|flags| flags.selected),
                    });
                }
            }
            _ => {}
        }
        if !saved.is_empty() {
            nodes.insert(marker.node_id.clone(), saved);
        }
    }
    UiDocumentStateSnapshot {
        focused_node,
        nodes,
    }
}

fn finish_preview_reloads(world: &mut World) {
    if !world.contains_resource::<UiDocumentPreviewRegistry>() {
        return;
    }
    let completed = {
        let registry = world.resource::<UiDocumentPreviewRegistry>();
        let runtime = world.resource::<UiDocumentRuntime>();
        registry
            .pending
            .keys()
            .filter_map(|request_id| {
                runtime.record(*request_id).filter(|record| {
                    matches!(
                        record.state,
                        UiDocumentBuildState::Committed
                            | UiDocumentBuildState::Failed
                            | UiDocumentBuildState::Cancelled
                    )
                })
            })
            .cloned()
            .collect::<Vec<_>>()
    };
    for record in completed {
        finish_preview_reload(world, record);
    }
}

pub(super) fn finish_committed_preview_reload(world: &mut World, request_id: UiDocumentRequestId) {
    if !world.contains_resource::<UiDocumentPreviewRegistry>() {
        return;
    }
    let record = world
        .resource::<UiDocumentRuntime>()
        .record(request_id)
        .filter(|record| record.state == UiDocumentBuildState::Committed)
        .cloned();
    if let Some(record) = record {
        finish_preview_reload(world, record);
    }
}

fn finish_preview_reload(world: &mut World, record: super::UiDocumentBuildRecord) {
    let pending = world
        .resource_mut::<UiDocumentPreviewRegistry>()
        .pending
        .remove(&record.request_id);
    let Some(pending) = pending else {
        return;
    };
    if record.state == UiDocumentBuildState::Committed {
        let current_instance = record.instance_id.or_else(|| {
            world
                .resource::<UiDocumentRuntime>()
                .active_instance(&pending.key.owner, &pending.key.document_id)
        });
        let decisions = current_instance.map_or_else(Vec::new, |instance_id| {
            restore_document_state(world, instance_id, &pending.snapshot)
        });
        world.write_message(UiDocumentReloadEvent(UiDocumentReloadReport {
            report_version: UI_DOCUMENT_RELOAD_REPORT_VERSION,
            reload_id: pending.reload_id,
            request_id: Some(pending.request_id),
            document_id: pending.key.document_id,
            owner: pending.key.owner,
            source_path: pending.source_path,
            status: UiDocumentReloadStatus::Committed,
            previous_instance: pending.previous_instance,
            current_instance,
            diff: Some(pending.diff),
            state_decisions: decisions,
            error: None,
        }));
    } else {
        let current_instance = world
            .resource::<UiDocumentRuntime>()
            .active_instance(&pending.key.owner, &pending.key.document_id);
        world.write_message(UiDocumentReloadEvent(UiDocumentReloadReport {
            report_version: UI_DOCUMENT_RELOAD_REPORT_VERSION,
            reload_id: pending.reload_id,
            request_id: Some(pending.request_id),
            document_id: pending.key.document_id,
            owner: pending.key.owner,
            source_path: pending.source_path,
            status: if record.state == UiDocumentBuildState::Cancelled {
                UiDocumentReloadStatus::Cancelled
            } else {
                UiDocumentReloadStatus::Failed
            },
            previous_instance: pending.previous_instance,
            current_instance,
            diff: Some(pending.diff),
            state_decisions: Vec::new(),
            error: Some(UiDocumentReloadError {
                code: record
                    .failure_code
                    .unwrap_or_else(|| "UI_DOCUMENT_RELOAD_FAILED".to_owned()),
                stage: reload_stage(record.failure_stage),
                document_path: None,
                node_id: None,
                field_path: None,
            }),
        }));
    }
}

fn reload_stage(stage: Option<UiDocumentFailureStage>) -> UiDocumentReloadStage {
    match stage {
        Some(UiDocumentFailureStage::StaticValidation) => UiDocumentReloadStage::Validation,
        Some(UiDocumentFailureStage::HostValidation) => UiDocumentReloadStage::HostValidation,
        Some(UiDocumentFailureStage::ResourcePreflight) => UiDocumentReloadStage::ResourcePreflight,
        Some(UiDocumentFailureStage::Commit) => UiDocumentReloadStage::Commit,
        Some(UiDocumentFailureStage::Cancel | UiDocumentFailureStage::Cleanup) | None => {
            UiDocumentReloadStage::Cancel
        }
    }
}

fn restore_document_state(
    world: &mut World,
    instance_id: UiDocumentInstanceId,
    snapshot: &UiDocumentStateSnapshot,
) -> Vec<UiDocumentStateDecision> {
    let active = world
        .resource::<UiDocumentRuntime>()
        .active_validated_document_by_instance(instance_id)
        .cloned();
    let Some(active) = active else {
        return Vec::new();
    };
    let mut decisions = Vec::new();
    for (node_id, saved_states) in &snapshot.nodes {
        for saved in saved_states {
            if let UiDocumentNodeState::Unsupported { reason, .. } = saved {
                decisions.push(state_decision(node_id, saved, false, reason));
                continue;
            }
            let entity = world
                .resource::<UiDocumentRuntime>()
                .node_entity(instance_id, node_id);
            let Some(entity) = entity else {
                decisions.push(state_decision(node_id, saved, false, "node_missing"));
                continue;
            };
            let Some(node) = find_node(&active.document().root, node_id) else {
                decisions.push(state_decision(node_id, saved, false, "node_missing"));
                continue;
            };
            let (preserved, reason) = restore_node_state(world, entity, node, saved);
            decisions.push(state_decision(node_id, saved, preserved, reason));
        }
    }
    if let Some(focused_node) = &snapshot.focused_node {
        let focused_entity = world
            .resource::<UiDocumentRuntime>()
            .node_entity(instance_id, focused_node)
            .filter(|entity| world.get::<FocusableButton>(*entity).is_some());
        let preserved = focused_entity.is_some();
        if let Some(entity) = focused_entity
            && let Some(mut focus) = world.get_resource_mut::<UiFocusState>()
        {
            focus.focused_entity = Some(entity);
        }
        decisions.push(UiDocumentStateDecision {
            node_id: focused_node.clone(),
            state: "focus".to_owned(),
            preserved,
            reason: if preserved {
                "semantic_match".to_owned()
            } else {
                "focus_target_missing_or_not_focusable".to_owned()
            },
        });
    }
    decisions.sort_by(|left, right| {
        left.node_id
            .cmp(&right.node_id)
            .then_with(|| left.state.cmp(&right.state))
    });
    decisions
}

fn restore_node_state(
    world: &mut World,
    entity: Entity,
    node: &UiNode,
    saved: &UiDocumentNodeState,
) -> (bool, &'static str) {
    match (saved, node) {
        (
            UiDocumentNodeState::TextInput {
                value,
                selection_start,
                selection_end,
            },
            UiNode::TextInput { .. },
        ) => {
            let max_chars = world.get::<UiTextInputMaxChars>(entity).map(|max| max.0);
            if max_chars.is_some_and(|max| value.chars().count() > max) {
                return (false, "max_chars_tightened");
            }
            let Some(mut text) = world.get::<UiTextInputValue>(entity).cloned() else {
                return (false, "runtime_state_component_missing");
            };
            let Some(mut cursor) = world.get::<UiTextInputCursor>(entity).cloned() else {
                return (false, "runtime_state_component_missing");
            };
            apply_native_text_input_state(
                &mut text.0,
                &mut cursor,
                UiTextInputNativeState {
                    text: value.clone(),
                    selection_start: *selection_start,
                    selection_end: *selection_end,
                },
                max_chars,
            );
            world.entity_mut(entity).insert((text, cursor));
            (true, "value_cursor_selection_preserved")
        }
        (UiDocumentNodeState::Scroll { position }, UiNode::Scroll { .. }) => {
            if !position.is_finite() {
                return (false, "scroll_position_not_finite");
            }
            let Some(mut scroll) = world.get_mut::<ScrollPosition>(entity) else {
                return (false, "runtime_state_component_missing");
            };
            scroll.0 = position.max(Vec2::ZERO);
            (true, "semantic_match")
        }
        (UiDocumentNodeState::Slider { value }, UiNode::Slider { .. }) => {
            let Some(mut slider) = world.get_mut::<UiSlider>(entity) else {
                return (false, "runtime_state_component_missing");
            };
            if *value < slider.min || *value > slider.max {
                return (false, "value_out_of_range");
            }
            slider.value = *value;
            (true, "semantic_match")
        }
        (UiDocumentNodeState::Stepper { value }, UiNode::Stepper { .. }) => {
            let Some(mut stepper) = world.get_mut::<UiStepper>(entity) else {
                return (false, "runtime_state_component_missing");
            };
            if *value < stepper.min || *value > stepper.max {
                return (false, "value_out_of_range");
            }
            stepper.value = *value;
            (true, "semantic_match")
        }
        (UiDocumentNodeState::Select { selected_value }, UiNode::Select { .. }) => {
            let Some(mut select) = world.get_mut::<UiDropdown>(entity) else {
                return (false, "runtime_state_component_missing");
            };
            let selected = selected_value.as_ref().and_then(|value| {
                select
                    .options
                    .iter()
                    .position(|option| &option.value == value && !option.disabled)
            });
            if selected_value.is_some() && selected.is_none() {
                return (false, "option_missing_or_disabled");
            }
            select.selected = selected;
            (true, "semantic_match")
        }
        (UiDocumentNodeState::Checkbox { checked }, UiNode::Checkbox { .. }) => {
            set_protocol_selection::<UiCheckboxChecked>(world, entity, *checked);
            (true, "semantic_match")
        }
        (UiDocumentNodeState::Toggle { on }, UiNode::Toggle { .. }) => {
            set_protocol_selection::<UiToggleOn>(world, entity, *on);
            (true, "semantic_match")
        }
        (UiDocumentNodeState::Segmented { selected_value }, UiNode::Segmented { options, .. }) => {
            if selected_value.as_ref().is_some_and(|selected| {
                !options
                    .iter()
                    .any(|option| &option.value == selected && !option.disabled)
            }) {
                return (false, "option_missing_or_disabled");
            }
            let option_entities = world
                .get::<Children>(entity)
                .map(|children| children.iter().collect::<Vec<_>>())
                .unwrap_or_default();
            if selected_value.as_ref().is_some_and(|selected| {
                !option_entities.iter().any(|candidate| {
                    world
                        .get::<UiSegmentOption>(*candidate)
                        .is_some_and(|option| &option.value == selected)
                })
            }) {
                return (false, "runtime_state_component_missing");
            }
            for option_entity in option_entities {
                let Some(option) = world.get::<UiSegmentOption>(option_entity) else {
                    continue;
                };
                let selected = selected_value.as_ref() == Some(&option.value);
                set_protocol_selection::<UiSegmentOptionSelected>(world, option_entity, selected);
            }
            (true, "semantic_match")
        }
        (
            UiDocumentNodeState::Tab { value, selected },
            UiNode::Tab {
                value: current_value,
                ..
            },
        ) => {
            if value != current_value {
                return (false, "tab_value_changed");
            }
            set_tab_selection(world, entity, *selected);
            (true, "semantic_match")
        }
        (UiDocumentNodeState::Unsupported { reason, .. }, _) => (false, reason),
        _ => (false, "node_kind_changed"),
    }
}

fn set_protocol_selection<T: Component + Default>(
    world: &mut World,
    entity: Entity,
    selected: bool,
) {
    if selected {
        world
            .entity_mut(entity)
            .insert((T::default(), SelectedButton));
    } else {
        world
            .entity_mut(entity)
            .remove::<T>()
            .remove::<SelectedButton>();
    }
    if let Some(mut flags) = world.get_mut::<UiControlFlags>(entity) {
        flags.selected = selected;
    }
}

fn set_selected_button(world: &mut World, entity: Entity, selected: bool) {
    if selected {
        world.entity_mut(entity).insert(SelectedButton);
    } else {
        world.entity_mut(entity).remove::<SelectedButton>();
    }
    if let Some(mut flags) = world.get_mut::<UiControlFlags>(entity) {
        flags.selected = selected;
    }
}

fn set_tab_selection(world: &mut World, entity: Entity, selected: bool) {
    if selected && let Some(parent) = world.get::<ChildOf>(entity).map(ChildOf::parent) {
        let siblings = world
            .get::<Children>(parent)
            .map(|children| children.iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for sibling in siblings {
            if sibling != entity && world.get::<UiTab>(sibling).is_some() {
                set_selected_button(world, sibling, false);
            }
        }
    }
    set_selected_button(world, entity, selected);
}

fn state_decision(
    node_id: &UiNodeId,
    state: &UiDocumentNodeState,
    preserved: bool,
    reason: &str,
) -> UiDocumentStateDecision {
    UiDocumentStateDecision {
        node_id: node_id.clone(),
        state: match state {
            UiDocumentNodeState::TextInput { .. } => "text_input",
            UiDocumentNodeState::Scroll { .. } => "scroll",
            UiDocumentNodeState::Slider { .. } => "slider",
            UiDocumentNodeState::Stepper { .. } => "stepper",
            UiDocumentNodeState::Select { .. } => "select",
            UiDocumentNodeState::Checkbox { .. } => "checkbox",
            UiDocumentNodeState::Toggle { .. } => "toggle",
            UiDocumentNodeState::Segmented { .. } => "segmented",
            UiDocumentNodeState::Tab { .. } => "tab",
            UiDocumentNodeState::Unsupported { state, .. } => state,
        }
        .to_owned(),
        preserved,
        reason: reason.to_owned(),
    }
}

pub fn diff_ui_documents(old: &UiDocument, new: &UiDocument) -> UiDocumentDiff {
    let mut page_reasons = Vec::new();
    if old.schema_version != new.schema_version {
        page_reasons.push("schema_version_changed".to_owned());
    }
    if old.document_id != new.document_id {
        page_reasons.push("document_id_changed".to_owned());
    }
    if old.assets != new.assets {
        page_reasons.push("asset_table_changed".to_owned());
    }
    if old.tokens != new.tokens || old.styles != new.styles {
        page_reasons.push("style_table_changed".to_owned());
    }
    if old.bindings != new.bindings {
        page_reasons.push("binding_schema_changed".to_owned());
    }
    if old.metadata != new.metadata {
        page_reasons.push("metadata_changed".to_owned());
    }
    if !page_reasons.is_empty() {
        return UiDocumentDiff {
            kind: UiDocumentDiffKind::RebuildPage,
            in_place_nodes: Vec::new(),
            rebuild_subtrees: Vec::new(),
            page_reasons,
        };
    }

    let old_nodes = flatten_nodes(&old.root);
    let new_nodes = flatten_nodes(&new.root);
    if old.root.id() != new.root.id() || node_kind(&old.root) != node_kind(&new.root) {
        return UiDocumentDiff {
            kind: UiDocumentDiffKind::RebuildPage,
            in_place_nodes: Vec::new(),
            rebuild_subtrees: Vec::new(),
            page_reasons: vec!["root_identity_changed".to_owned()],
        };
    }

    let old_ids = old_nodes.keys().cloned().collect::<BTreeSet<_>>();
    let new_ids = new_nodes.keys().cloned().collect::<BTreeSet<_>>();
    let mut subtree_candidates = BTreeSet::new();
    for node_id in old_ids.symmetric_difference(&new_ids) {
        let parent = old_nodes
            .get(node_id)
            .or_else(|| new_nodes.get(node_id))
            .and_then(|entry| entry.parent.clone())
            .unwrap_or_else(|| old.root.id().clone());
        subtree_candidates.insert(parent);
    }

    let mut in_place = BTreeSet::new();
    for node_id in old_ids.intersection(&new_ids) {
        let old_entry = &old_nodes[node_id];
        let new_entry = &new_nodes[node_id];
        if old_entry.parent != new_entry.parent
            || old_entry.index != new_entry.index
            || old_entry.kind != new_entry.kind
        {
            subtree_candidates.insert(
                old_entry
                    .parent
                    .clone()
                    .unwrap_or_else(|| old.root.id().clone()),
            );
            subtree_candidates.insert(
                new_entry
                    .parent
                    .clone()
                    .unwrap_or_else(|| new.root.id().clone()),
            );
            continue;
        }
        if old_entry.value == new_entry.value {
            continue;
        }
        if only_layout_or_style_changed(&old_entry.value, &new_entry.value) {
            in_place.insert(node_id.clone());
        } else {
            subtree_candidates.insert(node_id.clone());
        }
    }

    let rebuild_subtrees =
        remove_descendant_candidates(&subtree_candidates, &new_nodes, &old_nodes);
    in_place.retain(|node_id| {
        !rebuild_subtrees
            .iter()
            .any(|root| is_descendant_of(node_id, root, &new_nodes))
    });
    let kind = if !rebuild_subtrees.is_empty() {
        UiDocumentDiffKind::RebuildSubtrees
    } else if !in_place.is_empty() {
        UiDocumentDiffKind::InPlace
    } else {
        UiDocumentDiffKind::NoChanges
    };
    UiDocumentDiff {
        kind,
        in_place_nodes: in_place.into_iter().collect(),
        rebuild_subtrees,
        page_reasons: Vec::new(),
    }
}

#[derive(Clone)]
struct FlatNode {
    parent: Option<UiNodeId>,
    index: usize,
    kind: &'static str,
    value: Value,
}

fn flatten_nodes(root: &UiNode) -> BTreeMap<UiNodeId, FlatNode> {
    fn visit(
        node: &UiNode,
        parent: Option<UiNodeId>,
        index: usize,
        result: &mut BTreeMap<UiNodeId, FlatNode>,
    ) {
        result.insert(
            node.id().clone(),
            FlatNode {
                parent: parent.clone(),
                index,
                kind: node_kind(node),
                value: serde_json::to_value(node).expect("validated node serializes"),
            },
        );
        for (child_index, child) in node.children().iter().enumerate() {
            visit(child, Some(node.id().clone()), child_index, result);
        }
    }
    let mut result = BTreeMap::new();
    visit(root, None, 0, &mut result);
    result
}

fn only_layout_or_style_changed(old: &Value, new: &Value) -> bool {
    let mut old = old.clone();
    let mut new = new.clone();
    remove_children(&mut old);
    remove_children(&mut new);
    remove_layout_style(&mut old);
    remove_layout_style(&mut new);
    old == new
}

fn remove_children(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    object.remove("children");
    if let Some(component) = object.get_mut("component").and_then(Value::as_object_mut) {
        component.remove("children");
    }
}

fn remove_layout_style(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.remove("layout");
        object.remove("style");
    }
}

fn remove_descendant_candidates(
    candidates: &BTreeSet<UiNodeId>,
    new_nodes: &BTreeMap<UiNodeId, FlatNode>,
    old_nodes: &BTreeMap<UiNodeId, FlatNode>,
) -> Vec<UiNodeId> {
    candidates
        .iter()
        .filter(|candidate| {
            !candidates.iter().any(|other| {
                other != *candidate
                    && (is_descendant_of(candidate, other, new_nodes)
                        || is_descendant_of(candidate, other, old_nodes))
            })
        })
        .cloned()
        .collect()
}

fn is_descendant_of(
    node_id: &UiNodeId,
    ancestor: &UiNodeId,
    nodes: &BTreeMap<UiNodeId, FlatNode>,
) -> bool {
    let mut current = nodes.get(node_id).and_then(|entry| entry.parent.as_ref());
    while let Some(parent) = current {
        if parent == ancestor {
            return true;
        }
        current = nodes.get(parent).and_then(|entry| entry.parent.as_ref());
    }
    false
}

fn find_node<'a>(node: &'a UiNode, node_id: &UiNodeId) -> Option<&'a UiNode> {
    if node.id() == node_id {
        return Some(node);
    }
    node.children()
        .iter()
        .find_map(|child| find_node(child, node_id))
}

fn node_kind(node: &UiNode) -> &'static str {
    match node {
        UiNode::Container { .. } => "container",
        UiNode::Text { .. } => "text",
        UiNode::Image { .. } => "image",
        UiNode::Icon { .. } => "icon",
        UiNode::Spacer { .. } => "spacer",
        UiNode::Button { .. } => "button",
        UiNode::TextInput { .. } => "text_input",
        UiNode::Checkbox { .. } => "checkbox",
        UiNode::Toggle { .. } => "toggle",
        UiNode::Segmented { .. } => "segmented",
        UiNode::Slider { .. } => "slider",
        UiNode::Stepper { .. } => "stepper",
        UiNode::Scroll { .. } => "scroll",
        UiNode::Modal { .. } => "modal",
        UiNode::ImageButton { .. } => "image_button",
        UiNode::Badge { .. } => "badge",
        UiNode::Progress { .. } => "progress",
        UiNode::Tab { .. } => "tab",
        UiNode::Tooltip { .. } => "tooltip",
        UiNode::Select { .. } => "select",
    }
}

pub(crate) fn target_profile_from_viewport(viewport: &UiViewport) -> UiTargetProfile {
    let safe_area = if viewport.safe_area.left > 0.0
        || viewport.safe_area.right > 0.0
        || viewport.safe_area.top > 0.0
        || viewport.safe_area.bottom > 0.0
    {
        super::UiSafeAreaClass::Inset
    } else {
        super::UiSafeAreaClass::None
    };
    let input_mode = match viewport.input_mode {
        UiInputMode::Touch => super::UiDocumentInputMode::Touch,
        UiInputMode::MouseKeyboard => super::UiDocumentInputMode::MouseKeyboard,
        UiInputMode::MouseTouch => super::UiDocumentInputMode::MouseTouch,
    };
    let platform = if cfg!(target_os = "android") {
        super::UiDocumentPlatform::Android
    } else if cfg!(target_os = "ios") {
        super::UiDocumentPlatform::Ios
    } else if cfg!(target_os = "macos") {
        super::UiDocumentPlatform::Macos
    } else if cfg!(target_os = "linux") {
        super::UiDocumentPlatform::Linux
    } else if cfg!(target_arch = "wasm32") {
        super::UiDocumentPlatform::Web
    } else {
        super::UiDocumentPlatform::Windows
    };
    UiTargetProfile::new(
        viewport.logical_width,
        viewport.logical_height,
        safe_area,
        input_mode,
        platform,
    )
    .expect("runtime viewport dimensions are finite and positive")
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::message::MessageCursor;
    use std::str::FromStr;

    use crate::framework::ui::{
        core::{UiInputState, UiMetrics, UiPanelKind, UiPanelRoot, input::UiInputPlugin},
        style::{UiFontAssets, UiTheme},
        widgets::controls::UiTextInputSelection,
    };

    fn document(source: &str) -> UiDocument {
        UiDocument::validate_json(source)
            .validated()
            .expect("fixture must validate")
            .document()
            .clone()
    }

    fn base_document(child: &str) -> String {
        format!(
            r#"{{
              "schema_version": 1,
              "document_id": "preview.diff",
              "root": {{
                "type": "container",
                "id": "preview.root",
                "children": [{child}]
              }}
            }}"#
        )
    }

    #[test]
    fn ui_document_source_path_rejects_escape_and_absolute_inputs() {
        assert!(
            UiDocumentSourcePath::new(UiDocumentSourceRoot::Approved, "gallery/page.json").is_ok()
        );
        for invalid in [
            "../page.json",
            "/page.json",
            "C:/page.json",
            "a\\page.json",
            "A.json",
            "a.ron",
        ] {
            assert!(UiDocumentSourcePath::new(UiDocumentSourceRoot::Approved, invalid).is_err());
        }
        assert_eq!(
            UiDocumentSourceOrigin::Preview {
                source_path: "C:/users/name/page.json".to_owned(),
            }
            .audit_source_path(),
            "preview"
        );
    }

    #[test]
    fn ui_document_source_resolution_requires_canonical_root_containment() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = env::temp_dir().join(format!(
            "mybevy-ui-document-source-{}-{unique}",
            std::process::id()
        ));
        let root = base.join("approved");
        let sibling = base.join("approved-escape");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&sibling).unwrap();
        let inside = root.join("inside.json");
        let outside = sibling.join("outside.json");
        fs::write(&inside, "{}").unwrap();
        fs::write(&outside, "{}").unwrap();

        assert_eq!(
            resolve_contained_source_path(&base, &root, &inside).unwrap(),
            fs::canonicalize(&inside).unwrap()
        );
        assert_eq!(
            resolve_contained_source_path(&base, &root, &outside),
            Err(UI_DOCUMENT_SOURCE_OUTSIDE_ROOT)
        );
        assert_eq!(
            resolve_contained_source_path(&root, &sibling, &outside),
            Err(UI_DOCUMENT_SOURCE_OUTSIDE_ROOT)
        );
        assert!(canonical_path_is_within_root(
            Path::new("/safe/root"),
            Path::new("/safe/root/page.json")
        ));
        assert!(!canonical_path_is_within_root(
            Path::new("/safe/root"),
            Path::new("/safe/root-escape/page.json")
        ));

        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn ui_document_watch_state_reports_once_and_reloads_after_recovery() {
        let first = SourceSignature {
            modified: None,
            len: 10,
        };
        let changed = SourceSignature {
            modified: None,
            len: 11,
        };
        let (missing, action) = source_watch_transition(
            &SourceWatchState::Uninitialized,
            Err(UI_DOCUMENT_SOURCE_READ_FAILED),
        );
        assert_eq!(
            action,
            SourceWatchAction::Report(UI_DOCUMENT_SOURCE_READ_FAILED)
        );
        let (still_missing, action) =
            source_watch_transition(&missing, Err(UI_DOCUMENT_SOURCE_READ_FAILED));
        assert_eq!(still_missing, missing);
        assert_eq!(action, SourceWatchAction::Ignore);

        let (ready, action) = source_watch_transition(&missing, Ok(first));
        assert_eq!(action, SourceWatchAction::Reload);
        let (same, action) = source_watch_transition(&ready, Ok(first));
        assert_eq!(same, ready);
        assert_eq!(action, SourceWatchAction::Ignore);
        let (changed_state, action) = source_watch_transition(&ready, Ok(changed));
        assert_eq!(action, SourceWatchAction::Reload);

        let (deleted, action) =
            source_watch_transition(&changed_state, Err(UI_DOCUMENT_SOURCE_READ_FAILED));
        assert_eq!(
            action,
            SourceWatchAction::Report(UI_DOCUMENT_SOURCE_READ_FAILED)
        );
        let (_, action) = source_watch_transition(&deleted, Ok(changed));
        assert_eq!(action, SourceWatchAction::Reload);
    }

    #[test]
    fn ui_document_diff_has_stable_in_place_subtree_and_page_classes() {
        let old = document(&base_document(
            r#"{"type":"text","id":"preview.label","content":{"literal":"A"}}"#,
        ));
        let style = document(&base_document(
            r#"{"type":"text","id":"preview.label","content":{"literal":"A"},"layout":{"width":{"px":120}}}"#,
        ));
        let content = document(&base_document(
            r#"{"type":"text","id":"preview.label","content":{"literal":"B"}}"#,
        ));
        let inserted = document(&base_document(
            r#"{"type":"container","id":"preview.group","children":[{"type":"text","id":"preview.label","content":{"literal":"A"}}]}"#,
        ));
        let mut bindings = old.clone();
        bindings.bindings.insert(
            super::super::UiBindingPath::from_str("state.value").unwrap(),
            super::super::UiBindingDeclaration {
                scope: super::super::UiBindingScope::Local,
                value_type: super::super::UiBindingType::String,
                default: None,
                missing: super::super::UiBindingMissingBehavior::UseConsumerFallback,
            },
        );

        assert_eq!(
            diff_ui_documents(&old, &old).kind,
            UiDocumentDiffKind::NoChanges
        );
        assert_eq!(
            diff_ui_documents(&old, &style).kind,
            UiDocumentDiffKind::InPlace
        );
        assert_eq!(
            diff_ui_documents(&old, &content).kind,
            UiDocumentDiffKind::RebuildSubtrees
        );
        assert_eq!(
            diff_ui_documents(&old, &inserted).kind,
            UiDocumentDiffKind::RebuildSubtrees
        );
        assert_eq!(
            diff_ui_documents(&old, &bindings).kind,
            UiDocumentDiffKind::RebuildPage
        );
    }

    #[test]
    fn ui_document_audit_profiles_are_allowlisted_and_defaulted() {
        assert_eq!(
            normalized_audit_profiles(&["desktop".to_owned(), "phone-small".to_owned()]),
            vec!["phone-small"]
        );
        assert!(normalized_audit_profiles(&[]).contains(&"tablet-landscape".to_owned()));
    }

    #[test]
    fn ui_document_audit_recipes_are_isolated_by_document_and_owner() {
        let mut app = App::new();
        app.add_plugins((
            super::super::UiDocumentRuntimePlugin,
            UiDocumentPreviewPlugin,
        ));
        let document_id = UiDocumentId::from_str("preview.runtime").unwrap();
        let mut owner_a = preview_registration(preview_document(None));
        owner_a.owner = "owner_a".to_owned();
        owner_a.source_path =
            UiDocumentSourcePath::new(UiDocumentSourceRoot::Fixture, "preview/owner_a.json")
                .unwrap();
        owner_a.open_on_register = false;
        owner_a.audit_profiles = vec!["phone-small".to_owned()];
        let mut owner_b = owner_a.clone();
        owner_b.owner = "owner_b".to_owned();
        owner_b.source_path =
            UiDocumentSourcePath::new(UiDocumentSourceRoot::Fixture, "preview/owner_b.json")
                .unwrap();
        owner_b.audit_profiles = vec!["tablet-landscape".to_owned()];

        app.world_mut()
            .write_message(UiDocumentPreviewCommand::Register(owner_a));
        app.world_mut()
            .write_message(UiDocumentPreviewCommand::Register(owner_b));
        app.update();
        let recipes = app.world().resource::<UiDocumentAuditRecipeRegistry>();
        assert_eq!(recipes.entries().count(), 2);
        assert_eq!(
            recipes.entry(&document_id, "owner_a").unwrap().source_path,
            "ui/documents/fixtures/preview/owner_a.json"
        );
        assert_eq!(
            recipes.entry(&document_id, "owner_b").unwrap().profiles,
            vec!["tablet-landscape"]
        );

        app.world_mut()
            .write_message(UiDocumentPreviewCommand::Unregister {
                document_id: document_id.clone(),
                owner: "owner_b".to_owned(),
            });
        app.update();
        let recipes = app.world().resource::<UiDocumentAuditRecipeRegistry>();
        assert!(recipes.entry(&document_id, "owner_b").is_none());
        assert!(recipes.entry(&document_id, "owner_a").is_some());

        app.world_mut()
            .write_message(UiDocumentPreviewCommand::Unregister {
                document_id: document_id.clone(),
                owner: "owner_a".to_owned(),
            });
        app.update();
        assert!(
            app.world()
                .resource::<UiDocumentAuditRecipeRegistry>()
                .entry(&document_id, "owner_a")
                .is_none()
        );
    }

    #[test]
    fn release_watch_policy_is_compile_time_closed() {
        let config = UiDocumentPreviewConfig::default();
        if !cfg!(all(debug_assertions, not(target_os = "android"))) {
            assert!(!config.watch_supported());
            assert!(!config.watch_enabled());
        }
    }

    fn preview_document(layout_width: Option<u32>) -> String {
        let layout = layout_width.map_or_else(String::new, |width| {
            format!(r#","layout":{{"width":{{"px":{width}}}}}"#)
        });
        format!(
            r#"{{
              "schema_version": 1,
              "document_id": "preview.runtime",
              "root": {{
                "type": "slider",
                "id": "preview.slider",
                "value": 0.25,
                "min": 0.0,
                "max": 1.0,
                "component": {{
                  "slots": {{
                    "label": {{"kind":"text","content":{{"literal":"Scale"}}}}
                  }}
                }}
                {layout}
              }}
            }}"#
        )
    }

    fn preview_registration(source_json: String) -> UiDocumentPreviewRegistration {
        UiDocumentPreviewRegistration {
            document_id: UiDocumentId::from_str("preview.runtime").unwrap(),
            owner: "preview_owner".to_owned(),
            source_path: UiDocumentSourcePath::new(
                UiDocumentSourceRoot::Fixture,
                "preview/runtime.json",
            )
            .unwrap(),
            source_json,
            panel: UiDocumentPanel::Page,
            layer: UiDocumentLayer::Page,
            target_profile: UiTargetProfile::new(
                390.0,
                844.0,
                super::super::UiSafeAreaClass::None,
                super::super::UiDocumentInputMode::MouseTouch,
                super::super::UiDocumentPlatform::Windows,
            )
            .unwrap(),
            page_state: UiPageState::initial(),
            owner_alive: true,
            host_bindings: BTreeMap::new(),
            watch: false,
            open_on_register: true,
            audit_profiles: Vec::new(),
        }
    }

    fn host_invalid_document() -> String {
        r#"{
          "schema_version": 1,
          "document_id": "preview.runtime",
          "root": {
            "type": "button",
            "id": "preview.button",
            "label": { "literal": "Unknown action" },
            "on_click": { "action": "preview.unknown" }
          }
        }"#
        .to_owned()
    }

    fn stateful_preview_document(
        layout_width: u32,
        max_chars: u32,
        include_second_options: bool,
        checkbox_kind_changed: bool,
        details_tab_value: &str,
    ) -> String {
        let mut segmented_options = vec![serde_json::json!({
            "value": "one",
            "label": { "literal": "One" }
        })];
        let mut select_options = vec![serde_json::json!({
            "value": "one",
            "label": { "literal": "One" }
        })];
        if include_second_options {
            segmented_options.push(serde_json::json!({
                "value": "two",
                "label": { "literal": "Two" }
            }));
            select_options.push(serde_json::json!({
                "value": "two",
                "label": { "literal": "Two" }
            }));
        }
        let checkbox = if checkbox_kind_changed {
            serde_json::json!({
                "type": "toggle",
                "id": "state.checkbox",
                "on": false,
                "component": { "slots": { "label": {
                    "kind": "text", "content": { "literal": "Changed kind" }
                } } }
            })
        } else {
            serde_json::json!({
                "type": "checkbox",
                "id": "state.checkbox",
                "checked": true,
                "component": {
                    "states": ["selected"],
                    "slots": { "label": {
                        "kind": "text", "content": { "literal": "Checkbox" }
                    } }
                }
            })
        };
        serde_json::json!({
            "schema_version": 1,
            "document_id": "preview.runtime",
            "root": {
                "type": "container",
                "id": "state.root",
                "layout": { "width": { "px": layout_width } },
                "children": [
                    {
                        "type": "text_input",
                        "id": "state.input",
                        "value": "ok",
                        "max_chars": max_chars,
                        "component": { "slots": {
                            "label": { "kind": "text", "content": { "literal": "Input" } },
                            "placeholder": { "kind": "text", "content": { "literal": "Value" } }
                        } }
                    },
                    {
                        "type": "scroll",
                        "id": "state.scroll",
                        "row_gap": 4.0,
                        "max_height": 100.0,
                        "component": { "children": [{
                            "type": "text",
                            "id": "state.scroll_text",
                            "content": { "literal": "Scrollable" }
                        }] }
                    },
                    {
                        "type": "slider",
                        "id": "state.slider",
                        "value": 0.25,
                        "min": 0.0,
                        "max": 1.0,
                        "component": { "slots": { "label": {
                            "kind": "text", "content": { "literal": "Slider" }
                        } } }
                    },
                    {
                        "type": "stepper",
                        "id": "state.stepper",
                        "value": 2,
                        "min": 0,
                        "max": 8,
                        "step": 1,
                        "component": { "slots": { "label": {
                            "kind": "text", "content": { "literal": "Stepper" }
                        } } }
                    },
                    {
                        "type": "select",
                        "id": "state.select",
                        "selected": "one",
                        "options": select_options,
                        "component": { "slots": { "label": {
                            "kind": "text", "content": { "literal": "Select" }
                        } } }
                    },
                    checkbox,
                    {
                        "type": "toggle",
                        "id": "state.toggle",
                        "on": false,
                        "component": { "slots": { "label": {
                            "kind": "text", "content": { "literal": "Toggle" }
                        } } }
                    },
                    {
                        "type": "segmented",
                        "id": "state.segmented",
                        "selected": "one",
                        "options": segmented_options,
                        "component": { "slots": { "label": {
                            "kind": "text", "content": { "literal": "Segmented" }
                        } } }
                    },
                    {
                        "type": "container",
                        "id": "state.tabs",
                        "children": [
                            {
                                "type": "tab",
                                "id": "state.tab_overview",
                                "value": "overview",
                                "component": {
                                    "states": ["selected"],
                                    "slots": { "label": {
                                        "kind": "text", "content": { "literal": "Overview" }
                                    } }
                                }
                            },
                            {
                                "type": "tab",
                                "id": "state.tab_details",
                                "value": details_tab_value,
                                "component": { "slots": { "label": {
                                    "kind": "text", "content": { "literal": "Details" }
                                } } }
                            }
                        ]
                    }
                ]
            },
            "states": [{
                "id": "loading",
                "overrides": [{
                    "node_id": "state.root",
                    "set": { "style": { "inline": {
                        "opacity": { "kind": "literal", "value": 0.6 }
                    } } }
                }]
            }]
        })
        .to_string()
    }

    fn resource_preview_document() -> String {
        serde_json::json!({
            "schema_version": 1,
            "document_id": "preview.runtime",
            "assets": {
                "preview_image": {
                    "kind": "icon",
                    "source": { "kind": "packaged", "path": "ui/runtime/preview.png" }
                }
            },
            "root": {
                "type": "icon",
                "id": "preview.image",
                "asset": "preview_image"
            }
        })
        .to_string()
    }

    fn stateful_preview_app(source: String) -> App {
        let mut app = App::new();
        app.insert_resource(UiTheme::default());
        app.insert_resource(UiMetrics::default());
        app.insert_resource(UiFontAssets::test_registry());
        app.init_resource::<UiFocusState>();
        app.add_plugins((
            super::super::UiDocumentRuntimePlugin,
            UiDocumentPreviewPlugin,
        ));
        app.world_mut()
            .write_message(UiDocumentPreviewCommand::Register(preview_registration(
                source,
            )));
        app.update();
        app.update();
        let document_id = UiDocumentId::from_str("preview.runtime").unwrap();
        if app
            .world()
            .resource::<UiDocumentRuntime>()
            .active_instance("preview_owner", &document_id)
            .is_none()
        {
            panic!(
                "stateful preview did not commit: {:#?}",
                reload_report(&app, 1)
            );
        }
        app
    }

    fn modal_preview_document() -> String {
        r#"{
          "schema_version": 1,
          "document_id": "preview.runtime",
          "root": {
            "type": "modal",
            "id": "modal.dialog",
            "cancellable": true,
            "layout": {
              "width": { "percent": 100 },
              "min_height": { "px": 176 },
              "padding": { "all": { "px": 24 } }
            },
            "component": {
              "slots": {
                "title": { "kind": "text", "content": { "literal": "Confirm action" } },
                "body": { "kind": "text", "content": { "literal": "This is a formal modal node." } }
              }
            }
          }
        }"#
        .to_owned()
    }

    fn modal_preview_app() -> App {
        let mut app = App::new();
        app.insert_resource(UiTheme::default());
        app.insert_resource(UiMetrics::default());
        app.insert_resource(UiFontAssets::test_registry());
        app.init_resource::<UiFocusState>();
        app.add_plugins((
            super::super::UiDocumentRuntimePlugin,
            UiDocumentPreviewPlugin,
            UiInputPlugin,
        ));
        let mut registration = preview_registration(modal_preview_document());
        registration.panel = UiDocumentPanel::Modal;
        registration.layer = UiDocumentLayer::Modal;
        app.world_mut()
            .write_message(UiDocumentPreviewCommand::Register(registration));
        app.update();
        app.update();
        app.update();
        app
    }

    fn active_node(app: &App, owner: &str, node_id: &str) -> Entity {
        active_node_for_document(app, owner, "preview.runtime", node_id)
    }

    fn active_node_for_document(
        app: &App,
        owner: &str,
        document_id: &str,
        node_id: &str,
    ) -> Entity {
        let document_id = UiDocumentId::from_str(document_id).unwrap();
        let runtime = app.world().resource::<UiDocumentRuntime>();
        let instance = runtime.active_instance(owner, &document_id).unwrap();
        runtime
            .node_entity(instance, &UiNodeId::from_str(node_id).unwrap())
            .unwrap()
    }

    fn set_segmented_value(app: &mut App, owner: &str, value: &str) {
        let segmented = active_node(app, owner, "state.segmented");
        let children = app
            .world()
            .get::<Children>(segmented)
            .unwrap()
            .iter()
            .collect::<Vec<_>>();
        for child in children {
            let selected = app
                .world()
                .get::<UiSegmentOption>(child)
                .is_some_and(|option| option.value == value);
            if app.world().get::<UiSegmentOption>(child).is_some() {
                set_protocol_selection::<UiSegmentOptionSelected>(app.world_mut(), child, selected);
            }
        }
    }

    fn selected_segmented_value(app: &App, owner: &str) -> Option<String> {
        let segmented = active_node(app, owner, "state.segmented");
        app.world()
            .get::<Children>(segmented)
            .into_iter()
            .flat_map(|children| children.iter())
            .filter(|child| {
                app.world().get::<UiSegmentOptionSelected>(*child).is_some()
                    || app
                        .world()
                        .get::<UiControlFlags>(*child)
                        .is_some_and(|flags| flags.selected)
            })
            .filter_map(|child| app.world().get::<UiSegmentOption>(child))
            .map(|option| option.value.clone())
            .min()
    }

    fn reload_report(app: &App, reload_id: u64) -> UiDocumentReloadReport {
        let messages = app.world().resource::<Messages<UiDocumentReloadEvent>>();
        let mut cursor = MessageCursor::default();
        cursor
            .read(messages)
            .map(|event| event.0.clone())
            .filter(|report| report.reload_id == UiDocumentReloadId(reload_id))
            .last()
            .unwrap()
    }

    fn state_decision_for<'a>(
        report: &'a UiDocumentReloadReport,
        node_id: &str,
        state: &str,
    ) -> &'a UiDocumentStateDecision {
        report
            .state_decisions
            .iter()
            .find(|decision| decision.node_id.as_str() == node_id && decision.state == state)
            .unwrap_or_else(|| panic!("missing {node_id}/{state} decision in {report:#?}"))
    }

    #[test]
    fn ui_document_reload_preserves_all_supported_local_state_and_reports_rejections() {
        let initial = stateful_preview_document(200, 32, true, false, "details");
        let mut app = stateful_preview_app(initial.clone());

        let mut other = preview_registration(preview_document(None));
        other.owner = "other_owner".to_owned();
        other.source_path =
            UiDocumentSourcePath::new(UiDocumentSourceRoot::Fixture, "preview/other_owner.json")
                .unwrap();
        app.world_mut()
            .write_message(UiDocumentPreviewCommand::Register(other));
        app.update();
        app.update();
        let other_instance = app
            .world()
            .resource::<UiDocumentRuntime>()
            .active_instance(
                "other_owner",
                &UiDocumentId::from_str("preview.runtime").unwrap(),
            )
            .unwrap();
        let mut other_document = preview_registration(
            preview_document(None).replace("preview.runtime", "preview.other"),
        );
        other_document.document_id = UiDocumentId::from_str("preview.other").unwrap();
        other_document.source_path =
            UiDocumentSourcePath::new(UiDocumentSourceRoot::Fixture, "preview/other_document.json")
                .unwrap();
        app.world_mut()
            .write_message(UiDocumentPreviewCommand::Register(other_document));
        app.update();
        app.update();
        let other_document_instance = app
            .world()
            .resource::<UiDocumentRuntime>()
            .active_instance(
                "preview_owner",
                &UiDocumentId::from_str("preview.other").unwrap(),
            )
            .unwrap();

        let input = active_node(&app, "preview_owner", "state.input");
        app.world_mut().entity_mut(input).insert((
            UiTextInputValue("aé🙂z".to_owned()),
            UiTextInputCursor {
                position: 7,
                selection: Some(UiTextInputSelection { start: 1, end: 7 }),
            },
        ));
        app.world_mut()
            .resource_mut::<UiFocusState>()
            .focused_entity = Some(input);
        let scroll = active_node(&app, "preview_owner", "state.scroll");
        let slider = active_node(&app, "preview_owner", "state.slider");
        let stepper = active_node(&app, "preview_owner", "state.stepper");
        let select = active_node(&app, "preview_owner", "state.select");
        app.world_mut().get_mut::<ScrollPosition>(scroll).unwrap().0 = Vec2::new(12.0, 34.0);
        app.world_mut().get_mut::<UiSlider>(slider).unwrap().value = 0.8;
        app.world_mut().get_mut::<UiStepper>(stepper).unwrap().value = 4;
        app.world_mut()
            .get_mut::<UiDropdown>(select)
            .unwrap()
            .selected = Some(1);
        let checkbox = active_node(&app, "preview_owner", "state.checkbox");
        set_protocol_selection::<UiCheckboxChecked>(app.world_mut(), checkbox, false);
        let toggle = active_node(&app, "preview_owner", "state.toggle");
        set_protocol_selection::<UiToggleOn>(app.world_mut(), toggle, true);
        set_segmented_value(&mut app, "preview_owner", "two");
        let overview = active_node(&app, "preview_owner", "state.tab_overview");
        let details = active_node(&app, "preview_owner", "state.tab_details");
        set_selected_button(app.world_mut(), overview, false);
        set_selected_button(app.world_mut(), details, true);

        app.world_mut()
            .write_message(UiDocumentPreviewCommand::ReloadSource {
                reload_id: UiDocumentReloadId(50),
                document_id: UiDocumentId::from_str("preview.runtime").unwrap(),
                owner: "preview_owner".to_owned(),
                source_json: stateful_preview_document(240, 32, true, false, "details"),
            });
        app.update();

        let report = reload_report(&app, 50);
        assert_eq!(report.status, UiDocumentReloadStatus::Committed);
        for (node_id, state) in [
            ("state.input", "text_input"),
            ("state.scroll", "scroll"),
            ("state.slider", "slider"),
            ("state.stepper", "stepper"),
            ("state.select", "select"),
            ("state.checkbox", "checkbox"),
            ("state.toggle", "toggle"),
            ("state.segmented", "segmented"),
            ("state.tab_overview", "tab"),
            ("state.tab_details", "tab"),
            ("state.input", "focus"),
        ] {
            assert!(state_decision_for(&report, node_id, state).preserved);
        }
        let native_session =
            state_decision_for(&report, "state.input", "text_input_native_session");
        assert!(!native_session.preserved);
        assert_eq!(
            native_session.reason,
            "ime_composition_and_native_keyboard_session_not_migrated"
        );

        let new_input = active_node(&app, "preview_owner", "state.input");
        assert_eq!(
            app.world().get::<UiTextInputValue>(new_input).unwrap().0,
            "aé🙂z"
        );
        let cursor = app.world().get::<UiTextInputCursor>(new_input).unwrap();
        assert_eq!(cursor.position, 7);
        assert_eq!(
            cursor.selection,
            Some(UiTextInputSelection { start: 1, end: 7 })
        );
        assert_eq!(
            app.world().resource::<UiFocusState>().focused_entity,
            Some(new_input)
        );
        assert_eq!(
            app.world()
                .get::<ScrollPosition>(active_node(&app, "preview_owner", "state.scroll"))
                .unwrap()
                .0,
            Vec2::new(12.0, 34.0)
        );
        assert_eq!(
            app.world()
                .get::<UiSlider>(active_node(&app, "preview_owner", "state.slider"))
                .unwrap()
                .value,
            0.8
        );
        assert_eq!(
            app.world()
                .get::<UiStepper>(active_node(&app, "preview_owner", "state.stepper"))
                .unwrap()
                .value,
            4
        );
        assert_eq!(
            app.world()
                .get::<UiDropdown>(active_node(&app, "preview_owner", "state.select"))
                .unwrap()
                .selected_option()
                .unwrap()
                .value,
            "two"
        );
        assert!(
            !app.world()
                .entity(active_node(&app, "preview_owner", "state.checkbox"))
                .contains::<UiCheckboxChecked>()
        );
        assert!(
            app.world()
                .entity(active_node(&app, "preview_owner", "state.toggle"))
                .contains::<UiToggleOn>()
        );
        assert_eq!(
            selected_segmented_value(&app, "preview_owner").as_deref(),
            Some("two")
        );
        assert!(
            !app.world()
                .get::<UiControlFlags>(active_node(&app, "preview_owner", "state.tab_overview"))
                .unwrap()
                .selected
        );
        assert!(
            app.world()
                .get::<UiControlFlags>(active_node(&app, "preview_owner", "state.tab_details"))
                .unwrap()
                .selected
        );

        let runtime = app.world().resource::<UiDocumentRuntime>();
        assert_eq!(
            runtime.active_instance(
                "other_owner",
                &UiDocumentId::from_str("preview.runtime").unwrap()
            ),
            Some(other_instance)
        );
        assert_eq!(
            app.world()
                .get::<UiSlider>(active_node(&app, "other_owner", "preview.slider"))
                .unwrap()
                .value,
            0.25
        );
        assert_eq!(
            app.world().resource::<UiDocumentRuntime>().active_instance(
                "preview_owner",
                &UiDocumentId::from_str("preview.other").unwrap()
            ),
            Some(other_document_instance)
        );
        assert_eq!(
            app.world()
                .get::<UiSlider>(active_node_for_document(
                    &app,
                    "preview_owner",
                    "preview.other",
                    "preview.slider"
                ))
                .unwrap()
                .value,
            0.25
        );

        app.world_mut()
            .write_message(UiDocumentPreviewCommand::ReloadSource {
                reload_id: UiDocumentReloadId(51),
                document_id: UiDocumentId::from_str("preview.runtime").unwrap(),
                owner: "preview_owner".to_owned(),
                source_json: stateful_preview_document(260, 2, false, true, "renamed"),
            });
        app.update();
        let rejected = reload_report(&app, 51);
        assert_eq!(rejected.status, UiDocumentReloadStatus::Committed);
        for (node_id, state, reason) in [
            ("state.input", "text_input", "max_chars_tightened"),
            ("state.segmented", "segmented", "option_missing_or_disabled"),
            ("state.checkbox", "checkbox", "node_kind_changed"),
            ("state.tab_details", "tab", "tab_value_changed"),
        ] {
            let decision = state_decision_for(&rejected, node_id, state);
            assert!(!decision.preserved);
            assert_eq!(decision.reason, reason);
        }
    }

    #[test]
    fn ui_document_preview_switches_declared_page_state_through_the_runtime_reload_path() {
        let mut app =
            stateful_preview_app(stateful_preview_document(200, 32, true, false, "details"));
        let document_id = UiDocumentId::from_str("preview.runtime").unwrap();
        let old_instance = app
            .world()
            .resource::<UiDocumentRuntime>()
            .active_instance("preview_owner", &document_id)
            .unwrap();
        let old_root = app
            .world()
            .resource::<UiDocumentRuntime>()
            .node_entity(old_instance, &UiNodeId::from_str("state.root").unwrap())
            .unwrap();
        let old_scroll = active_node(&app, "preview_owner", "state.scroll");
        app.world_mut()
            .get_mut::<ScrollPosition>(old_scroll)
            .unwrap()
            .0 = Vec2::new(0.0, 24.0);

        app.world_mut()
            .write_message(UiDocumentPreviewCommand::SetPageState {
                reload_id: UiDocumentReloadId(70),
                document_id: document_id.clone(),
                owner: "preview_owner".to_owned(),
                page_state: UiPageState::loading(),
            });
        app.update();

        let report = reload_report(&app, 70);
        assert_eq!(report.status, UiDocumentReloadStatus::Committed);
        let new_instance = app
            .world()
            .resource::<UiDocumentRuntime>()
            .active_instance("preview_owner", &document_id)
            .unwrap();
        assert_ne!(new_instance, old_instance);
        assert!(app.world().get_entity(old_root).is_err());
        assert!(state_decision_for(&report, "state.scroll", "scroll").preserved);
        assert_eq!(
            app.world()
                .get::<ScrollPosition>(active_node(&app, "preview_owner", "state.scroll"))
                .unwrap()
                .0,
            Vec2::new(0.0, 24.0)
        );
        assert_eq!(
            app.world()
                .resource::<UiDocumentPreviewRegistry>()
                .registrations
                .get(&PreviewKey {
                    document_id,
                    owner: "preview_owner".to_owned(),
                })
                .unwrap()
                .registration
                .page_state,
            UiPageState::loading()
        );
    }

    #[test]
    fn ui_document_modal_panel_blocks_input_and_owner_cleanup_removes_the_modal_root() {
        let source = modal_preview_document();
        let parsed = document(&source);
        assert!(matches!(parsed.root, UiNode::Modal { .. }));

        let mut app = modal_preview_app();
        let document_id = UiDocumentId::from_str("preview.runtime").unwrap();
        let instance = app
            .world()
            .resource::<UiDocumentRuntime>()
            .active_instance("preview_owner", &document_id)
            .unwrap();
        let root = app
            .world()
            .resource::<UiDocumentRuntime>()
            .instance(instance)
            .unwrap()
            .root;
        assert_eq!(
            app.world().get::<UiPanelRoot>(root).unwrap().kind,
            UiPanelKind::Modal
        );
        assert!(app.world().resource::<UiInputState>().pointer_blocked);
        assert!(
            app.world()
                .resource::<UiInputState>()
                .top_blocking_panel
                .is_some()
        );

        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::CloseAllForOwner {
                owner: "preview_owner".to_owned(),
            });
        app.update();
        app.update();

        assert!(
            app.world()
                .resource::<UiDocumentRuntime>()
                .active_instance("preview_owner", &document_id)
                .is_none()
        );
        assert!(app.world().get_entity(root).is_err());
        assert!(!app.world().resource::<UiInputState>().pointer_blocked);
    }

    #[test]
    fn ui_document_reload_is_transactional_and_preserves_compatible_slider_state() {
        let mut app = App::new();
        app.add_plugins((
            super::super::UiDocumentRuntimePlugin,
            UiDocumentPreviewPlugin,
        ));
        app.world_mut()
            .write_message(UiDocumentPreviewCommand::Register(preview_registration(
                preview_document(None),
            )));
        app.update();

        let document_id = UiDocumentId::from_str("preview.runtime").unwrap();
        let audit_registry = app.world().resource::<UiDocumentAuditRecipeRegistry>();
        let audit_entry = audit_registry.entry(&document_id, "preview_owner").unwrap();
        assert_eq!(audit_entry.screen, "document_preview_runtime");
        assert!(audit_entry.profiles.contains(&"phone-small".to_owned()));
        let old_instance = app
            .world()
            .resource::<UiDocumentRuntime>()
            .active_instance("preview_owner", &document_id)
            .unwrap();
        let slider_id = UiNodeId::from_str("preview.slider").unwrap();
        let old_entity = app
            .world()
            .resource::<UiDocumentRuntime>()
            .node_entity(old_instance, &slider_id)
            .unwrap();
        app.world_mut()
            .get_mut::<UiSlider>(old_entity)
            .unwrap()
            .value = 0.8;

        app.world_mut()
            .write_message(UiDocumentPreviewCommand::ReloadSource {
                reload_id: UiDocumentReloadId(40),
                document_id: document_id.clone(),
                owner: "preview_owner".to_owned(),
                source_json: preview_document(Some(240)),
            });
        app.update();

        let runtime = app.world().resource::<UiDocumentRuntime>();
        let new_instance = runtime
            .active_instance("preview_owner", &document_id)
            .unwrap();
        assert_ne!(old_instance, new_instance);
        let new_entity = runtime.node_entity(new_instance, &slider_id).unwrap();

        let messages = app.world().resource::<Messages<UiDocumentReloadEvent>>();
        let mut cursor = MessageCursor::default();
        let reports = cursor
            .read(messages)
            .map(|event| event.0.clone())
            .collect::<Vec<_>>();
        let committed = reports
            .iter()
            .find(|report| {
                report.reload_id == UiDocumentReloadId(40)
                    && report.status == UiDocumentReloadStatus::Committed
            })
            .unwrap();
        assert!(
            committed.state_decisions.iter().any(|decision| {
                decision.node_id == slider_id && decision.state == "slider" && decision.preserved
            }),
            "committed reload did not preserve slider: {committed:#?}"
        );
        assert_eq!(app.world().get::<UiSlider>(new_entity).unwrap().value, 0.8);
        assert_eq!(
            committed.diff.as_ref().unwrap().kind,
            UiDocumentDiffKind::InPlace
        );

        app.world_mut()
            .write_message(UiDocumentPreviewCommand::ReloadSource {
                reload_id: UiDocumentReloadId(41),
                document_id: document_id.clone(),
                owner: "preview_owner".to_owned(),
                source_json: "{not json".to_owned(),
            });
        app.update();
        assert_eq!(
            app.world()
                .resource::<UiDocumentRuntime>()
                .active_instance("preview_owner", &document_id),
            Some(new_instance)
        );

        let messages = app.world().resource::<Messages<UiDocumentReloadEvent>>();
        let mut cursor = MessageCursor::default();
        let failed = cursor.read(messages).map(|event| &event.0).find(|report| {
            report.reload_id == UiDocumentReloadId(41)
                && report.status == UiDocumentReloadStatus::Failed
        });
        assert_eq!(
            failed
                .and_then(|report| report.error.as_ref())
                .map(|error| error.stage),
            Some(UiDocumentReloadStage::Validation)
        );

        app.world_mut()
            .write_message(UiDocumentPreviewCommand::ReloadSource {
                reload_id: UiDocumentReloadId(42),
                document_id: document_id.clone(),
                owner: "preview_owner".to_owned(),
                source_json: host_invalid_document(),
            });
        app.update();
        assert_eq!(
            app.world()
                .resource::<UiDocumentRuntime>()
                .active_instance("preview_owner", &document_id),
            Some(new_instance)
        );
        let messages = app.world().resource::<Messages<UiDocumentReloadEvent>>();
        let mut cursor = MessageCursor::default();
        let host_failed = cursor.read(messages).map(|event| &event.0).find(|report| {
            report.reload_id == UiDocumentReloadId(42)
                && report.status == UiDocumentReloadStatus::Failed
        });
        assert_eq!(
            host_failed
                .and_then(|report| report.error.as_ref())
                .map(|error| error.stage),
            Some(UiDocumentReloadStage::HostValidation)
        );
    }

    #[test]
    fn ui_document_resource_preflight_failure_keeps_previous_instance_and_stable_report() {
        let mut app = stateful_preview_app(preview_document(None));
        let document_id = UiDocumentId::from_str("preview.runtime").unwrap();
        let old_instance = app
            .world()
            .resource::<UiDocumentRuntime>()
            .active_instance("preview_owner", &document_id)
            .unwrap();
        let asset_id = super::super::UiAssetId::from_str("preview_image").unwrap();
        app.world_mut()
            .resource_mut::<super::super::UiDocumentAssetPreflightOverrides>()
            .set(
                document_id.clone(),
                asset_id,
                super::super::UiDocumentAssetPreflightStatus::Failed {
                    code: "UI_DOCUMENT_TEST_RESOURCE_FAILED".to_owned(),
                },
            );
        app.world_mut()
            .write_message(UiDocumentPreviewCommand::ReloadSource {
                reload_id: UiDocumentReloadId(60),
                document_id: document_id.clone(),
                owner: "preview_owner".to_owned(),
                source_json: resource_preview_document(),
            });
        app.update();

        assert_eq!(
            app.world()
                .resource::<UiDocumentRuntime>()
                .active_instance("preview_owner", &document_id),
            Some(old_instance)
        );
        let report = reload_report(&app, 60);
        assert_eq!(report.status, UiDocumentReloadStatus::Failed);
        assert_eq!(
            report.error.as_ref().map(|error| error.stage),
            Some(UiDocumentReloadStage::ResourcePreflight)
        );
        assert_eq!(
            report.error.as_ref().map(|error| error.code.as_str()),
            Some("UI_DOCUMENT_TEST_RESOURCE_FAILED")
        );
        assert_eq!(
            report.diff.as_ref().map(|diff| diff.kind),
            Some(UiDocumentDiffKind::RebuildPage)
        );
    }

    #[test]
    fn ui_document_cancelled_reload_keeps_old_state_and_does_not_leak_snapshot() {
        let mut app = stateful_preview_app(preview_document(None));
        let document_id = UiDocumentId::from_str("preview.runtime").unwrap();
        let slider_id = UiNodeId::from_str("preview.slider").unwrap();
        let old_instance = app
            .world()
            .resource::<UiDocumentRuntime>()
            .active_instance("preview_owner", &document_id)
            .unwrap();
        let old_slider = app
            .world()
            .resource::<UiDocumentRuntime>()
            .node_entity(old_instance, &slider_id)
            .unwrap();
        app.world_mut()
            .get_mut::<UiSlider>(old_slider)
            .unwrap()
            .value = 0.8;

        let asset_id = super::super::UiAssetId::from_str("preview_image").unwrap();
        app.world_mut()
            .resource_mut::<super::super::UiDocumentAssetPreflightOverrides>()
            .set(
                document_id.clone(),
                asset_id,
                super::super::UiDocumentAssetPreflightStatus::Pending,
            );
        app.world_mut()
            .write_message(UiDocumentPreviewCommand::ReloadSource {
                reload_id: UiDocumentReloadId(61),
                document_id: document_id.clone(),
                owner: "preview_owner".to_owned(),
                source_json: resource_preview_document(),
            });
        app.update();
        let request_id = app
            .world()
            .resource::<UiDocumentPreviewRegistry>()
            .pending
            .values()
            .find(|pending| pending.reload_id == UiDocumentReloadId(61))
            .unwrap()
            .request_id;
        assert_eq!(
            app.world()
                .resource::<UiDocumentRuntime>()
                .active_instance("preview_owner", &document_id),
            Some(old_instance)
        );

        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Cancel { request_id });
        app.update();
        let cancelled = reload_report(&app, 61);
        assert_eq!(cancelled.status, UiDocumentReloadStatus::Cancelled);
        assert_eq!(
            cancelled.error.as_ref().map(|error| error.stage),
            Some(UiDocumentReloadStage::Cancel)
        );
        assert_eq!(
            cancelled.error.as_ref().map(|error| error.code.as_str()),
            Some("UI_DOCUMENT_BUILD_CANCELLED")
        );
        assert_eq!(
            cancelled.diff.as_ref().map(|diff| diff.kind),
            Some(UiDocumentDiffKind::RebuildPage)
        );
        assert_eq!(
            app.world()
                .resource::<UiDocumentRuntime>()
                .active_instance("preview_owner", &document_id),
            Some(old_instance)
        );

        app.world_mut()
            .get_mut::<UiSlider>(old_slider)
            .unwrap()
            .value = 0.63;
        app.world_mut()
            .write_message(UiDocumentPreviewCommand::ReloadSource {
                reload_id: UiDocumentReloadId(62),
                document_id: document_id.clone(),
                owner: "preview_owner".to_owned(),
                source_json: preview_document(Some(280)),
            });
        app.update();
        let new_instance = app
            .world()
            .resource::<UiDocumentRuntime>()
            .active_instance("preview_owner", &document_id)
            .unwrap();
        assert_ne!(new_instance, old_instance);
        let new_slider = app
            .world()
            .resource::<UiDocumentRuntime>()
            .node_entity(new_instance, &slider_id)
            .unwrap();
        assert_eq!(app.world().get::<UiSlider>(new_slider).unwrap().value, 0.63);
        assert!(state_decision_for(&reload_report(&app, 62), "preview.slider", "slider").preserved);
    }
}
