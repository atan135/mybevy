use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

use bevy::{
    asset::LoadState,
    prelude::*,
    text::{FontWeight, LineHeight},
};
use serde::Serialize;

use super::{
    UiActionDispatch, UiActionDispatchContext, UiActionId, UiActionRegistry, UiActionRejected,
    UiActionValue, UiAssetEntry, UiAssetId, UiAssetKind, UiAssetSource, UiBindingPath,
    UiBindingScope, UiBindingType, UiColor, UiComponentSize, UiComponentState, UiComponentVariant,
    UiControlSlot, UiControlSlotContent, UiDocument, UiDocumentHostValidationContext, UiDocumentId,
    UiDocumentMarker, UiHostBindingKey, UiImageContentState, UiImageFailurePresentation,
    UiImagePresentation, UiNode, UiNodeId, UiNodeMarker, UiPageState, UiRegisteredActionKind,
    UiResolvedBackground, UiResolvedImageFallback, UiResolvedMaterialParameters, UiResolvedStyle,
    UiTargetProfile, UiTextContent, UiTextFormat, UiTextLineHeight, UiTextOverflow,
    UiTextTypography, UiTooltipToneSpec, UiWidgetControlAdapter, UiWidgetVariantAdapter,
    ValidatedUiDocument, resolve_image_fallback,
};
use crate::framework::ui::{
    core::{
        UiFocusSystems, UiLayer, UiLayerRoot, UiPanelId, UiPanelKind, UiPanelRoot,
        binding::UiBindingValues,
    },
    i18n::{UiI18n, UiI18nSystems, UiI18nText},
    style::{
        UiFontAssets, UiFontWeight, UiTextLineHeight as FrameworkTextLineHeight, UiTheme,
        theme::ButtonColors, try_ui_styled_text,
    },
    widgets::{
        DisabledButton, FocusableButton, FocusedButton, LoadingButton, ReadonlyTextInput,
        SelectedButton, UiAdvancedImageSource, UiAdvancedImageSpec, UiBadge, UiButtonEvent,
        UiButtonEventKind, UiControlFlags, UiControlId, UiControlKind, UiControlMeta,
        UiControlState, UiDropdown, UiDropdownOption, UiImagePixelSize, UiImageSize,
        UiImageTextureSource, UiProgress, UiScrollViewConfig, UiSlider, UiStepper, UiTextInput,
        UiTextInputMaxChars, UiTextInputValue, UiTooltip, UiTooltipTone,
        controls::{
            SelectionVisualState, UiBadgeLabel, UiButtonStyleLabel, UiDropdownLabel,
            UiNumericControlLabel, UiProgressLabel, UiSegmentOption, UiSegmentOptionSelected,
            UiSegmentedControl, UiSelectionText, UiStepperDecrementButton,
            UiStepperIncrementButton, UiTab, UiTabLabel, UiTextInputPlaceholder, UiTextInputText,
            UiTextInputTextPart, badge, progress, progress_display_text, resolve_control_state,
            segment_option_key_bundle, slider_bundle, stepper_bundle, tab,
        },
        dropdown_key, primary_action_button, secondary_action_button, segmented_control, tab_list,
        text_input, try_ui_advanced_image_from_handle, ui_image, ui_scroll_column_bundle,
    },
};

pub const UI_DOCUMENT_RUNTIME_RECORD_LIMIT: usize = 256;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct UiDocumentRequestId(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct UiDocumentInstanceId(pub u64);

#[derive(Clone, Debug)]
pub enum UiDocumentOpenSource {
    Json(String),
    Validated(ValidatedUiDocument),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiDocumentSourceOrigin {
    Fixture { fixture_id: String },
    Packaged { asset_id: UiAssetId },
    Runtime { producer: String },
    Preview { source_path: String },
}

impl UiDocumentSourceOrigin {
    pub fn audit_source_path(&self) -> String {
        match self {
            Self::Fixture { fixture_id } => safe_origin_label("fixture", fixture_id),
            Self::Packaged { asset_id } => format!("packaged:{}", asset_id.as_str()),
            Self::Runtime { producer } => safe_origin_label("runtime", producer),
            Self::Preview { source_path } if safe_preview_source_path(source_path) => {
                source_path.clone()
            }
            Self::Preview { .. } => "preview".to_owned(),
        }
    }
}

fn safe_preview_source_path(value: &str) -> bool {
    let allowed_root = value.starts_with("ui/documents/approved/")
        || value.starts_with("ui/documents/fixtures/")
        || value.starts_with("ui-documents/source/");
    allowed_root
        && value.len() <= 280
        && value.is_ascii()
        && value.ends_with(".json")
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

fn safe_origin_label(kind: &str, value: &str) -> String {
    if !value.is_empty()
        && value.len() <= 240
        && value.is_ascii()
        && !value.starts_with('/')
        && !value.contains(['\\', ':', '\0', '\n', '\r'])
        && !value.split('/').any(|segment| segment == "..")
    {
        format!("{kind}:{value}")
    } else {
        kind.to_owned()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiDocumentPanel {
    Page,
    Hud,
    Floating,
    Modal,
    BlockingOverlay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiDocumentLayer {
    Page,
    Floating,
    Modal,
    Loading,
    Toast,
    Debug,
}

impl UiDocumentLayer {
    fn to_framework(self) -> UiLayer {
        match self {
            Self::Page => UiLayer::Page,
            Self::Floating => UiLayer::Floating,
            Self::Modal => UiLayer::Modal,
            Self::Loading => UiLayer::Loading,
            Self::Toast => UiLayer::Toast,
            Self::Debug => UiLayer::Debug,
        }
    }
}

impl UiDocumentPanel {
    fn framework_root(self) -> UiPanelRoot {
        let (id, kind) = match self {
            Self::Page => (UiPanelId::new("document_page"), UiPanelKind::Page),
            Self::Hud => (UiPanelId::new("document_hud"), UiPanelKind::Hud),
            Self::Floating => (UiPanelId::new("document_floating"), UiPanelKind::Floating),
            Self::Modal => (UiPanelId::new("document_modal"), UiPanelKind::Modal),
            Self::BlockingOverlay => (
                UiPanelId::new("document_blocking_overlay"),
                UiPanelKind::BlockingOverlay,
            ),
        };
        UiPanelRoot {
            id,
            kind,
            // UiPanelRoot uses static owner IDs; the dynamic owner remains authoritative on
            // UiDocumentRuntimeRoot and is handled by runtime close/switch commands.
            owner: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct UiDocumentOpenRequest {
    pub request_id: UiDocumentRequestId,
    pub document_id: UiDocumentId,
    pub owner: String,
    pub source: UiDocumentOpenSource,
    pub origin: UiDocumentSourceOrigin,
    pub panel: UiDocumentPanel,
    pub layer: UiDocumentLayer,
    pub target_profile: UiTargetProfile,
    pub page_state: UiPageState,
    pub owner_alive: bool,
    pub host_bindings: BTreeMap<UiHostBindingKey, UiBindingType>,
}

#[derive(Clone, Debug, Message)]
pub enum UiDocumentRuntimeCommand {
    Open(UiDocumentOpenRequest),
    Cancel {
        request_id: UiDocumentRequestId,
    },
    Close {
        owner: String,
        document_id: UiDocumentId,
    },
    ClosePanel {
        owner: String,
        panel: UiDocumentPanel,
    },
    CloseAllForOwner {
        owner: String,
    },
    SwitchOwner {
        previous_owner: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiDocumentBuildState {
    Queued,
    Validating,
    Preflighting,
    Ready,
    Committed,
    Failed,
    Cancelled,
    Cleaned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiDocumentFailureStage {
    StaticValidation,
    HostValidation,
    ResourcePreflight,
    Commit,
    Cancel,
    Cleanup,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UiDocumentBuildRecord {
    pub request_id: UiDocumentRequestId,
    pub instance_id: Option<UiDocumentInstanceId>,
    pub document_id: UiDocumentId,
    pub owner: String,
    pub generation: u64,
    pub state: UiDocumentBuildState,
    pub elapsed_micros: u64,
    pub protocol_node_count: usize,
    pub ecs_entity_count: usize,
    pub asset_count: usize,
    pub failure_stage: Option<UiDocumentFailureStage>,
    pub failure_code: Option<String>,
}

#[derive(Clone, Debug, Message, PartialEq)]
pub struct UiDocumentRuntimeEvent(pub UiDocumentBuildRecord);

#[derive(Clone, Debug, Component, PartialEq)]
pub struct UiDocumentRuntimeRoot {
    pub request_id: UiDocumentRequestId,
    pub instance_id: UiDocumentInstanceId,
    pub generation: u64,
    pub document_id: UiDocumentId,
    pub schema_version: u32,
    pub owner: String,
    pub panel: UiDocumentPanel,
    pub layer: UiDocumentLayer,
    pub origin: UiDocumentSourceOrigin,
}

#[derive(Clone, Debug, Component, Eq, PartialEq)]
pub struct UiDocumentNodeMarker {
    pub instance_id: UiDocumentInstanceId,
    pub node_id: UiNodeId,
}

#[derive(Clone, Debug, Component, Eq, PartialEq, Serialize)]
pub struct UiDocumentNodeAuditMarker {
    pub instance_id: UiDocumentInstanceId,
    pub document_id: UiDocumentId,
    pub schema_version: u32,
    pub node_id: UiNodeId,
    pub document_path: String,
    pub source_path: String,
}

#[derive(Clone, Debug, Component, PartialEq)]
pub struct UiDocumentResolvedStyleMarker(pub UiResolvedStyle);

#[derive(Clone, Debug, Component, Eq, PartialEq)]
pub struct UiDocumentActionMarker {
    pub instance_id: UiDocumentInstanceId,
    pub node_id: UiNodeId,
    pub action_id: UiActionId,
}

#[derive(Clone, Debug)]
pub struct UiDocumentInstanceIndex {
    pub instance_id: UiDocumentInstanceId,
    pub request_id: UiDocumentRequestId,
    pub generation: u64,
    pub document_id: UiDocumentId,
    pub owner: String,
    pub panel: UiDocumentPanel,
    pub layer: UiDocumentLayer,
    pub root: Entity,
    pub nodes: BTreeMap<UiNodeId, Entity>,
    pub ecs_entity_count: usize,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DocumentKey {
    owner: String,
    document_id: UiDocumentId,
}

#[derive(Clone, Debug, Resource)]
pub struct UiDocumentRuntime {
    next_instance: u64,
    sequence: u64,
    generations: BTreeMap<DocumentKey, u64>,
    latest_generation: BTreeMap<DocumentKey, u64>,
    pending: BTreeMap<UiDocumentRequestId, PendingBuild>,
    active: BTreeMap<DocumentKey, UiDocumentInstanceId>,
    instances: BTreeMap<UiDocumentInstanceId, ActiveDocument>,
    records: BTreeMap<UiDocumentRequestId, (u64, UiDocumentBuildRecord)>,
}

impl Default for UiDocumentRuntime {
    fn default() -> Self {
        Self {
            next_instance: 1,
            sequence: 0,
            generations: default(),
            latest_generation: default(),
            pending: default(),
            active: default(),
            instances: default(),
            records: default(),
        }
    }
}

impl UiDocumentRuntime {
    pub fn record(&self, request_id: UiDocumentRequestId) -> Option<&UiDocumentBuildRecord> {
        self.records.get(&request_id).map(|(_, record)| record)
    }

    pub fn active_instance(
        &self,
        owner: &str,
        document_id: &UiDocumentId,
    ) -> Option<UiDocumentInstanceId> {
        self.active
            .get(&DocumentKey {
                owner: owner.to_owned(),
                document_id: document_id.clone(),
            })
            .copied()
    }

    pub fn instance(&self, instance_id: UiDocumentInstanceId) -> Option<&UiDocumentInstanceIndex> {
        self.instances.get(&instance_id).map(|active| &active.index)
    }

    pub fn node_entity(
        &self,
        instance_id: UiDocumentInstanceId,
        node_id: &UiNodeId,
    ) -> Option<Entity> {
        self.instance(instance_id)?.nodes.get(node_id).copied()
    }

    pub fn active_validated_document(
        &self,
        owner: &str,
        document_id: &UiDocumentId,
    ) -> Option<&ValidatedUiDocument> {
        let instance_id = self.active_instance(owner, document_id)?;
        self.active_validated_document_by_instance(instance_id)
    }

    pub fn active_validated_document_by_instance(
        &self,
        instance_id: UiDocumentInstanceId,
    ) -> Option<&ValidatedUiDocument> {
        self.instances
            .get(&instance_id)
            .map(|active| &active.validated)
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    fn next_generation(&mut self, key: &DocumentKey) -> u64 {
        let generation = self.generations.entry(key.clone()).or_default();
        *generation += 1;
        self.latest_generation.insert(key.clone(), *generation);
        *generation
    }

    fn next_instance_id(&mut self) -> UiDocumentInstanceId {
        let result = UiDocumentInstanceId(self.next_instance);
        self.next_instance += 1;
        result
    }

    fn store_record(&mut self, record: UiDocumentBuildRecord) {
        self.sequence += 1;
        self.records
            .insert(record.request_id, (self.sequence, record));
        while self.records.len() > UI_DOCUMENT_RUNTIME_RECORD_LIMIT {
            let Some(oldest) = self
                .records
                .iter()
                .min_by_key(|(_, (sequence, _))| *sequence)
                .map(|(request_id, _)| *request_id)
            else {
                break;
            };
            self.records.remove(&oldest);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiDocumentAssetPreflightStatus {
    Pending,
    Ready { asset: UiDocumentResolvedAsset },
    Failed { code: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiDocumentResolvedAsset {
    Image(Handle<Image>),
    Font(Handle<Font>),
    BuiltInMaterial,
}

impl UiDocumentResolvedAsset {
    fn matches_kind(&self, kind: UiAssetKind) -> bool {
        matches!(
            (self, kind),
            (
                Self::Image(_),
                UiAssetKind::Image | UiAssetKind::Icon | UiAssetKind::Atlas
            ) | (Self::Font(_), UiAssetKind::Font)
                | (Self::BuiltInMaterial, UiAssetKind::Material)
        )
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct AssetOverrideKey {
    document_id: UiDocumentId,
    asset_id: UiAssetId,
}

#[derive(Clone, Debug, Default, Resource)]
pub struct UiDocumentAssetPreflightOverrides {
    statuses: BTreeMap<AssetOverrideKey, UiDocumentAssetPreflightStatus>,
}

impl UiDocumentAssetPreflightOverrides {
    pub fn set(
        &mut self,
        document_id: UiDocumentId,
        asset_id: UiAssetId,
        status: UiDocumentAssetPreflightStatus,
    ) {
        self.statuses.insert(
            AssetOverrideKey {
                document_id,
                asset_id,
            },
            status,
        );
    }

    pub fn remove(&mut self, document_id: &UiDocumentId, asset_id: &UiAssetId) {
        self.statuses.remove(&AssetOverrideKey {
            document_id: document_id.clone(),
            asset_id: asset_id.clone(),
        });
    }

    fn get(
        &self,
        document_id: &UiDocumentId,
        asset_id: &UiAssetId,
    ) -> Option<&UiDocumentAssetPreflightStatus> {
        self.statuses.get(&AssetOverrideKey {
            document_id: document_id.clone(),
            asset_id: asset_id.clone(),
        })
    }
}

#[derive(Clone, Debug)]
struct PendingAsset {
    entry: UiAssetEntry,
    handle: Option<UiDocumentResolvedAsset>,
    ready: bool,
    failed: bool,
    commit_required: bool,
    actual_decoded_bytes: Option<u64>,
}

#[derive(Clone, Debug)]
struct PreparedNode {
    source: UiNode,
    path: String,
    layout: super::UiBevyLayout,
    style: UiResolvedStyle,
    control: Option<UiWidgetControlAdapter>,
    control_styles: Option<PreparedControlStyles>,
    children: Vec<PreparedNode>,
}

#[derive(Clone, Debug)]
struct PreparedControlStyles {
    base: UiResolvedStyle,
    states: BTreeMap<UiComponentState, UiResolvedStyle>,
}

#[derive(Clone, Debug)]
struct PreparedDocument {
    validated: ValidatedUiDocument,
    root: PreparedNode,
    fingerprint: String,
    node_count: usize,
}

#[derive(Clone, Debug)]
struct PendingBuild {
    request: UiDocumentOpenRequest,
    key: DocumentKey,
    generation: u64,
    started: Instant,
    prepared: PreparedDocument,
    assets: BTreeMap<UiAssetId, PendingAsset>,
    ready: bool,
}

#[derive(Clone, Debug)]
struct ActiveDocument {
    index: UiDocumentInstanceIndex,
    validated: ValidatedUiDocument,
    fingerprint: String,
    origin: UiDocumentSourceOrigin,
    asset_decoded_bytes: BTreeMap<UiAssetId, u64>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub enum UiDocumentRuntimeSystems {
    Commands,
    Preflight,
    Commit,
    Reconcile,
}

pub struct UiDocumentRuntimePlugin;

impl Plugin for UiDocumentRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiDocumentRuntime>()
            .init_resource::<UiDocumentAssetPreflightOverrides>()
            .init_resource::<UiActionRegistry>()
            .init_resource::<UiBindingValues>()
            .add_message::<UiDocumentRuntimeCommand>()
            .add_message::<UiDocumentRuntimeEvent>()
            .add_message::<UiButtonEvent>()
            .add_message::<UiActionDispatch>()
            .add_message::<UiActionRejected>()
            .configure_sets(
                Update,
                (
                    UiDocumentRuntimeSystems::Commands,
                    UiDocumentRuntimeSystems::Preflight,
                    UiDocumentRuntimeSystems::Commit,
                    UiDocumentRuntimeSystems::Reconcile,
                )
                    .chain(),
            )
            .configure_sets(
                Update,
                UiDocumentRuntimeSystems::Reconcile.after(UiFocusSystems::Visuals),
            )
            .add_systems(
                Update,
                handle_runtime_commands.in_set(UiDocumentRuntimeSystems::Commands),
            )
            .add_systems(
                Update,
                poll_runtime_assets.in_set(UiDocumentRuntimeSystems::Preflight),
            )
            .add_systems(
                Update,
                commit_ready_documents.in_set(UiDocumentRuntimeSystems::Commit),
            )
            .add_systems(
                Update,
                (
                    reconcile_runtime_documents,
                    dispatch_document_actions,
                    update_document_bound_texts,
                    update_document_control_text_sources,
                    enforce_document_control_presentations,
                    update_document_runtime_images,
                    enforce_document_generated_text_styles,
                    enforce_document_text_constraints,
                )
                    .chain()
                    .after(UiI18nSystems::Refresh)
                    .in_set(UiDocumentRuntimeSystems::Reconcile),
            );
    }
}

fn handle_runtime_commands(
    mut commands: Commands,
    mut incoming: MessageReader<UiDocumentRuntimeCommand>,
    registry: Res<UiActionRegistry>,
    i18n: Option<Res<UiI18n>>,
    mut runtime: ResMut<UiDocumentRuntime>,
    mut events: MessageWriter<UiDocumentRuntimeEvent>,
    mut binding_values: Option<ResMut<UiBindingValues>>,
) {
    for command in incoming.read().cloned() {
        match command {
            UiDocumentRuntimeCommand::Open(request) => open_document(
                request,
                &registry,
                i18n.as_deref(),
                &mut runtime,
                &mut events,
            ),
            UiDocumentRuntimeCommand::Cancel { request_id } => {
                cancel_pending(
                    request_id,
                    "UI_DOCUMENT_BUILD_CANCELLED",
                    &mut runtime,
                    &mut events,
                );
            }
            UiDocumentRuntimeCommand::Close { owner, document_id } => close_key(
                DocumentKey { owner, document_id },
                &mut commands,
                &mut runtime,
                &mut events,
                binding_values.as_deref_mut(),
            ),
            UiDocumentRuntimeCommand::ClosePanel { owner, panel } => close_panel(
                &owner,
                panel,
                &mut commands,
                &mut runtime,
                &mut events,
                binding_values.as_deref_mut(),
            ),
            UiDocumentRuntimeCommand::CloseAllForOwner { owner }
            | UiDocumentRuntimeCommand::SwitchOwner {
                previous_owner: owner,
            } => close_owner(
                &owner,
                &mut commands,
                &mut runtime,
                &mut events,
                binding_values.as_deref_mut(),
            ),
        }
    }
}

fn open_document(
    request: UiDocumentOpenRequest,
    registry: &UiActionRegistry,
    i18n: Option<&UiI18n>,
    runtime: &mut UiDocumentRuntime,
    events: &mut MessageWriter<UiDocumentRuntimeEvent>,
) {
    if runtime.records.contains_key(&request.request_id)
        || runtime.pending.contains_key(&request.request_id)
    {
        let record = base_record(&request, 0, UiDocumentBuildState::Failed, 0, 0).failed(
            UiDocumentFailureStage::StaticValidation,
            "UI_DOCUMENT_REQUEST_ID_DUPLICATE",
        );
        // The first request owns its stable record. Reuse still emits a diagnostic event,
        // but cannot overwrite the original lifecycle history under the same key.
        events.write(UiDocumentRuntimeEvent(record));
        return;
    }

    let key = DocumentKey {
        owner: request.owner.clone(),
        document_id: request.document_id.clone(),
    };
    let superseded = runtime
        .pending
        .iter()
        .filter(|(_, pending)| pending.key == key)
        .map(|(request_id, _)| *request_id)
        .collect::<Vec<_>>();
    for request_id in superseded {
        cancel_pending(request_id, "UI_DOCUMENT_BUILD_SUPERSEDED", runtime, events);
    }

    emit_record(
        runtime,
        events,
        base_record(&request, 0, UiDocumentBuildState::Queued, 0, 0),
    );
    emit_record(
        runtime,
        events,
        base_record(&request, 0, UiDocumentBuildState::Validating, 0, 0),
    );

    let validated = match &request.source {
        UiDocumentOpenSource::Json(source) => UiDocument::parse_and_validate_json(source),
        UiDocumentOpenSource::Validated(document) => Ok(document.clone()),
    };
    let validated = match validated {
        Ok(validated) => validated,
        Err(error) => {
            let record = base_record(&request, 0, UiDocumentBuildState::Failed, 0, 0)
                .failed(UiDocumentFailureStage::StaticValidation, error.code());
            emit_record(runtime, events, record);
            return;
        }
    };
    if validated.document().document_id != request.document_id {
        let record = base_record(&request, 0, UiDocumentBuildState::Failed, 0, 0).failed(
            UiDocumentFailureStage::StaticValidation,
            "UI_DOCUMENT_ID_MISMATCH",
        );
        emit_record(runtime, events, record);
        return;
    }

    let effective = match validated.effective_document(&request.target_profile, &request.page_state)
    {
        Ok(effective) => effective,
        Err(error) => {
            let record = base_record(&request, 0, UiDocumentBuildState::Failed, 0, 0)
                .failed(UiDocumentFailureStage::StaticValidation, error.code());
            emit_record(runtime, events, record);
            return;
        }
    };
    let effective = match ValidatedUiDocument::new(effective.document) {
        Ok(effective) => effective,
        Err(error) => {
            let record = base_record(&request, 0, UiDocumentBuildState::Failed, 0, 0)
                .failed(UiDocumentFailureStage::StaticValidation, error.code());
            emit_record(runtime, events, record);
            return;
        }
    };
    let host_errors = effective.validate_with_host(&UiDocumentHostValidationContext {
        owner: &request.owner,
        owner_alive: request.owner_alive,
        action_registry: registry,
        bindings: &request.host_bindings,
    });
    if let Some(error) = host_errors.first() {
        let record = base_record(
            &request,
            0,
            UiDocumentBuildState::Failed,
            count_nodes(&effective.document().root),
            effective.document().assets.len(),
        )
        .failed(UiDocumentFailureStage::HostValidation, error.code);
        emit_record(runtime, events, record);
        return;
    }
    if let Some(error) = i18n.and_then(|catalog| {
        effective
            .document()
            .validate_content_with_catalog(catalog)
            .into_iter()
            .next()
    }) {
        let record = base_record(
            &request,
            0,
            UiDocumentBuildState::Failed,
            count_nodes(&effective.document().root),
            effective.document().assets.len(),
        )
        .failed(UiDocumentFailureStage::HostValidation, error.code);
        emit_record(runtime, events, record);
        return;
    }

    let prepared = match prepare_document(effective) {
        Ok(prepared) => prepared,
        Err(code) => {
            let record = base_record(&request, 0, UiDocumentBuildState::Failed, 0, 0)
                .failed(UiDocumentFailureStage::StaticValidation, code);
            emit_record(runtime, events, record);
            return;
        }
    };
    if let Some(active_instance) = runtime.active.get(&key).copied()
        && let Some(active) = runtime.instances.get(&active_instance)
        && active.fingerprint == prepared.fingerprint
        && active.index.panel == request.panel
        && active.index.layer == request.layer
        && active.origin == request.origin
    {
        let mut record = base_record(
            &request,
            active.index.generation,
            UiDocumentBuildState::Committed,
            prepared.node_count,
            prepared.validated.document().assets.len(),
        );
        record.instance_id = Some(active_instance);
        record.ecs_entity_count = active.index.ecs_entity_count;
        emit_record(runtime, events, record);
        return;
    }

    let generation = runtime.next_generation(&key);
    let optional_image_assets = optional_image_primary_assets(&prepared.validated.document().root);
    let required_image_assets = required_image_assets(&prepared.validated.document().root);
    let assets = prepared
        .validated
        .document()
        .assets
        .iter()
        .map(|(id, entry)| {
            (
                id.clone(),
                PendingAsset {
                    entry: entry.clone(),
                    handle: None,
                    ready: false,
                    failed: false,
                    commit_required: !optional_image_assets.contains(id)
                        || required_image_assets.contains(id),
                    actual_decoded_bytes: None,
                },
            )
        })
        .collect();
    let node_count = prepared.node_count;
    let asset_count = prepared.validated.document().assets.len();
    emit_record(
        runtime,
        events,
        base_record(
            &request,
            generation,
            UiDocumentBuildState::Preflighting,
            node_count,
            asset_count,
        ),
    );
    runtime.pending.insert(
        request.request_id,
        PendingBuild {
            request,
            key,
            generation,
            started: Instant::now(),
            prepared,
            assets,
            ready: false,
        },
    );
}

fn prepare_document(validated: ValidatedUiDocument) -> Result<PreparedDocument, &'static str> {
    fn prepare_node(
        document: &UiDocument,
        validated: &ValidatedUiDocument,
        node: &UiNode,
    ) -> Result<PreparedNode, &'static str> {
        let path = validated
            .node_path(node.id())
            .ok_or("UI_DOCUMENT_NODE_INDEX_MISSING")?
            .to_owned();
        let layout = node
            .layout()
            .to_bevy_layout()
            .map_err(|_| "UI_DOCUMENT_LAYOUT_ADAPTER_FAILED")?;
        let base_style = document
            .resolve_style(node.style(), &format!("{path}.style"))
            .map_err(|_| "UI_DOCUMENT_STYLE_ADAPTER_FAILED")?;
        let control = node.widget_adapter();
        reject_unsupported_letter_spacing(&base_style)?;
        let control_styles = if let Some(component) = node.component() {
            let mut states = BTreeMap::new();
            for (state, state_override) in &component.state_overrides {
                let override_style = document
                    .resolve_style(
                        state_override,
                        &format!("{path}.component.state_overrides.{state}"),
                    )
                    .map_err(|_| "UI_DOCUMENT_CONTROL_STATE_STYLE_ADAPTER_FAILED")?;
                let mut resolved = base_style.clone();
                merge_resolved_style(&mut resolved, override_style);
                reject_unsupported_letter_spacing(&resolved)?;
                states.insert(*state, resolved);
            }
            Some(PreparedControlStyles {
                base: base_style.clone(),
                states,
            })
        } else {
            None
        };
        let style = control
            .and_then(|control| {
                control_styles.as_ref().and_then(|styles| {
                    styles
                        .states
                        .get(&component_state_from_widget(control.state))
                        .cloned()
                })
            })
            .unwrap_or_else(|| base_style.clone());
        let children = node
            .children()
            .iter()
            .map(|child| prepare_node(document, validated, child))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PreparedNode {
            source: node.clone(),
            path,
            layout,
            style,
            control,
            control_styles,
            children,
        })
    }

    let fingerprint = validated
        .document()
        .to_canonical_json()
        .map_err(|_| "UI_DOCUMENT_CANONICALIZATION_FAILED")?;
    let node_count = count_nodes(&validated.document().root);
    let root = prepare_node(validated.document(), &validated, &validated.document().root)?;
    Ok(PreparedDocument {
        validated,
        root,
        fingerprint,
        node_count,
    })
}

fn reject_unsupported_letter_spacing(style: &UiResolvedStyle) -> Result<(), &'static str> {
    if style
        .properties
        .text
        .as_ref()
        .and_then(|text| text.letter_spacing)
        .is_some_and(|spacing| spacing != 0.0)
    {
        Err("UI_DOCUMENT_TEXT_LETTER_SPACING_UNSUPPORTED")
    } else {
        Ok(())
    }
}

fn component_state_from_widget(state: UiControlState) -> UiComponentState {
    match state {
        UiControlState::Normal => UiComponentState::Normal,
        UiControlState::Hovered => UiComponentState::Hovered,
        UiControlState::Pressed => UiComponentState::Pressed,
        UiControlState::Focused => UiComponentState::Focused,
        UiControlState::Selected => UiComponentState::Selected,
        UiControlState::Disabled => UiComponentState::Disabled,
        UiControlState::Loading => UiComponentState::Loading,
        UiControlState::Empty => UiComponentState::Empty,
        UiControlState::Error => UiComponentState::Error,
    }
}

fn merge_resolved_style(base: &mut UiResolvedStyle, higher: UiResolvedStyle) {
    if higher.component.is_some() {
        base.component = higher.component;
    }
    if higher.role.is_some() {
        base.role = higher.role;
    }
    if higher.text_role.is_some() {
        base.text_role = higher.text_role;
    }
    let higher = higher.properties;
    if higher.background.is_some() {
        base.properties.background = higher.background;
    }
    if higher.border.is_some() {
        base.properties.border = higher.border;
    }
    if higher.corner_radius.is_some() {
        base.properties.corner_radius = higher.corner_radius;
    }
    if let Some(higher_text) = higher.text {
        if let Some(base_text) = &mut base.properties.text {
            if higher_text.color.is_some() {
                base_text.color = higher_text.color;
            }
            if higher_text.font.is_some() {
                base_text.font = higher_text.font;
            }
            if higher_text.font_size.is_some() {
                base_text.font_size = higher_text.font_size;
            }
            if higher_text.line_height.is_some() {
                base_text.line_height = higher_text.line_height;
            }
            if higher_text.letter_spacing.is_some() {
                base_text.letter_spacing = higher_text.letter_spacing;
            }
            if higher_text.weight.is_some() {
                base_text.weight = higher_text.weight;
            }
        } else {
            base.properties.text = Some(higher_text);
        }
    }
    if higher.opacity.is_some() {
        base.properties.opacity = higher.opacity;
    }
    if higher.shadows.is_some() {
        base.properties.shadows = higher.shadows;
    }
    if higher.material.is_some() {
        base.properties.material = higher.material;
    }
}

fn count_nodes(node: &UiNode) -> usize {
    1 + node.children().iter().map(count_nodes).sum::<usize>()
}

fn optional_image_primary_assets(node: &UiNode) -> BTreeSet<UiAssetId> {
    let mut assets = BTreeSet::new();
    collect_image_asset_roles(node, &mut assets, &mut BTreeSet::new());
    assets
}

fn required_image_assets(node: &UiNode) -> BTreeSet<UiAssetId> {
    let mut assets = BTreeSet::new();
    collect_image_asset_roles(node, &mut BTreeSet::new(), &mut assets);
    assets
}

fn collect_image_asset_roles(
    node: &UiNode,
    optional: &mut BTreeSet<UiAssetId>,
    required: &mut BTreeSet<UiAssetId>,
) {
    match node {
        UiNode::Image {
            asset,
            placeholder,
            failure,
            ..
        } => {
            optional.insert(asset.clone());
            if let Some(placeholder) = placeholder {
                required.insert(placeholder.clone());
            }
            if let UiImageFailurePresentation::Asset { asset } = failure {
                required.insert(asset.clone());
            }
        }
        UiNode::Icon { asset, .. } | UiNode::ImageButton { asset, .. } => {
            required.insert(asset.clone());
        }
        _ => {}
    }
    if let Some(component) = node.component() {
        for content in component.slots.values() {
            match content {
                UiControlSlotContent::Icon { asset, .. }
                | UiControlSlotContent::Image { asset, .. } => {
                    required.insert(asset.clone());
                }
                UiControlSlotContent::Text { .. } => {}
            }
        }
    }
    for child in node.children() {
        collect_image_asset_roles(child, optional, required);
    }
}

fn poll_runtime_assets(
    asset_server: Option<Res<AssetServer>>,
    images: Option<Res<Assets<Image>>>,
    overrides: Res<UiDocumentAssetPreflightOverrides>,
    mut runtime: ResMut<UiDocumentRuntime>,
    mut events: MessageWriter<UiDocumentRuntimeEvent>,
) {
    let request_ids = runtime.pending.keys().copied().collect::<Vec<_>>();
    for request_id in request_ids {
        let mut failure = None;
        let mut became_ready = false;
        {
            let Some(pending) = runtime.pending.get_mut(&request_id) else {
                continue;
            };
            if pending.ready {
                continue;
            }
            for (asset_id, asset) in &mut pending.assets {
                if asset.ready {
                    continue;
                }
                if let Some(status) = overrides.get(&pending.request.document_id, asset_id) {
                    match status {
                        UiDocumentAssetPreflightStatus::Pending => continue,
                        UiDocumentAssetPreflightStatus::Ready { asset: resolved } => {
                            if !resolved.matches_kind(asset.entry.kind) {
                                failure =
                                    Some("UI_DOCUMENT_PREFLIGHT_ASSET_KIND_MISMATCH".to_owned());
                                break;
                            }
                            let actual_bytes = match validate_resolved_asset_metadata(
                                &asset.entry,
                                resolved,
                                images.as_deref(),
                            ) {
                                Ok(actual_bytes) => actual_bytes,
                                Err(_code) if !asset.commit_required => {
                                    asset.failed = true;
                                    asset.handle = Some(resolved.clone());
                                    asset.actual_decoded_bytes = None;
                                    continue;
                                }
                                Err(code) => {
                                    failure = Some(code.to_owned());
                                    break;
                                }
                            };
                            asset.handle = Some(resolved.clone());
                            asset.ready = true;
                            asset.failed = false;
                            asset.actual_decoded_bytes = actual_bytes;
                            continue;
                        }
                        UiDocumentAssetPreflightStatus::Failed { code } => {
                            if asset.commit_required {
                                failure = Some(code.clone());
                                break;
                            }
                            asset.failed = true;
                            continue;
                        }
                    }
                }
                match &asset.entry.source {
                    UiAssetSource::BuiltInMaterial { .. } => {
                        asset.handle = Some(UiDocumentResolvedAsset::BuiltInMaterial);
                        asset.ready = true;
                    }
                    UiAssetSource::ContentCache { .. } => {
                        failure = Some("UI_DOCUMENT_CONTENT_CACHE_UNRESOLVED".to_owned());
                        break;
                    }
                    UiAssetSource::Packaged { path } => {
                        let Some(asset_server) = asset_server.as_deref() else {
                            failure = Some("UI_DOCUMENT_ASSET_SERVER_UNAVAILABLE".to_owned());
                            break;
                        };
                        if asset.handle.is_none() {
                            asset.handle = Some(match asset.entry.kind {
                                UiAssetKind::Image | UiAssetKind::Icon | UiAssetKind::Atlas => {
                                    UiDocumentResolvedAsset::Image(asset_server.load(path.clone()))
                                }
                                UiAssetKind::Font => {
                                    UiDocumentResolvedAsset::Font(asset_server.load(path.clone()))
                                }
                                UiAssetKind::Material => UiDocumentResolvedAsset::BuiltInMaterial,
                            });
                        }
                        let state = match asset.handle.as_ref() {
                            Some(UiDocumentResolvedAsset::Image(handle)) => {
                                asset_server.get_load_state(handle.id())
                            }
                            Some(UiDocumentResolvedAsset::Font(handle)) => {
                                asset_server.get_load_state(handle.id())
                            }
                            Some(UiDocumentResolvedAsset::BuiltInMaterial) => {
                                Some(LoadState::Loaded)
                            }
                            None => None,
                        };
                        match state {
                            Some(LoadState::Loaded) => {
                                let resolved = asset.handle.as_ref().expect("asset handle exists");
                                match validate_resolved_asset_metadata(
                                    &asset.entry,
                                    resolved,
                                    images.as_deref(),
                                ) {
                                    Ok(actual_bytes) => {
                                        asset.ready = true;
                                        asset.failed = false;
                                        asset.actual_decoded_bytes = actual_bytes;
                                    }
                                    Err(_) if !asset.commit_required => asset.failed = true,
                                    Err(code) => {
                                        failure = Some(code.to_owned());
                                        break;
                                    }
                                }
                            }
                            Some(LoadState::Failed(_)) => {
                                if asset.commit_required {
                                    failure = Some("UI_DOCUMENT_ASSET_LOAD_FAILED".to_owned());
                                    break;
                                }
                                asset.failed = true;
                            }
                            Some(LoadState::NotLoaded | LoadState::Loading) | None => {}
                        }
                    }
                }
            }
            let total_actual_bytes = pending
                .assets
                .values()
                .filter_map(|asset| asset.actual_decoded_bytes)
                .sum::<u64>();
            if total_actual_bytes > super::UI_ASSET_MAX_TOTAL_DECODED_BYTES {
                failure = Some("UI_DOCUMENT_ASSET_ACTUAL_TOTAL_BYTES_BUDGET_EXCEEDED".to_owned());
            }
            if failure.is_none()
                && pending
                    .assets
                    .values()
                    .filter(|asset| asset.commit_required)
                    .all(|asset| asset.ready)
            {
                pending.ready = true;
                became_ready = true;
            }
        }
        if let Some(code) = failure {
            let Some(pending) = runtime.pending.remove(&request_id) else {
                continue;
            };
            let record = record_for_pending(&pending, UiDocumentBuildState::Failed)
                .failed(UiDocumentFailureStage::ResourcePreflight, code);
            emit_record(&mut runtime, &mut events, record);
        } else if became_ready {
            let pending = runtime.pending.get(&request_id).expect("pending exists");
            let record = record_for_pending(pending, UiDocumentBuildState::Ready);
            emit_record(&mut runtime, &mut events, record);
        }
    }
}

fn validate_resolved_asset_metadata(
    entry: &UiAssetEntry,
    resolved: &UiDocumentResolvedAsset,
    images: Option<&Assets<Image>>,
) -> Result<Option<u64>, &'static str> {
    let UiDocumentResolvedAsset::Image(handle) = resolved else {
        return Ok(None);
    };
    let Some(image) = images.and_then(|images| images.get(handle)) else {
        return Err("UI_DOCUMENT_IMAGE_METADATA_UNAVAILABLE");
    };
    let width = image.width();
    let height = image.height();
    let decoded_bytes = image
        .data
        .as_ref()
        .map(|data| data.len() as u64)
        .ok_or("UI_DOCUMENT_IMAGE_METADATA_UNAVAILABLE")?;
    if width == 0 || height == 0 || decoded_bytes == 0 {
        return Err("UI_DOCUMENT_IMAGE_METADATA_INVALID");
    }
    if width > super::UI_ASSET_MAX_DIMENSION || height > super::UI_ASSET_MAX_DIMENSION {
        return Err("UI_DOCUMENT_ASSET_ACTUAL_DIMENSION_BUDGET_EXCEEDED");
    }
    if decoded_bytes > super::UI_ASSET_MAX_DECODED_BYTES {
        return Err("UI_DOCUMENT_ASSET_ACTUAL_BYTES_BUDGET_EXCEEDED");
    }
    if entry.declared_size.is_some_and(|declared| {
        declared.width != width
            || declared.height != height
            || declared.decoded_bytes != decoded_bytes
    }) {
        return Err("UI_DOCUMENT_ASSET_METADATA_MISMATCH");
    }
    Ok(Some(decoded_bytes))
}

fn commit_ready_documents(world: &mut World) {
    let request_ids = world
        .resource::<UiDocumentRuntime>()
        .pending
        .iter()
        .filter_map(|(request_id, pending)| pending.ready.then_some(*request_id))
        .collect::<Vec<_>>();
    for request_id in request_ids {
        let pending = world
            .resource_mut::<UiDocumentRuntime>()
            .pending
            .remove(&request_id);
        let Some(pending) = pending else {
            continue;
        };
        let current_generation = world
            .resource::<UiDocumentRuntime>()
            .latest_generation
            .get(&pending.key)
            .copied();
        if current_generation != Some(pending.generation) {
            let record = record_for_pending(&pending, UiDocumentBuildState::Cancelled).failed(
                UiDocumentFailureStage::Cancel,
                "UI_DOCUMENT_BUILD_STALE_GENERATION",
            );
            write_record(world, record);
            continue;
        }
        let instance_id = world.resource_mut::<UiDocumentRuntime>().next_instance_id();
        match spawn_document(world, &pending, instance_id) {
            Ok(mut active) => {
                let old_instance = world
                    .resource::<UiDocumentRuntime>()
                    .active
                    .get(&pending.key)
                    .copied();
                if let Some(old_instance) = old_instance {
                    cleanup_replaced_instance(world, old_instance);
                }
                world
                    .entity_mut(active.index.root)
                    .insert(Visibility::Visible);
                active.index.ecs_entity_count = count_entity_tree(world, active.index.root);
                let entity_count = active.index.ecs_entity_count;
                world
                    .resource_mut::<UiDocumentRuntime>()
                    .active
                    .insert(pending.key.clone(), instance_id);
                world
                    .resource_mut::<UiDocumentRuntime>()
                    .instances
                    .insert(instance_id, active);
                let mut record = record_for_pending(&pending, UiDocumentBuildState::Committed);
                record.instance_id = Some(instance_id);
                record.ecs_entity_count = entity_count;
                write_record(world, record);
                super::finish_committed_preview_reload(world, request_id);
            }
            Err(code) => {
                let record = record_for_pending(&pending, UiDocumentBuildState::Failed)
                    .failed(UiDocumentFailureStage::Commit, code);
                write_record(world, record);
            }
        }
    }
}

fn spawn_document(
    world: &mut World,
    pending: &PendingBuild,
    instance_id: UiDocumentInstanceId,
) -> Result<ActiveDocument, &'static str> {
    let mut nodes = BTreeMap::new();
    let root = match spawn_prepared_node(
        world,
        pending,
        &pending.prepared.root,
        instance_id,
        &mut nodes,
    ) {
        Ok(root) => root,
        Err(error) => {
            for entity in nodes.values().copied().collect::<Vec<_>>() {
                if world.get_entity(entity).is_ok() {
                    world.entity_mut(entity).despawn();
                }
            }
            return Err(error);
        }
    };
    world.entity_mut(root).insert((
        Visibility::Hidden,
        UiDocumentMarker {
            document_id: pending.request.document_id.clone(),
            schema_version: pending.prepared.validated.document().schema_version,
        },
        UiDocumentRuntimeRoot {
            request_id: pending.request.request_id,
            instance_id,
            generation: pending.generation,
            document_id: pending.request.document_id.clone(),
            schema_version: pending.prepared.validated.document().schema_version,
            owner: pending.request.owner.clone(),
            panel: pending.request.panel,
            layer: pending.request.layer,
            origin: pending.request.origin.clone(),
        },
        UiLayerRoot {
            layer: pending.request.layer.to_framework(),
        },
        pending.request.panel.framework_root(),
    ));
    Ok(ActiveDocument {
        index: UiDocumentInstanceIndex {
            instance_id,
            request_id: pending.request.request_id,
            generation: pending.generation,
            document_id: pending.request.document_id.clone(),
            owner: pending.request.owner.clone(),
            panel: pending.request.panel,
            layer: pending.request.layer,
            root,
            nodes,
            ecs_entity_count: 0,
        },
        validated: pending.prepared.validated.clone(),
        fingerprint: pending.prepared.fingerprint.clone(),
        origin: pending.request.origin.clone(),
        asset_decoded_bytes: pending
            .assets
            .iter()
            .filter_map(|(asset_id, asset)| {
                asset
                    .actual_decoded_bytes
                    .map(|bytes| (asset_id.clone(), bytes))
            })
            .collect(),
    })
}

fn spawn_prepared_node(
    world: &mut World,
    pending: &PendingBuild,
    prepared: &PreparedNode,
    instance_id: UiDocumentInstanceId,
    nodes: &mut BTreeMap<UiNodeId, Entity>,
) -> Result<Entity, &'static str> {
    let entity = world.spawn_empty().id();
    let node_id = prepared.source.id().clone();
    world.entity_mut(entity).insert((
        prepared.layout.node.clone(),
        UiNodeMarker {
            document_id: pending.request.document_id.clone(),
            node_id: node_id.clone(),
        },
        UiDocumentNodeMarker {
            instance_id,
            node_id: node_id.clone(),
        },
        UiDocumentNodeAuditMarker {
            instance_id,
            document_id: pending.request.document_id.clone(),
            schema_version: pending.prepared.validated.document().schema_version,
            node_id: node_id.clone(),
            document_path: prepared.path.clone(),
            source_path: pending.request.origin.audit_source_path(),
        },
        UiDocumentResolvedStyleMarker(prepared.style.clone()),
        Name::new(format!("UI document node {node_id}")),
    ));
    if let Some(z_index) = prepared.layout.z_index {
        world.entity_mut(entity).insert(z_index);
    }
    nodes.insert(node_id.clone(), entity);

    if prepared
        .children
        .iter()
        .any(|child| matches!(child.source, UiNode::Tab { .. }))
        && let Some(theme) = world.get_resource::<UiTheme>().cloned()
    {
        world.entity_mut(entity).insert(tab_list(&theme));
    }

    spawn_node_content(world, entity, pending, prepared, instance_id)?;
    apply_text_constraints(world, entity, &prepared.source, &prepared.style);
    apply_resolved_style(world, entity, &prepared.style);
    if let Some(control) = prepared.control {
        apply_control_state(world, entity, control);
    }

    for child in &prepared.children {
        let child_entity = spawn_prepared_node(world, pending, child, instance_id, nodes)?;
        world.entity_mut(entity).add_child(child_entity);
    }
    Ok(entity)
}

fn spawn_node_content(
    world: &mut World,
    entity: Entity,
    pending: &PendingBuild,
    prepared: &PreparedNode,
    instance_id: UiDocumentInstanceId,
) -> Result<(), &'static str> {
    match &prepared.source {
        UiNode::Container { .. } | UiNode::Spacer { .. } => {}
        UiNode::Text {
            content,
            typography,
            ..
        } => insert_text(world, entity, pending, content, typography, &prepared.style),
        UiNode::Image {
            asset,
            presentation,
            tint,
            placeholder,
            failure,
            ..
        } => insert_document_image(
            world,
            entity,
            pending,
            instance_id,
            asset,
            presentation,
            *tint,
            placeholder.as_ref(),
            failure,
        )?,
        UiNode::Icon { asset, tint, .. } => insert_image(
            world,
            entity,
            pending,
            asset,
            &UiImagePresentation::default(),
            *tint,
        )?,
        UiNode::Button {
            component,
            label,
            on_click,
            ..
        } => {
            let label = label
                .as_ref()
                .or_else(|| slot_content(component, UiControlSlot::Label));
            let label_text = label
                .map(|content| render_text(world, pending, content))
                .unwrap_or_default();
            let mut fallback_label = None;
            if let (Some(theme), Some(metrics), Some(fonts)) = (
                world.get_resource::<UiTheme>().cloned(),
                world
                    .get_resource::<crate::framework::ui::core::UiMetrics>()
                    .copied(),
                world.get_resource::<UiFontAssets>().cloned(),
            ) {
                match prepared.control.map(|control| control.variant) {
                    Some(UiWidgetVariantAdapter::Secondary) => {
                        world.entity_mut(entity).insert(secondary_action_button(
                            &theme, &metrics, &fonts, label_text,
                        ));
                    }
                    _ => {
                        world
                            .entity_mut(entity)
                            .insert(primary_action_button(&theme, &metrics, &fonts, label_text));
                    }
                }
            } else {
                world.entity_mut(entity).insert((Button, FocusableButton));
                fallback_label = Some(spawn_plain_text_child(world, entity, label_text));
            }
            if let Some(content) = label {
                let label_entity =
                    find_descendant_with::<UiButtonStyleLabel>(world, entity).or(fallback_label);
                if let Some(label_entity) = label_entity {
                    configure_generated_text(
                        world,
                        label_entity,
                        pending,
                        content,
                        &prepared.style,
                    );
                }
            }
            world.entity_mut(entity).insert(UiDocumentActionMarker {
                instance_id,
                node_id: prepared.source.id().clone(),
                action_id: on_click.action.clone(),
            });
        }
        UiNode::TextInput {
            component,
            value,
            max_chars,
            readonly,
            ..
        } => {
            let placeholder_content = slot_content(component, UiControlSlot::Placeholder);
            let placeholder = placeholder_content
                .map(|content| render_text(world, pending, content))
                .unwrap_or_default();
            if let (Some(theme), Some(metrics), Some(fonts)) = (
                world.get_resource::<UiTheme>().cloned(),
                world
                    .get_resource::<crate::framework::ui::core::UiMetrics>()
                    .copied(),
                world.get_resource::<UiFontAssets>().cloned(),
            ) {
                world.entity_mut(entity).insert(text_input(
                    &theme,
                    &metrics,
                    &fonts,
                    placeholder,
                    value.clone(),
                ));
            } else {
                world.entity_mut(entity).insert((Button, UiTextInput));
            }
            if let Some(max_chars) = max_chars {
                world
                    .entity_mut(entity)
                    .insert(UiTextInputMaxChars(*max_chars as usize));
            }
            if *readonly {
                world.entity_mut(entity).insert(ReadonlyTextInput);
            }
            if let Some(content) = placeholder_content
                && let Some(text_entity) = find_descendant_with::<UiTextInputText>(world, entity)
            {
                let spans = find_descendants_with::<TextSpan>(world, text_entity);
                let plain_span = spans.iter().copied().find(|span| {
                    world.get::<UiTextInputTextPart>(*span) == Some(&UiTextInputTextPart::Plain)
                });
                world
                    .entity_mut(text_entity)
                    .insert(UiDocumentTextInputSource {
                        control: entity,
                        plain_span,
                        document_id: pending.request.document_id.clone(),
                        owner: pending.request.owner.clone(),
                        content: content.clone(),
                    });
                apply_generated_text_style(world, text_entity, pending, &prepared.style);
                for span in spans {
                    apply_generated_text_style(world, span, pending, &prepared.style);
                }
            }
        }
        UiNode::Checkbox {
            component, checked, ..
        } => {
            let label_content = slot_content(component, UiControlSlot::Label);
            let label = label_content
                .map(|content| render_text(world, pending, content))
                .unwrap_or_default();
            let mut fallback_label = None;
            if let (Some(theme), Some(fonts)) = (
                world.get_resource::<UiTheme>().cloned(),
                world.get_resource::<UiFontAssets>().cloned(),
            ) {
                if *checked {
                    world.entity_mut(entity).insert(
                        crate::framework::ui::widgets::controls::checked_checkbox(
                            &theme, &fonts, label,
                        ),
                    );
                } else {
                    world.entity_mut(entity).insert(
                        crate::framework::ui::widgets::controls::checkbox(&theme, &fonts, label),
                    );
                }
            } else {
                world.entity_mut(entity).insert(Button);
                fallback_label = Some(spawn_plain_text_child(world, entity, label));
            }
            if let Some(content) = label_content
                && let Some(label_entity) =
                    find_descendant_with::<UiSelectionText>(world, entity).or(fallback_label)
            {
                configure_generated_text(world, label_entity, pending, content, &prepared.style);
            }
        }
        UiNode::Toggle { component, on, .. } => {
            let label_content = slot_content(component, UiControlSlot::Label);
            let label = label_content
                .map(|content| render_text(world, pending, content))
                .unwrap_or_default();
            let mut fallback_label = None;
            if let (Some(theme), Some(fonts)) = (
                world.get_resource::<UiTheme>().cloned(),
                world.get_resource::<UiFontAssets>().cloned(),
            ) {
                if *on {
                    world.entity_mut(entity).insert(
                        crate::framework::ui::widgets::controls::toggle_on(&theme, &fonts, label),
                    );
                } else {
                    world.entity_mut(entity).insert(
                        crate::framework::ui::widgets::controls::toggle(&theme, &fonts, label),
                    );
                }
            } else {
                world.entity_mut(entity).insert(Button);
                fallback_label = Some(spawn_plain_text_child(world, entity, label));
            }
            if let Some(content) = label_content
                && let Some(label_entity) =
                    find_descendant_with::<UiSelectionText>(world, entity).or(fallback_label)
            {
                configure_generated_text(world, label_entity, pending, content, &prepared.style);
            }
        }
        UiNode::Segmented {
            component,
            options,
            selected,
            ..
        } => {
            let theme = world.get_resource::<UiTheme>().cloned();
            let fonts = world.get_resource::<UiFontAssets>().cloned();
            if let Some(theme) = &theme {
                world.entity_mut(entity).insert(segmented_control(theme));
            } else {
                world.entity_mut(entity).insert(UiSegmentedControl);
            }
            if let Some(content) = slot_content(component, UiControlSlot::Label) {
                let text = render_text(world, pending, content);
                let label = spawn_plain_text_child(world, entity, text);
                configure_generated_text(world, label, pending, content, &prepared.style);
            }
            for option in options {
                let text = render_text(world, pending, &option.label);
                let is_selected = selected.as_ref() == Some(&option.value);
                let option_entity = world.spawn_empty().id();
                if let (Some(theme), Some(fonts)) = (&theme, &fonts) {
                    let state = if option.disabled {
                        SelectionVisualState::Disabled
                    } else if is_selected {
                        SelectionVisualState::Selected
                    } else {
                        SelectionVisualState::Idle
                    };
                    if option.disabled {
                        world
                            .entity_mut(option_entity)
                            .insert(segment_option_key_bundle(
                                theme,
                                fonts,
                                text,
                                option.value.clone(),
                                state,
                                (),
                                (),
                                DisabledButton,
                            ));
                    } else if is_selected {
                        world
                            .entity_mut(option_entity)
                            .insert(segment_option_key_bundle(
                                theme,
                                fonts,
                                text,
                                option.value.clone(),
                                state,
                                (),
                                (),
                                (UiSegmentOptionSelected, SelectedButton),
                            ));
                    } else {
                        world
                            .entity_mut(option_entity)
                            .insert(segment_option_key_bundle(
                                theme,
                                fonts,
                                text,
                                option.value.clone(),
                                state,
                                (),
                                (),
                                (),
                            ));
                    }
                } else {
                    world.entity_mut(option_entity).insert((
                        Button,
                        FocusableButton,
                        UiSegmentOption {
                            value: option.value.clone(),
                        },
                        UiControlFlags {
                            selected: is_selected,
                            disabled: option.disabled,
                            ..default()
                        },
                    ));
                    let label = spawn_plain_text_child(world, option_entity, text);
                    configure_generated_text(world, label, pending, &option.label, &prepared.style);
                }
                world.entity_mut(entity).add_child(option_entity);
                if let Some(label) = find_descendant_with::<UiSelectionText>(world, option_entity) {
                    configure_generated_text(world, label, pending, &option.label, &prepared.style);
                }
            }
        }
        UiNode::Slider {
            component,
            value,
            min,
            max,
            ..
        } => {
            let label_content = slot_content(component, UiControlSlot::Label);
            let label = label_content
                .map(|content| render_text(world, pending, content))
                .unwrap_or_default();
            if let (Some(theme), Some(metrics), Some(fonts)) = (
                world.get_resource::<UiTheme>().cloned(),
                world
                    .get_resource::<crate::framework::ui::core::UiMetrics>()
                    .copied(),
                world.get_resource::<UiFontAssets>().cloned(),
            ) {
                let disabled = prepared
                    .control
                    .is_some_and(|control| control.flags.disabled);
                world.entity_mut(entity).insert(slider_bundle(
                    &theme,
                    &metrics,
                    &fonts,
                    label,
                    *value,
                    *min,
                    *max,
                    UiI18nText::new("document.slider", ""),
                    UiControlMeta::new(UiControlId::new("document.slider"), UiControlKind::Slider),
                    disabled,
                ));
                if let Some(content) = label_content
                    && let Some(label_entity) =
                        find_descendant_with::<UiNumericControlLabel>(world, entity)
                {
                    configure_generated_text(
                        world,
                        label_entity,
                        pending,
                        content,
                        &prepared.style,
                    );
                }
            } else {
                world
                    .entity_mut(entity)
                    .insert((Button, UiSlider::new(*value, *min, *max)));
                let label_entity = spawn_plain_text_child(world, entity, label);
                if let Some(content) = label_content {
                    configure_generated_text(
                        world,
                        label_entity,
                        pending,
                        content,
                        &prepared.style,
                    );
                }
            }
        }
        UiNode::Stepper {
            component,
            value,
            min,
            max,
            step,
            ..
        } => {
            let label_content = slot_content(component, UiControlSlot::Label);
            let label = label_content
                .map(|content| render_text(world, pending, content))
                .unwrap_or_default();
            if let (Some(theme), Some(metrics), Some(fonts)) = (
                world.get_resource::<UiTheme>().cloned(),
                world
                    .get_resource::<crate::framework::ui::core::UiMetrics>()
                    .copied(),
                world.get_resource::<UiFontAssets>().cloned(),
            ) {
                let disabled = prepared
                    .control
                    .is_some_and(|control| control.flags.disabled);
                if disabled {
                    world.entity_mut(entity).insert(stepper_bundle(
                        &theme,
                        &metrics,
                        &fonts,
                        label,
                        *value,
                        *min,
                        *max,
                        *step,
                        UiI18nText::new("document.stepper", ""),
                        (
                            UiControlMeta::new(
                                UiControlId::new("document.stepper"),
                                UiControlKind::Stepper,
                            ),
                            DisabledButton,
                        ),
                        (UiStepperDecrementButton, DisabledButton),
                        (UiStepperIncrementButton, DisabledButton),
                        true,
                    ));
                } else {
                    world.entity_mut(entity).insert(stepper_bundle(
                        &theme,
                        &metrics,
                        &fonts,
                        label,
                        *value,
                        *min,
                        *max,
                        *step,
                        UiI18nText::new("document.stepper", ""),
                        UiControlMeta::new(
                            UiControlId::new("document.stepper"),
                            UiControlKind::Stepper,
                        ),
                        UiStepperDecrementButton,
                        UiStepperIncrementButton,
                        false,
                    ));
                }
                if let Some(content) = label_content
                    && let Some(label_entity) =
                        find_descendant_with::<UiNumericControlLabel>(world, entity)
                {
                    configure_generated_text(
                        world,
                        label_entity,
                        pending,
                        content,
                        &prepared.style,
                    );
                }
            } else {
                world
                    .entity_mut(entity)
                    .insert(UiStepper::new(*value, *min, *max, *step));
                let label_entity = spawn_plain_text_child(world, entity, label);
                if let Some(content) = label_content {
                    configure_generated_text(
                        world,
                        label_entity,
                        pending,
                        content,
                        &prepared.style,
                    );
                }
            }
        }
        UiNode::Scroll {
            row_gap,
            max_height,
            block_lower,
            ..
        } => {
            let config = UiScrollViewConfig {
                row_gap: *row_gap,
                max_height: max_height.map_or(Val::Auto, Val::Px),
                should_block_lower: *block_lower,
            };
            world
                .entity_mut(entity)
                .insert(ui_scroll_column_bundle(config));
        }
        UiNode::Modal { component, .. } => {
            for slot in [UiControlSlot::Title, UiControlSlot::Body] {
                if let Some(content) = slot_content(component, slot) {
                    let text = render_text(world, pending, content);
                    let label = spawn_plain_text_child(world, entity, text);
                    configure_generated_text(world, label, pending, content, &prepared.style);
                }
            }
        }
        UiNode::ImageButton {
            asset,
            presentation,
            tint,
            component,
            ..
        } => {
            world.entity_mut(entity).insert((Button, FocusableButton));
            insert_image(world, entity, pending, asset, presentation, *tint)?;
            if let Some(content) = slot_content(component, UiControlSlot::Label) {
                let text = render_text(world, pending, content);
                let label = spawn_plain_text_child(world, entity, text);
                configure_generated_text(world, label, pending, content, &prepared.style);
            }
        }
        UiNode::Badge { component, .. } => {
            let state = prepared
                .control
                .map(|control| control.state)
                .unwrap_or_default();
            let content = slot_content(component, UiControlSlot::Label);
            let text = content
                .map(|content| render_text(world, pending, content))
                .unwrap_or_default();
            if let (Some(theme), Some(fonts)) = (
                world.get_resource::<UiTheme>().cloned(),
                world.get_resource::<UiFontAssets>().cloned(),
            ) {
                world
                    .entity_mut(entity)
                    .insert(badge(&theme, &fonts, text, state));
                if let Some(content) = content
                    && let Some(label) = find_descendant_with::<UiBadgeLabel>(world, entity)
                {
                    configure_generated_text(world, label, pending, content, &prepared.style);
                }
            } else {
                world.entity_mut(entity).insert(UiBadge { state });
                let label = spawn_plain_text_child(world, entity, text);
                if let Some(content) = content {
                    configure_generated_text(world, label, pending, content, &prepared.style);
                }
            }
        }
        UiNode::Progress {
            component, value, ..
        } => {
            let state = prepared
                .control
                .map(|control| control.state)
                .unwrap_or_default();
            let content = slot_content(component, UiControlSlot::Label);
            let text = content
                .map(|content| render_text(world, pending, content))
                .unwrap_or_default();
            if let (Some(theme), Some(fonts)) = (
                world.get_resource::<UiTheme>().cloned(),
                world.get_resource::<UiFontAssets>().cloned(),
            ) {
                world
                    .entity_mut(entity)
                    .insert(progress(&theme, &fonts, text, *value, state));
                if let Some(content) = content
                    && let Some(label) = find_descendant_with::<UiProgressLabel>(world, entity)
                {
                    world.entity_mut(label).insert(UiDocumentProgressSource {
                        control: entity,
                        document_id: pending.request.document_id.clone(),
                        owner: pending.request.owner.clone(),
                        content: content.clone(),
                    });
                    apply_generated_text_style(world, label, pending, &prepared.style);
                }
            } else {
                world
                    .entity_mut(entity)
                    .insert(UiProgress::new(*value, state));
                let label = spawn_plain_text_child(world, entity, text);
                if let Some(content) = content {
                    configure_generated_text(world, label, pending, content, &prepared.style);
                }
            }
        }
        UiNode::Tab {
            component, value, ..
        } => {
            let state = prepared
                .control
                .map(|control| control.state)
                .unwrap_or_default();
            let content = slot_content(component, UiControlSlot::Label);
            let text = content
                .map(|content| render_text(world, pending, content))
                .unwrap_or_default();
            if let (Some(theme), Some(fonts)) = (
                world.get_resource::<UiTheme>().cloned(),
                world.get_resource::<UiFontAssets>().cloned(),
            ) {
                world
                    .entity_mut(entity)
                    .insert(tab(&theme, &fonts, value.clone(), text, state));
                if let Some(content) = content
                    && let Some(label) = find_descendant_with::<UiTabLabel>(world, entity)
                {
                    configure_generated_text(world, label, pending, content, &prepared.style);
                }
            } else {
                world.entity_mut(entity).insert((
                    Button,
                    FocusableButton,
                    UiTab {
                        value: value.clone(),
                    },
                ));
                let label = spawn_plain_text_child(world, entity, text);
                if let Some(content) = content {
                    configure_generated_text(world, label, pending, content, &prepared.style);
                }
            }
        }
        UiNode::Tooltip {
            component, tone, ..
        } => {
            let content = slot_content(component, UiControlSlot::Body);
            let text = content
                .map(|content| render_text(world, pending, content))
                .unwrap_or_default();
            world.entity_mut(entity).insert((
                Button,
                FocusableButton,
                UiControlMeta::new(UiControlId::new("document.tooltip"), UiControlKind::Tooltip),
                UiTooltip {
                    text,
                    tone: match tone {
                        UiTooltipToneSpec::Standard => UiTooltipTone::Standard,
                        UiTooltipToneSpec::Error => UiTooltipTone::Error,
                    },
                },
            ));
            if let Some(content) = content {
                world.entity_mut(entity).insert(UiDocumentTooltipSource {
                    document_id: pending.request.document_id.clone(),
                    owner: pending.request.owner.clone(),
                    content: content.clone(),
                });
            }
        }
        UiNode::Select {
            component,
            options,
            selected,
            ..
        } => {
            let options = options
                .iter()
                .map(|option| {
                    let option_value = UiDropdownOption::new(
                        option.value.clone(),
                        render_text(world, pending, &option.label),
                    );
                    if option.disabled {
                        option_value.disabled()
                    } else {
                        option_value
                    }
                })
                .collect::<Vec<_>>();
            let selected_index = selected
                .as_ref()
                .and_then(|selected| options.iter().position(|option| &option.value == selected));
            let placeholder = slot_text(world, pending, component, UiControlSlot::Placeholder);
            if let (Some(theme), Some(fonts), Some(asset_server), Some(i18n)) = (
                world.get_resource::<UiTheme>().cloned(),
                world.get_resource::<UiFontAssets>().cloned(),
                world.get_resource::<AssetServer>().cloned(),
                world.get_resource::<UiI18n>().cloned(),
            ) {
                world.entity_mut(entity).insert(dropdown_key(
                    &theme,
                    &fonts,
                    &asset_server,
                    &i18n,
                    "document.dropdown",
                    "Select",
                    options,
                    selected_index,
                    prepared
                        .control
                        .map(|control| control.state)
                        .unwrap_or_default(),
                ));
                if let Some(label) = find_descendant_with::<UiDropdownLabel>(world, entity) {
                    world.entity_mut(label).insert(UiDocumentDropdownSources {
                        control: entity,
                        document_id: pending.request.document_id.clone(),
                        owner: pending.request.owner.clone(),
                        placeholder: slot_content(component, UiControlSlot::Placeholder).cloned(),
                        empty: slot_content(component, UiControlSlot::Empty).cloned(),
                        error: slot_content(component, UiControlSlot::Error).cloned(),
                        options: options_content(&prepared.source),
                    });
                    apply_generated_text_style(world, label, pending, &prepared.style);
                }
            } else {
                world.entity_mut(entity).insert((
                    Button,
                    FocusableButton,
                    UiDropdown::new(placeholder.clone(), options, selected_index),
                ));
                let label = spawn_plain_text_child(world, entity, placeholder);
                world.entity_mut(label).insert(UiDropdownLabel);
                world.entity_mut(label).insert(UiDocumentDropdownSources {
                    control: entity,
                    document_id: pending.request.document_id.clone(),
                    owner: pending.request.owner.clone(),
                    placeholder: slot_content(component, UiControlSlot::Placeholder).cloned(),
                    empty: slot_content(component, UiControlSlot::Empty).cloned(),
                    error: slot_content(component, UiControlSlot::Error).cloned(),
                    options: options_content(&prepared.source),
                });
                apply_generated_text_style(world, label, pending, &prepared.style);
            }
        }
    }
    materialize_remaining_control_slots(world, entity, pending, prepared)?;
    let framework_node = world.get::<Node>(entity).cloned();
    world
        .entity_mut(entity)
        .insert(prepared.layout.node.clone());
    if let Some(control) = prepared.control {
        apply_control_presentation(
            world,
            entity,
            pending,
            prepared,
            control,
            framework_node.as_ref(),
        );
    }
    Ok(())
}

fn materialize_remaining_control_slots(
    world: &mut World,
    parent: Entity,
    pending: &PendingBuild,
    prepared: &PreparedNode,
) -> Result<(), &'static str> {
    let Some(component) = prepared.source.component() else {
        return Ok(());
    };
    let slots: &[UiControlSlot] = match &prepared.source {
        UiNode::Button { .. } => &[UiControlSlot::Leading, UiControlSlot::Trailing],
        UiNode::TextInput { .. } => &[
            UiControlSlot::Label,
            UiControlSlot::Helper,
            UiControlSlot::Error,
        ],
        UiNode::Segmented { .. } | UiNode::Slider { .. } | UiNode::Stepper { .. } => {
            &[UiControlSlot::Helper, UiControlSlot::Error]
        }
        UiNode::Scroll { .. } => &[UiControlSlot::Empty, UiControlSlot::Error],
        UiNode::Modal { .. } => &[UiControlSlot::Error],
        UiNode::Tab { .. } => &[UiControlSlot::Leading],
        UiNode::Select { .. } => &[UiControlSlot::Label],
        _ => &[],
    };
    let state = prepared
        .control
        .map(|control| control.state)
        .unwrap_or_default();
    for slot in slots {
        let Some(content) = component.slots.get(slot) else {
            continue;
        };
        let child = world.spawn_empty().id();
        let visible = match slot {
            UiControlSlot::Error => state == UiControlState::Error,
            UiControlSlot::Empty => state == UiControlState::Empty,
            UiControlSlot::Helper => {
                state != UiControlState::Error && state != UiControlState::Empty
            }
            _ => true,
        };
        world.entity_mut(child).insert((
            UiDocumentControlSlotMarker(*slot),
            if visible {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            },
        ));
        match content {
            UiControlSlotContent::Text { content } => {
                let text = render_text(world, pending, content);
                world.entity_mut(child).insert((
                    Text::new(text),
                    TextFont::default(),
                    TextColor(Color::WHITE),
                ));
                configure_generated_text(world, child, pending, content, &prepared.style);
            }
            UiControlSlotContent::Icon { asset, tint } => {
                insert_image(
                    world,
                    child,
                    pending,
                    asset,
                    &UiImagePresentation::default(),
                    *tint,
                )?;
            }
            UiControlSlotContent::Image {
                asset,
                presentation,
                tint,
            } => insert_image(world, child, pending, asset, presentation, *tint)?,
        }
        if matches!(slot, UiControlSlot::Leading) {
            world.entity_mut(parent).insert_children(0, &[child]);
        } else {
            world.entity_mut(parent).add_child(child);
        }
    }
    Ok(())
}

fn apply_control_presentation(
    world: &mut World,
    entity: Entity,
    pending: &PendingBuild,
    prepared: &PreparedNode,
    adapter: UiWidgetControlAdapter,
    framework_node: Option<&Node>,
) {
    let variant = match adapter.variant {
        UiWidgetVariantAdapter::Primary => UiComponentVariant::Primary,
        UiWidgetVariantAdapter::Secondary => UiComponentVariant::Secondary,
        UiWidgetVariantAdapter::Document(variant) => variant,
    };
    let prepared_styles = prepared
        .control_styles
        .as_ref()
        .expect("control nodes retain prepared state styles");
    let font_handles = pending
        .assets
        .iter()
        .filter_map(|(asset_id, asset)| match asset.handle.as_ref() {
            Some(UiDocumentResolvedAsset::Font(handle)) => Some((asset_id.clone(), handle.clone())),
            _ => None,
        })
        .collect();
    world.entity_mut(entity).insert((
        UiDocumentControlPresentation {
            variant,
            size: adapter.size,
            state: adapter.state,
        },
        UiDocumentControlStateStyles {
            base: prepared_styles.base.clone(),
            states: prepared_styles.states.clone(),
            font_handles,
        },
        UiDocumentControlCurrentState(adapter.state),
    ));
    let Some(theme) = world.get_resource::<UiTheme>().cloned() else {
        if let Some(node) = world.get::<Node>(entity).cloned() {
            world
                .entity_mut(entity)
                .insert(UiDocumentControlLayout(node));
        }
        return;
    };
    let (background, border) = control_variant_colors(&theme, variant, adapter.state);
    if let Some(background) = background {
        world.entity_mut(entity).insert(BackgroundColor(background));
    }
    if let Some(border) = border {
        world.entity_mut(entity).insert(BorderColor::all(border));
    }
    let scale = match adapter.size {
        UiComponentSize::Small => 0.85,
        UiComponentSize::Medium => 1.0,
        UiComponentSize::Large => 1.15,
    };
    if let Some(mut node) = world.get_mut::<Node>(entity) {
        let source_layout = prepared.source.layout();
        if source_layout.min_height == super::UiLength::Auto {
            node.min_height = px(theme.button.height * scale);
        }
        if source_layout.padding == super::UiInsets::default() {
            if let Some(framework_node) = framework_node {
                node.padding = framework_node.padding;
            }
            node.padding.left = px(theme.button.padding_x * scale);
            node.padding.right = px(theme.button.padding_x * scale);
        }
    }
    if let Some(node) = world.get::<Node>(entity).cloned() {
        world
            .entity_mut(entity)
            .insert(UiDocumentControlLayout(node));
    }
    let baseline = UiDocumentControlVisualBaseline {
        background: world.get::<BackgroundColor>(entity).copied(),
        border: world.get::<BorderColor>(entity).cloned(),
        text_color: world.get::<TextColor>(entity).copied(),
        image_color: world.get::<ImageNode>(entity).map(|image| image.color),
    };
    world.entity_mut(entity).insert(baseline);

    for child in find_internal_descendants_with::<TextFont>(world, entity) {
        let generated = world.get::<UiDocumentGeneratedTextStyle>(child).cloned();
        let baseline = UiDocumentControlTextBaseline {
            font: generated
                .as_ref()
                .and_then(|style| style.base_font.clone())
                .or_else(|| world.get::<TextFont>(child).cloned())
                .unwrap_or_default(),
            color: generated
                .as_ref()
                .map(|style| style.base_color)
                .or_else(|| world.get::<TextColor>(child).map(|color| color.0))
                .unwrap_or(Color::WHITE),
            line_height: generated
                .as_ref()
                .and_then(|style| style.base_line_height)
                .or_else(|| world.get::<LineHeight>(child).copied()),
            last_applied_color: generated
                .as_ref()
                .and_then(|style| style.last_applied_color),
        };
        world
            .entity_mut(child)
            .remove::<UiDocumentGeneratedTextStyle>()
            .insert(baseline);
    }
}

fn control_variant_colors(
    theme: &UiTheme,
    variant: UiComponentVariant,
    state: UiControlState,
) -> (Option<Color>, Option<Color>) {
    let state_color = |colors: ButtonColors| match state {
        UiControlState::Hovered => colors.hovered,
        UiControlState::Pressed => colors.pressed,
        UiControlState::Focused => colors.focused,
        UiControlState::Selected => colors.selected,
        UiControlState::Disabled => colors.disabled,
        UiControlState::Loading => colors.loading,
        UiControlState::Normal | UiControlState::Empty | UiControlState::Error => colors.idle,
    };
    match variant {
        UiComponentVariant::Default => (None, None),
        UiComponentVariant::Primary => (Some(state_color(theme.colors.primary_button)), None),
        UiComponentVariant::Secondary => (Some(state_color(theme.colors.secondary_button)), None),
        UiComponentVariant::Destructive | UiComponentVariant::Error => (
            Some(
                theme
                    .colors
                    .error
                    .with_alpha(if state == UiControlState::Pressed {
                        0.48
                    } else {
                        0.32
                    }),
            ),
            Some(theme.colors.error),
        ),
        UiComponentVariant::Subtle => (
            Some(state_color(theme.colors.secondary_button).with_alpha(0.55)),
            None,
        ),
        UiComponentVariant::Outline => (Some(Color::NONE), Some(theme.colors.panel_border)),
        UiComponentVariant::Info => (
            Some(theme.colors.primary_button.focused.with_alpha(0.32)),
            Some(theme.colors.primary_button.focused),
        ),
        UiComponentVariant::Success => (
            Some(theme.colors.primary_button.selected.with_alpha(0.42)),
            Some(theme.colors.primary_button.selected),
        ),
        UiComponentVariant::Warning => (
            Some(theme.colors.secondary_button.focused.with_alpha(0.52)),
            Some(theme.colors.secondary_button.focused),
        ),
    }
}

fn slot_content(component: &super::UiComponentSpec, slot: UiControlSlot) -> Option<&UiTextContent> {
    match component.slots.get(&slot) {
        Some(UiControlSlotContent::Text { content }) => Some(content),
        _ => None,
    }
}

fn options_content(node: &UiNode) -> Vec<UiTextContent> {
    match node {
        UiNode::Select { options, .. } => {
            options.iter().map(|option| option.label.clone()).collect()
        }
        _ => Vec::new(),
    }
}

fn find_descendant_with<T: Component>(world: &World, root: Entity) -> Option<Entity> {
    find_descendants_with::<T>(world, root).into_iter().next()
}

fn find_descendants_with<T: Component>(world: &World, root: Entity) -> Vec<Entity> {
    let mut matches = Vec::new();
    let mut pending = world
        .get::<Children>(root)
        .map(|children| children.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    while let Some(entity) = pending.pop() {
        if world.get::<T>(entity).is_some() {
            matches.push(entity);
        }
        if let Some(children) = world.get::<Children>(entity) {
            pending.extend(children.iter());
        }
    }
    matches
}

fn find_internal_descendants_with<T: Component>(world: &World, root: Entity) -> Vec<Entity> {
    let mut matches = Vec::new();
    let mut pending = world
        .get::<Children>(root)
        .map(|children| children.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    while let Some(entity) = pending.pop() {
        if world.get::<UiDocumentNodeMarker>(entity).is_some() {
            continue;
        }
        if world.get::<T>(entity).is_some() {
            matches.push(entity);
        }
        if let Some(children) = world.get::<Children>(entity) {
            pending.extend(children.iter());
        }
    }
    matches
}

fn configure_generated_text(
    world: &mut World,
    entity: Entity,
    pending: &PendingBuild,
    content: &UiTextContent,
    resolved_style: &UiResolvedStyle,
) {
    let rendered = render_text(world, pending, content);
    if let Some(mut text) = world.get_mut::<Text>(entity) {
        text.0 = rendered;
    }
    world
        .entity_mut(entity)
        .remove::<UiI18nText>()
        .remove::<UiDocumentBoundText>();
    match content {
        UiTextContent::I18n(source) => {
            world.entity_mut(entity).insert(UiI18nText::new(
                source.i18n_key.as_str(),
                source.fallback.clone(),
            ));
        }
        UiTextContent::Binding(source) => {
            world.entity_mut(entity).insert(UiDocumentBoundText {
                document_id: pending.request.document_id.clone(),
                owner: pending.request.owner.clone(),
                path: source.binding_path.clone(),
                format: source.format.clone(),
                fallback: source.fallback.clone(),
            });
        }
        UiTextContent::Literal(_) => {}
    }
    apply_generated_text_style(world, entity, pending, resolved_style);
}

fn apply_generated_text_style(
    world: &mut World,
    entity: Entity,
    pending: &PendingBuild,
    resolved_style: &UiResolvedStyle,
) {
    let visual = resolved_style.properties.text.as_ref();
    let opacity = resolved_style.properties.opacity;
    let explicit_color = visual.and_then(|style| style.color).map(ui_color);
    let base_color = world
        .get::<TextColor>(entity)
        .map(|color| color.0)
        .unwrap_or(Color::WHITE);
    let font = visual
        .and_then(|style| style.font.as_ref())
        .and_then(|font_id| pending.assets.get(font_id))
        .and_then(|asset| asset.handle.as_ref())
        .and_then(|asset| match asset {
            UiDocumentResolvedAsset::Font(handle) => Some(handle.clone()),
            UiDocumentResolvedAsset::Image(_) | UiDocumentResolvedAsset::BuiltInMaterial => None,
        });
    let font_size = visual.and_then(|style| style.font_size);
    let line_height = visual
        .and_then(|style| style.line_height)
        .map(LineHeight::Px);
    let weight = visual
        .and_then(|style| style.weight)
        .map(|weight| match weight {
            super::UiTextWeight::Regular => FontWeight::NORMAL,
            super::UiTextWeight::Medium => FontWeight::MEDIUM,
            super::UiTextWeight::Bold => FontWeight::BOLD,
        });
    if explicit_color.is_none()
        && opacity.is_none()
        && font.is_none()
        && font_size.is_none()
        && line_height.is_none()
        && weight.is_none()
    {
        return;
    }
    let mut marker = UiDocumentGeneratedTextStyle {
        explicit_color,
        opacity: opacity.unwrap_or(1.0),
        base_color,
        last_applied_color: None,
        font,
        font_size,
        line_height,
        weight,
        base_font: world.get::<TextFont>(entity).cloned(),
        base_line_height: world.get::<LineHeight>(entity).copied(),
    };
    apply_document_generated_text_style(world, entity, &mut marker);
    world.entity_mut(entity).insert(marker);
}

fn insert_text(
    world: &mut World,
    entity: Entity,
    pending: &PendingBuild,
    content: &UiTextContent,
    typography: &UiTextTypography,
    resolved_style: &UiResolvedStyle,
) {
    let text = render_text(world, pending, content);
    let text_visual = resolved_style.properties.text.as_ref();
    let font_size = text_visual
        .and_then(|style| style.font_size)
        .unwrap_or(18.0);
    let color = text_visual
        .and_then(|style| style.color)
        .map(ui_color)
        .unwrap_or(Color::WHITE);
    let mut adapter = typography.to_framework_adapter(font_size);
    if let Some(weight) = text_visual.and_then(|style| style.weight) {
        adapter.style.font_weight = match weight {
            super::UiTextWeight::Regular => UiFontWeight::Regular,
            super::UiTextWeight::Medium => UiFontWeight::Medium,
            super::UiTextWeight::Bold => UiFontWeight::Bold,
        };
    }
    if let Some(line_height) = text_visual.and_then(|style| style.line_height) {
        adapter.style.line_height = FrameworkTextLineHeight::Pixels(line_height);
    }
    if let Some(fonts) = world.get_resource::<UiFontAssets>().cloned()
        && let Ok(bundle) = try_ui_styled_text(&fonts, text.clone(), adapter.style.clone(), color)
    {
        world.entity_mut(entity).insert(bundle);
    } else {
        world.entity_mut(entity).insert((
            Text::new(text.clone()),
            TextFont {
                font_size,
                ..default()
            },
            TextColor(color),
            adapter.bevy_layout,
            resolved_line_height(typography, resolved_style, font_size),
            adapter.style.clone(),
        ));
    }
    if let Some(font_id) = text_visual.and_then(|style| style.font.as_ref())
        && let Some(UiDocumentResolvedAsset::Font(handle)) = pending
            .assets
            .get(font_id)
            .and_then(|asset| asset.handle.as_ref())
    {
        if let Some(mut font) = world.get_mut::<TextFont>(entity) {
            font.font = handle.clone();
        }
    }
    match content {
        UiTextContent::I18n(source) => {
            world.entity_mut(entity).insert(UiI18nText::new(
                source.i18n_key.as_str(),
                source.fallback.clone(),
            ));
        }
        UiTextContent::Binding(source) => {
            world.entity_mut(entity).insert(UiDocumentBoundText {
                document_id: pending.request.document_id.clone(),
                owner: pending.request.owner.clone(),
                path: source.binding_path.clone(),
                format: source.format.clone(),
                fallback: source.fallback.clone(),
            });
        }
        UiTextContent::Literal(_) => {}
    }
}

#[derive(Clone, Debug, Component)]
struct UiDocumentBoundText {
    document_id: UiDocumentId,
    owner: String,
    path: UiBindingPath,
    format: UiTextFormat,
    fallback: String,
}

#[derive(Clone, Debug, Component)]
struct UiDocumentTextInputSource {
    control: Entity,
    plain_span: Option<Entity>,
    document_id: UiDocumentId,
    owner: String,
    content: UiTextContent,
}

#[derive(Clone, Debug, Component)]
struct UiDocumentDropdownSources {
    control: Entity,
    document_id: UiDocumentId,
    owner: String,
    placeholder: Option<UiTextContent>,
    empty: Option<UiTextContent>,
    error: Option<UiTextContent>,
    options: Vec<UiTextContent>,
}

#[derive(Clone, Debug, Component)]
struct UiDocumentTooltipSource {
    document_id: UiDocumentId,
    owner: String,
    content: UiTextContent,
}

#[derive(Clone, Debug, Component)]
struct UiDocumentProgressSource {
    control: Entity,
    document_id: UiDocumentId,
    owner: String,
    content: UiTextContent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Component)]
struct UiDocumentControlSlotMarker(UiControlSlot);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Component)]
struct UiDocumentControlPresentation {
    variant: UiComponentVariant,
    size: UiComponentSize,
    state: UiControlState,
}

#[derive(Clone, Debug, Component)]
struct UiDocumentControlStateStyles {
    base: UiResolvedStyle,
    states: BTreeMap<UiComponentState, UiResolvedStyle>,
    font_handles: BTreeMap<UiAssetId, Handle<Font>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Component)]
struct UiDocumentControlCurrentState(UiControlState);

#[derive(Clone, Debug, Component)]
struct UiDocumentControlLayout(Node);

#[derive(Clone, Debug, Component)]
struct UiDocumentControlVisualBaseline {
    background: Option<BackgroundColor>,
    border: Option<BorderColor>,
    text_color: Option<TextColor>,
    image_color: Option<Color>,
}

#[derive(Clone, Debug, Component)]
struct UiDocumentControlTextBaseline {
    font: TextFont,
    color: Color,
    line_height: Option<LineHeight>,
    last_applied_color: Option<Color>,
}

#[derive(Clone, Debug, Component)]
struct UiDocumentGeneratedTextStyle {
    explicit_color: Option<Color>,
    opacity: f32,
    base_color: Color,
    last_applied_color: Option<Color>,
    font: Option<Handle<Font>>,
    font_size: Option<f32>,
    line_height: Option<LineHeight>,
    weight: Option<FontWeight>,
    base_font: Option<TextFont>,
    base_line_height: Option<LineHeight>,
}

#[derive(Clone, Copy, Debug, Component)]
struct UiDocumentTextConstraint {
    max_lines: Option<u16>,
    overflow: UiTextOverflow,
    line_height_px: f32,
}

fn apply_text_constraints(
    world: &mut World,
    entity: Entity,
    node: &UiNode,
    resolved_style: &UiResolvedStyle,
) {
    let UiNode::Text { typography, .. } = node else {
        return;
    };
    let font_size = resolved_style
        .properties
        .text
        .as_ref()
        .and_then(|style| style.font_size)
        .unwrap_or(18.0);
    let line_height_px = resolved_line_height_px(typography, resolved_style, font_size);
    world.entity_mut(entity).insert(UiDocumentTextConstraint {
        max_lines: typography.max_lines,
        overflow: typography.overflow,
        line_height_px,
    });
    apply_text_node_constraint(
        world
            .get_mut::<Node>(entity)
            .expect("document text nodes always have Node"),
        typography.max_lines,
        typography.overflow,
        line_height_px,
    );
}

fn resolved_line_height(
    typography: &UiTextTypography,
    resolved_style: &UiResolvedStyle,
    _font_size: f32,
) -> LineHeight {
    if let Some(line_height) = resolved_style
        .properties
        .text
        .as_ref()
        .and_then(|style| style.line_height)
    {
        return LineHeight::Px(line_height);
    }
    match typography.line_height {
        UiTextLineHeight::Normal => LineHeight::RelativeToFont(1.2),
        UiTextLineHeight::Relative(value) => LineHeight::RelativeToFont(value),
        UiTextLineHeight::Pixels(value) => LineHeight::Px(value),
    }
}

fn resolved_line_height_px(
    typography: &UiTextTypography,
    resolved_style: &UiResolvedStyle,
    font_size: f32,
) -> f32 {
    match resolved_line_height(typography, resolved_style, font_size) {
        LineHeight::Px(value) => value,
        LineHeight::RelativeToFont(value) => value * font_size,
    }
}

fn apply_text_node_constraint(
    mut node: Mut<Node>,
    max_lines: Option<u16>,
    overflow: UiTextOverflow,
    line_height_px: f32,
) {
    let clip = Overflow::clip();
    if overflow != UiTextOverflow::Visible && node.overflow != clip {
        node.overflow = clip;
    }
    if let Some(max_lines) = max_lines {
        let max_height = px(line_height_px * f32::from(max_lines));
        if node.max_height != max_height {
            node.max_height = max_height;
        }
    }
}

fn constrain_explicit_lines(
    text: &str,
    max_lines: Option<u16>,
    overflow: UiTextOverflow,
) -> String {
    let Some(max_lines) = max_lines.map(usize::from) else {
        return text.to_owned();
    };
    if overflow != UiTextOverflow::Ellipsis {
        return text.to_owned();
    }
    let lines = text.split('\n').collect::<Vec<_>>();
    if lines.len() <= max_lines {
        return text.to_owned();
    }
    let mut constrained = lines[..max_lines].join("\n");
    while matches!(constrained.chars().last(), Some('\r' | ' ' | '\t')) {
        constrained.pop();
    }
    constrained.push('…');
    constrained
}

fn render_text(world: &World, pending: &PendingBuild, content: &UiTextContent) -> String {
    match content {
        UiTextContent::Literal(source) => source.literal.clone(),
        UiTextContent::I18n(source) => world
            .get_resource::<UiI18n>()
            .map(|i18n| i18n.tr(source.i18n_key.as_str(), source.fallback.clone()))
            .unwrap_or_else(|| source.fallback.clone()),
        UiTextContent::Binding(source) => {
            let declaration = pending
                .prepared
                .validated
                .document()
                .bindings
                .get(&source.binding_path);
            declaration
                .and_then(|declaration| {
                    world.get_resource::<UiBindingValues>()?.scoped_value(
                        pending.request.document_id.as_str(),
                        &pending.request.owner,
                        &source.binding_path,
                        declaration,
                    )
                })
                .map(|value| format_binding_value(&value, &source.format))
                .unwrap_or_else(|| source.fallback.clone())
        }
    }
}

fn format_binding_value(value: &super::UiBindingValue, format: &UiTextFormat) -> String {
    use super::{UiBindingValue, UiBindingVisibility};
    match (value, format) {
        (UiBindingValue::String(value), UiTextFormat::Plain)
        | (UiBindingValue::Enum(value), UiTextFormat::Plain) => value.clone(),
        (UiBindingValue::Bool(value), UiTextFormat::Plain) => value.to_string(),
        (UiBindingValue::Visibility(UiBindingVisibility::Inherited), UiTextFormat::Plain) => {
            "inherited".to_owned()
        }
        (UiBindingValue::Visibility(UiBindingVisibility::Visible), UiTextFormat::Plain) => {
            "visible".to_owned()
        }
        (UiBindingValue::Visibility(UiBindingVisibility::Hidden), UiTextFormat::Plain) => {
            "hidden".to_owned()
        }
        (
            UiBindingValue::Number(value),
            UiTextFormat::Number {
                min_fraction_digits,
                max_fraction_digits,
                grouping,
            },
        ) => format_decimal(
            *value,
            *min_fraction_digits,
            *max_fraction_digits,
            *grouping,
        ),
        (
            UiBindingValue::Number(value),
            UiTextFormat::Percent {
                min_fraction_digits,
                max_fraction_digits,
            },
        ) => format!(
            "{}%",
            format_decimal(
                value * 100.0,
                *min_fraction_digits,
                *max_fraction_digits,
                false,
            )
        ),
        (
            UiBindingValue::Number(value),
            UiTextFormat::Bytes {
                precision,
                binary_units,
            },
        ) => {
            let base = if *binary_units { 1024.0 } else { 1000.0 };
            let suffixes = if *binary_units {
                ["B", "KiB", "MiB", "GiB"]
            } else {
                ["B", "KB", "MB", "GB"]
            };
            let mut scaled = value.max(0.0);
            let mut index = 0;
            while scaled >= base && index + 1 < suffixes.len() {
                scaled /= base;
                index += 1;
            }
            format!(
                "{scaled:.precision$} {}",
                suffixes[index],
                precision = *precision as usize
            )
        }
        (UiBindingValue::Number(value), UiTextFormat::Plain) => value.to_string(),
        _ => String::new(),
    }
}

fn format_decimal(
    value: f64,
    min_fraction_digits: u8,
    max_fraction_digits: u8,
    grouping: bool,
) -> String {
    let max_fraction_digits = max_fraction_digits as usize;
    let min_fraction_digits = min_fraction_digits.min(max_fraction_digits as u8) as usize;
    let mut formatted = format!("{value:.max_fraction_digits$}");
    if let Some(decimal_index) = formatted.find('.') {
        while formatted.len() > decimal_index + 1 + min_fraction_digits && formatted.ends_with('0')
        {
            formatted.pop();
        }
        if formatted.ends_with('.') {
            formatted.pop();
        }
    }
    if !grouping {
        return formatted;
    }

    let (sign, unsigned) = formatted
        .strip_prefix('-')
        .map_or(("", formatted.as_str()), |unsigned| ("-", unsigned));
    let (integer, fraction) = unsigned
        .split_once('.')
        .map_or((unsigned, None), |(integer, fraction)| {
            (integer, Some(fraction))
        });
    let mut grouped = String::with_capacity(formatted.len() + integer.len() / 3);
    grouped.push_str(sign);
    for (index, character) in integer.chars().enumerate() {
        if index > 0 && (integer.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(character);
    }
    if let Some(fraction) = fraction {
        grouped.push('.');
        grouped.push_str(fraction);
    }
    grouped
}

fn slot_text(
    world: &World,
    pending: &PendingBuild,
    component: &super::UiComponentSpec,
    slot: UiControlSlot,
) -> String {
    match component.slots.get(&slot) {
        Some(UiControlSlotContent::Text { content }) => render_text(world, pending, content),
        _ => String::new(),
    }
}

fn spawn_plain_text_child(world: &mut World, parent: Entity, text: String) -> Entity {
    let child = world
        .spawn((
            Text::new(text),
            TextFont::default(),
            TextColor(Color::WHITE),
        ))
        .id();
    world.entity_mut(parent).add_child(child);
    child
}

#[derive(Clone, Debug, Component)]
struct UiDocumentRuntimeImage {
    instance_id: UiDocumentInstanceId,
    document_id: UiDocumentId,
    asset_id: UiAssetId,
    presentation: UiImagePresentation,
    tint: UiColor,
    placeholder: Option<UiAssetId>,
    failure: UiImageFailurePresentation,
    entries: BTreeMap<UiAssetId, UiAssetEntry>,
    handles: BTreeMap<UiAssetId, Handle<Image>>,
    state: UiDocumentRuntimeImageState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UiDocumentRuntimeImageState {
    Loading,
    Ready,
    Failed,
}

#[allow(clippy::too_many_arguments)]
fn insert_document_image(
    world: &mut World,
    entity: Entity,
    pending: &PendingBuild,
    instance_id: UiDocumentInstanceId,
    asset_id: &UiAssetId,
    presentation: &UiImagePresentation,
    tint: UiColor,
    placeholder: Option<&UiAssetId>,
    failure: &UiImageFailurePresentation,
) -> Result<(), &'static str> {
    let main = pending
        .assets
        .get(asset_id)
        .ok_or("UI_DOCUMENT_IMAGE_ASSET_MISSING")?;
    let state = if main.ready {
        UiDocumentRuntimeImageState::Ready
    } else if main.failed {
        UiDocumentRuntimeImageState::Failed
    } else {
        UiDocumentRuntimeImageState::Loading
    };
    let entries = pending
        .assets
        .iter()
        .map(|(id, asset)| (id.clone(), asset.entry.clone()))
        .collect::<BTreeMap<_, _>>();
    let handles = pending
        .assets
        .iter()
        .filter_map(|(id, asset)| match asset.handle.as_ref() {
            Some(UiDocumentResolvedAsset::Image(handle)) => Some((id.clone(), handle.clone())),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let runtime_image = UiDocumentRuntimeImage {
        instance_id,
        document_id: pending.request.document_id.clone(),
        asset_id: asset_id.clone(),
        presentation: presentation.clone(),
        tint,
        placeholder: placeholder.cloned(),
        failure: failure.clone(),
        entries,
        handles,
        state,
    };
    apply_runtime_image_presentation(world, entity, &runtime_image)?;
    world.entity_mut(entity).insert(runtime_image);
    Ok(())
}

fn apply_runtime_image_presentation(
    world: &mut World,
    entity: Entity,
    image: &UiDocumentRuntimeImage,
) -> Result<(), &'static str> {
    match image.state {
        UiDocumentRuntimeImageState::Ready => {
            insert_runtime_image_asset(world, entity, image, &image.asset_id)?;
            world.entity_mut(entity).insert(Visibility::Inherited);
        }
        UiDocumentRuntimeImageState::Loading => apply_runtime_image_fallback(
            world,
            entity,
            image,
            resolve_image_fallback(
                image.placeholder.as_ref(),
                &image.failure,
                UiImageContentState::Loading,
            ),
        )?,
        UiDocumentRuntimeImageState::Failed => apply_runtime_image_fallback(
            world,
            entity,
            image,
            resolve_image_fallback(
                image.placeholder.as_ref(),
                &image.failure,
                UiImageContentState::Failed,
            ),
        )?,
    }
    Ok(())
}

fn apply_runtime_image_fallback(
    world: &mut World,
    entity: Entity,
    image: &UiDocumentRuntimeImage,
    fallback: UiResolvedImageFallback<'_>,
) -> Result<(), &'static str> {
    match fallback {
        UiResolvedImageFallback::Asset(asset_id) => {
            insert_runtime_image_asset(world, entity, image, asset_id)?;
            world.entity_mut(entity).insert(Visibility::Inherited);
        }
        UiResolvedImageFallback::Solid(color) => {
            world
                .entity_mut(entity)
                .remove::<ImageNode>()
                .insert((BackgroundColor(ui_color(color)), Visibility::Inherited));
        }
        UiResolvedImageFallback::Hidden => {
            world
                .entity_mut(entity)
                .remove::<ImageNode>()
                .insert((BackgroundColor(Color::NONE), Visibility::Hidden));
        }
    }
    Ok(())
}

fn insert_runtime_image_asset(
    world: &mut World,
    entity: Entity,
    image: &UiDocumentRuntimeImage,
    asset_id: &UiAssetId,
) -> Result<(), &'static str> {
    let handle = image
        .handles
        .get(asset_id)
        .cloned()
        .ok_or("UI_DOCUMENT_IMAGE_HANDLE_MISSING")?;
    let entry = image
        .entries
        .get(asset_id)
        .ok_or("UI_DOCUMENT_IMAGE_ASSET_MISSING")?;
    insert_image_handle(
        world,
        entity,
        entry,
        handle,
        &image.presentation,
        image.tint,
    )
}

fn insert_image(
    world: &mut World,
    entity: Entity,
    pending: &PendingBuild,
    asset_id: &UiAssetId,
    presentation: &UiImagePresentation,
    tint: UiColor,
) -> Result<(), &'static str> {
    let handle = pending
        .assets
        .get(asset_id)
        .and_then(|asset| asset.handle.as_ref())
        .and_then(|handle| match handle {
            UiDocumentResolvedAsset::Image(handle) => Some(handle.clone()),
            _ => None,
        })
        .ok_or("UI_DOCUMENT_IMAGE_HANDLE_MISSING")?;
    let entry = &pending
        .assets
        .get(asset_id)
        .ok_or("UI_DOCUMENT_IMAGE_ASSET_MISSING")?
        .entry;
    insert_image_handle(world, entity, entry, handle, presentation, tint)
}

fn insert_image_handle(
    world: &mut World,
    entity: Entity,
    entry: &UiAssetEntry,
    handle: Handle<Image>,
    presentation: &UiImagePresentation,
    tint: UiColor,
) -> Result<(), &'static str> {
    if let Some(fit) = presentation.to_widget_fit() {
        let (_, mut image, background, widget, status, name) = ui_image(
            handle,
            fit,
            UiImageSize::FixedBox {
                width: 1.0,
                height: 1.0,
            },
        );
        if let UiImagePresentation::AtlasFrame { frame, .. } = presentation
            && let Some(description) = entry.frames.get(frame)
        {
            image = image.with_rect(Rect::from_corners(
                Vec2::new(description.x as f32, description.y as f32),
                Vec2::new(
                    description.x.saturating_add(description.width) as f32,
                    description.y.saturating_add(description.height) as f32,
                ),
            ));
        }
        image.color = ui_color(tint);
        world
            .entity_mut(entity)
            .insert((image, background, widget, status, name));
        return Ok(());
    }
    if let Some(mode) = presentation.to_widget_advanced_mode() {
        let size = entry
            .declared_size
            .ok_or("UI_DOCUMENT_ADVANCED_IMAGE_SIZE_MISSING")?;
        let source_path = match &entry.source {
            UiAssetSource::Packaged { path } => path.clone(),
            UiAssetSource::ContentCache { logical_id } => {
                format!("ui/runtime_cache/{logical_id}.png")
            }
            UiAssetSource::BuiltInMaterial { .. } => {
                return Err("UI_DOCUMENT_ADVANCED_IMAGE_SOURCE_UNAVAILABLE");
            }
        };
        let source = UiAdvancedImageSource::Texture(UiImageTextureSource::new(
            source_path,
            UiImagePixelSize::new(size.width, size.height),
        ));
        let bundle = try_ui_advanced_image_from_handle(
            handle,
            UiAdvancedImageSpec { source, mode },
            UiImageSize::FixedBox {
                width: 1.0,
                height: 1.0,
            },
        )
        .map_err(|_| "UI_DOCUMENT_ADVANCED_IMAGE_ADAPTER_FAILED")?;
        world.entity_mut(entity).insert(bundle);
        if let Some(mut image) = world.get_mut::<ImageNode>(entity) {
            image.color = ui_color(tint);
        }
        return Ok(());
    }
    world
        .entity_mut(entity)
        .insert(ImageNode::new(handle).with_color(ui_color(tint)));
    Ok(())
}

fn ui_color(color: UiColor) -> Color {
    let [red, green, blue, alpha] = color.to_srgba();
    Color::srgba(red, green, blue, alpha)
}

fn multiply_color_alpha(color: Color, opacity: f32) -> Color {
    color.with_alpha(color.to_srgba().alpha * opacity.clamp(0.0, 1.0))
}

fn apply_resolved_style(world: &mut World, entity: Entity, style: &UiResolvedStyle) {
    let opacity = style.properties.opacity.unwrap_or(1.0);
    if let Some(border) = &style.properties.border {
        if let Some(mut node) = world.get_mut::<Node>(entity) {
            node.border = UiRect::all(px(border.width));
        }
        world
            .entity_mut(entity)
            .insert(BorderColor::all(ui_color(border.color)));
    }
    if let Some(radius) = style.properties.corner_radius {
        if let Some(mut node) = world.get_mut::<Node>(entity) {
            node.border_radius = BorderRadius {
                top_left: px(radius[0]),
                top_right: px(radius[1]),
                bottom_right: px(radius[2]),
                bottom_left: px(radius[3]),
            };
        }
    }
    match &style.properties.background {
        Some(UiResolvedBackground::Solid(color)) => {
            world
                .entity_mut(entity)
                .insert(BackgroundColor(ui_color(*color)));
        }
        Some(UiResolvedBackground::LinearGradient {
            angle_degrees,
            stops,
        }) => {
            let stops = stops
                .iter()
                .map(|(position, color)| {
                    ColorStop::percent(
                        multiply_color_alpha(ui_color(*color), opacity),
                        position * 100.0,
                    )
                })
                .collect();
            world
                .entity_mut(entity)
                .insert(BackgroundGradient::from(LinearGradient::new(
                    angle_degrees.rem_euclid(360.0).to_radians(),
                    stops,
                )));
        }
        None => {}
    }
    if style.properties.background.is_none()
        && let Some(material) = &style.properties.material
    {
        match material.parameters {
            UiResolvedMaterialParameters::FrostedPanelV1 {
                opacity: material_opacity,
                tint,
                ..
            } => {
                world
                    .entity_mut(entity)
                    .insert(BackgroundColor(multiply_color_alpha(
                        ui_color(tint),
                        material_opacity,
                    )));
            }
        }
    }
    if let Some(shadows) = &style.properties.shadows {
        world.entity_mut(entity).insert(BoxShadow(
            shadows
                .iter()
                .map(|shadow| ShadowStyle {
                    color: multiply_color_alpha(ui_color(shadow.color), opacity),
                    x_offset: px(shadow.x_offset),
                    y_offset: px(shadow.y_offset),
                    spread_radius: px(shadow.spread),
                    blur_radius: px(shadow.blur),
                })
                .collect(),
        ));
    }
    if let Some(mut background) = world.get_mut::<BackgroundColor>(entity) {
        background.0 = multiply_color_alpha(background.0, opacity);
    }
    if let Some(mut border) = world.get_mut::<BorderColor>(entity) {
        border.top = multiply_color_alpha(border.top, opacity);
        border.right = multiply_color_alpha(border.right, opacity);
        border.bottom = multiply_color_alpha(border.bottom, opacity);
        border.left = multiply_color_alpha(border.left, opacity);
    }
    if let Some(mut text) = world.get_mut::<TextColor>(entity) {
        text.0 = multiply_color_alpha(text.0, opacity);
    }
    if let Some(mut image) = world.get_mut::<ImageNode>(entity) {
        image.color = multiply_color_alpha(image.color, opacity);
    }
}

fn apply_control_state(world: &mut World, entity: Entity, adapter: UiWidgetControlAdapter) {
    world.entity_mut(entity).insert(adapter.flags);
    if adapter.flags.disabled {
        world.entity_mut(entity).insert(DisabledButton);
    }
    if adapter.flags.loading {
        world.entity_mut(entity).insert(LoadingButton);
    }
    if adapter.flags.selected {
        world.entity_mut(entity).insert(SelectedButton);
    }
}

fn reconcile_runtime_documents(
    mut commands: Commands,
    roots: Query<Entity, With<UiDocumentRuntimeRoot>>,
    node_markers: Query<(Entity, &UiDocumentNodeMarker)>,
    mut runtime: ResMut<UiDocumentRuntime>,
    mut bindings: Option<ResMut<UiBindingValues>>,
    mut events: MessageWriter<UiDocumentRuntimeEvent>,
) {
    let present = roots.iter().collect::<BTreeSet<_>>();
    let present_nodes = node_markers
        .iter()
        .map(|(entity, marker)| ((marker.instance_id, marker.node_id.clone()), entity))
        .collect::<BTreeMap<_, _>>();
    let missing = runtime
        .instances
        .iter()
        .filter_map(|(instance_id, active)| {
            let root_missing = !present.contains(&active.index.root);
            let node_missing = active.index.nodes.iter().any(|(node_id, entity)| {
                present_nodes.get(&(*instance_id, node_id.clone())) != Some(entity)
            });
            (root_missing || node_missing).then_some(*instance_id)
        })
        .collect::<Vec<_>>();
    for instance_id in missing {
        if let Some(active) = runtime.instances.get(&instance_id)
            && present.contains(&active.index.root)
        {
            commands.entity(active.index.root).try_despawn();
        }
        cleanup_active_instance(
            instance_id,
            &mut runtime,
            &mut events,
            bindings.as_deref_mut(),
        );
    }
}

fn update_document_bound_texts(
    values: Option<Res<UiBindingValues>>,
    runtime: Res<UiDocumentRuntime>,
    mut texts: Query<(&UiDocumentBoundText, &mut Text)>,
) {
    let Some(values) = values else {
        return;
    };
    for (binding, mut text) in &mut texts {
        let key = DocumentKey {
            owner: binding.owner.clone(),
            document_id: binding.document_id.clone(),
        };
        let next = runtime
            .active
            .get(&key)
            .and_then(|instance_id| runtime.instances.get(instance_id))
            .and_then(|active| active.validated.document().bindings.get(&binding.path))
            .and_then(|declaration| {
                values.scoped_value(
                    binding.document_id.as_str(),
                    &binding.owner,
                    &binding.path,
                    declaration,
                )
            })
            .map(|value| format_binding_value(&value, &binding.format))
            .unwrap_or_else(|| binding.fallback.clone());
        if text.0 != next {
            text.0 = next;
        }
    }
}

fn update_document_control_text_sources(world: &mut World) {
    let progress_sources = {
        let mut query = world.query::<(Entity, &UiDocumentProgressSource)>();
        query
            .iter(world)
            .map(|(entity, source)| (entity, source.clone()))
            .collect::<Vec<_>>()
    };
    for (label_entity, source) in progress_sources {
        let fallback =
            resolve_document_content(world, &source.document_id, &source.owner, &source.content);
        if let Some(mut label) = world.get_mut::<UiProgressLabel>(label_entity) {
            label.set_dynamic_fallback(fallback.clone());
        }
        let display = world
            .get::<UiProgress>(source.control)
            .copied()
            .map(|progress| progress_display_text(progress, fallback));
        if let Some(display) = display
            && let Some(mut text) = world.get_mut::<Text>(label_entity)
            && text.0 != display
        {
            text.0 = display;
        }
    }

    let text_input_sources = {
        let mut query = world.query::<(Entity, &UiDocumentTextInputSource)>();
        query
            .iter(world)
            .map(|(entity, source)| (entity, source.clone()))
            .collect::<Vec<_>>()
    };
    for (_, source) in text_input_sources {
        let next =
            resolve_document_content(world, &source.document_id, &source.owner, &source.content);
        if let Some(mut placeholder) = world.get_mut::<UiTextInputPlaceholder>(source.control) {
            placeholder.0 = next.clone();
        }
        let value_is_empty = world
            .get::<UiTextInputValue>(source.control)
            .is_some_and(|value| value.0.is_empty());
        if value_is_empty
            && let Some(plain_span) = source.plain_span
            && let Some(mut span) = world.get_mut::<TextSpan>(plain_span)
            && span.as_str() != next
        {
            span.0 = next;
        }
    }

    let dropdown_sources = {
        let mut query = world.query::<(Entity, &UiDocumentDropdownSources)>();
        query
            .iter(world)
            .map(|(entity, source)| (entity, source.clone()))
            .collect::<Vec<_>>()
    };
    for (label_entity, source) in dropdown_sources {
        let placeholder = source.placeholder.as_ref().map(|content| {
            resolve_document_content(world, &source.document_id, &source.owner, content)
        });
        let empty = source.empty.as_ref().map(|content| {
            resolve_document_content(world, &source.document_id, &source.owner, content)
        });
        let error = source.error.as_ref().map(|content| {
            resolve_document_content(world, &source.document_id, &source.owner, content)
        });
        let options = source
            .options
            .iter()
            .map(|content| {
                resolve_document_content(world, &source.document_id, &source.owner, content)
            })
            .collect::<Vec<_>>();
        let flags = world
            .get::<UiControlFlags>(source.control)
            .copied()
            .unwrap_or_default();
        let display = if let Some(mut dropdown) = world.get_mut::<UiDropdown>(source.control) {
            if let Some(placeholder) = placeholder {
                dropdown.placeholder = placeholder;
            }
            for (option, label) in dropdown.options.iter_mut().zip(options) {
                option.label = label;
            }
            dropdown.set_document_status_text(empty, error);
            Some(dropdown.display_text(flags))
        } else {
            None
        };
        if let Some(display) = display
            && let Some(mut text) = world.get_mut::<Text>(label_entity)
            && text.0 != display
        {
            text.0 = display;
        }
    }

    let tooltip_sources = {
        let mut query = world.query::<(Entity, &UiDocumentTooltipSource)>();
        query
            .iter(world)
            .map(|(entity, source)| (entity, source.clone()))
            .collect::<Vec<_>>()
    };
    for (entity, source) in tooltip_sources {
        let next =
            resolve_document_content(world, &source.document_id, &source.owner, &source.content);
        if let Some(mut tooltip) = world.get_mut::<UiTooltip>(entity)
            && tooltip.text != next
        {
            tooltip.text = next;
        }
    }
}

fn resolve_document_content(
    world: &World,
    document_id: &UiDocumentId,
    owner: &str,
    content: &UiTextContent,
) -> String {
    match content {
        UiTextContent::Literal(source) => source.literal.clone(),
        UiTextContent::I18n(source) => world
            .get_resource::<UiI18n>()
            .map(|i18n| i18n.tr(source.i18n_key.as_str(), source.fallback.clone()))
            .unwrap_or_else(|| source.fallback.clone()),
        UiTextContent::Binding(source) => {
            let key = DocumentKey {
                owner: owner.to_owned(),
                document_id: document_id.clone(),
            };
            world
                .get_resource::<UiDocumentRuntime>()
                .and_then(|runtime| {
                    runtime
                        .active
                        .get(&key)
                        .and_then(|id| runtime.instances.get(id))
                })
                .and_then(|active| {
                    active
                        .validated
                        .document()
                        .bindings
                        .get(&source.binding_path)
                })
                .and_then(|declaration| {
                    world.get_resource::<UiBindingValues>().and_then(|values| {
                        values.scoped_value(
                            document_id.as_str(),
                            owner,
                            &source.binding_path,
                            declaration,
                        )
                    })
                })
                .map(|value| format_binding_value(&value, &source.format))
                .unwrap_or_else(|| source.fallback.clone())
        }
    }
}

fn enforce_document_control_presentations(world: &mut World) {
    let Some(theme) = world.get_resource::<UiTheme>().cloned() else {
        return;
    };
    let controls = {
        let mut query = world.query::<(
            Entity,
            &UiDocumentControlPresentation,
            &UiDocumentControlStateStyles,
            &UiDocumentControlLayout,
            &UiDocumentControlVisualBaseline,
            &UiDocumentControlCurrentState,
        )>();
        query
            .iter(world)
            .map(
                |(entity, presentation, styles, layout, baseline, current)| {
                    (
                        entity,
                        *presentation,
                        styles.clone(),
                        layout.clone(),
                        baseline.clone(),
                        *current,
                    )
                },
            )
            .collect::<Vec<_>>()
    };
    for (entity, presentation, styles, layout, mut baseline, current) in controls {
        let effective_state = if presentation.state == UiControlState::Normal {
            resolve_control_state(
                world
                    .get::<Interaction>(entity)
                    .copied()
                    .unwrap_or(Interaction::None),
                world.get::<FocusedButton>(entity).is_some(),
                world
                    .get::<UiControlFlags>(entity)
                    .copied()
                    .unwrap_or_default(),
            )
        } else {
            presentation.state
        };
        if current.0 != effective_state {
            baseline.background = world.get::<BackgroundColor>(entity).copied();
            baseline.border = world.get::<BorderColor>(entity).cloned();
            baseline.text_color = world.get::<TextColor>(entity).copied();
            baseline.image_color = world.get::<ImageNode>(entity).map(|image| image.color);
        }
        let style = styles
            .states
            .get(&component_state_from_widget(effective_state))
            .unwrap_or(&styles.base)
            .clone();
        let scale = match presentation.size {
            UiComponentSize::Small => 0.85,
            UiComponentSize::Medium => 1.0,
            UiComponentSize::Large => 1.15,
        };
        apply_document_control_root_style(
            world,
            entity,
            &theme,
            presentation.variant,
            effective_state,
            &layout,
            &baseline,
            &style,
        );
        apply_document_control_text_styles(world, entity, &style, &styles.font_handles, scale);
        sync_document_control_slot_visibility(world, entity, effective_state);
        if let Some(mut marker) = world.get_mut::<UiDocumentResolvedStyleMarker>(entity) {
            marker.0 = style;
        }
        world
            .entity_mut(entity)
            .insert((UiDocumentControlCurrentState(effective_state), baseline));
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_document_control_root_style(
    world: &mut World,
    entity: Entity,
    theme: &UiTheme,
    variant: UiComponentVariant,
    state: UiControlState,
    layout: &UiDocumentControlLayout,
    baseline: &UiDocumentControlVisualBaseline,
    style: &UiResolvedStyle,
) {
    if let Some(mut node) = world.get_mut::<Node>(entity) {
        *node = layout.0.clone();
    }
    world
        .entity_mut(entity)
        .remove::<BackgroundGradient>()
        .remove::<BoxShadow>();

    let opacity = style.properties.opacity.unwrap_or(1.0);
    let (variant_background, variant_border) = control_variant_colors(theme, variant, state);
    let mut background = variant_background
        .map(BackgroundColor)
        .or(baseline.background);
    match &style.properties.background {
        Some(UiResolvedBackground::Solid(color)) => {
            background = Some(BackgroundColor(ui_color(*color)));
        }
        Some(UiResolvedBackground::LinearGradient {
            angle_degrees,
            stops,
        }) => {
            world
                .entity_mut(entity)
                .insert(BackgroundGradient::from(LinearGradient::new(
                    angle_degrees.rem_euclid(360.0).to_radians(),
                    stops
                        .iter()
                        .map(|(position, color)| {
                            ColorStop::percent(
                                multiply_color_alpha(ui_color(*color), opacity),
                                position * 100.0,
                            )
                        })
                        .collect(),
                )));
        }
        None => {
            if let Some(material) = &style.properties.material {
                match material.parameters {
                    UiResolvedMaterialParameters::FrostedPanelV1 {
                        opacity: material_opacity,
                        tint,
                        ..
                    } => {
                        background = Some(BackgroundColor(multiply_color_alpha(
                            ui_color(tint),
                            material_opacity,
                        )));
                    }
                }
            }
        }
    }
    if let Some(mut background) = background {
        background.0 = multiply_color_alpha(background.0, opacity);
        world.entity_mut(entity).insert(background);
    } else {
        world.entity_mut(entity).remove::<BackgroundColor>();
    }

    let border = style
        .properties
        .border
        .as_ref()
        .map(|border| {
            if let Some(mut node) = world.get_mut::<Node>(entity) {
                node.border = UiRect::all(px(border.width));
            }
            BorderColor::all(multiply_color_alpha(ui_color(border.color), opacity))
        })
        .or_else(|| {
            variant_border.map(|color| BorderColor::all(multiply_color_alpha(color, opacity)))
        })
        .or_else(|| {
            baseline.border.clone().map(|mut border| {
                border.top = multiply_color_alpha(border.top, opacity);
                border.right = multiply_color_alpha(border.right, opacity);
                border.bottom = multiply_color_alpha(border.bottom, opacity);
                border.left = multiply_color_alpha(border.left, opacity);
                border
            })
        });
    if let Some(border) = border {
        world.entity_mut(entity).insert(border);
    } else {
        world.entity_mut(entity).remove::<BorderColor>();
    }

    if let Some(radius) = style.properties.corner_radius
        && let Some(mut node) = world.get_mut::<Node>(entity)
    {
        node.border_radius = BorderRadius {
            top_left: px(radius[0]),
            top_right: px(radius[1]),
            bottom_right: px(radius[2]),
            bottom_left: px(radius[3]),
        };
    }
    if let Some(shadows) = &style.properties.shadows {
        world.entity_mut(entity).insert(BoxShadow(
            shadows
                .iter()
                .map(|shadow| ShadowStyle {
                    color: multiply_color_alpha(ui_color(shadow.color), opacity),
                    x_offset: px(shadow.x_offset),
                    y_offset: px(shadow.y_offset),
                    spread_radius: px(shadow.spread),
                    blur_radius: px(shadow.blur),
                })
                .collect(),
        ));
    }
    if let Some(mut text_color) = baseline.text_color {
        text_color.0 = multiply_color_alpha(
            style
                .properties
                .text
                .as_ref()
                .and_then(|text| text.color)
                .map(ui_color)
                .unwrap_or(text_color.0),
            opacity,
        );
        world.entity_mut(entity).insert(text_color);
    }
    if let Some(base_color) = baseline.image_color
        && let Some(mut image) = world.get_mut::<ImageNode>(entity)
    {
        image.color = multiply_color_alpha(base_color, opacity);
    }
}

fn apply_document_control_text_styles(
    world: &mut World,
    root: Entity,
    style: &UiResolvedStyle,
    font_handles: &BTreeMap<UiAssetId, Handle<Font>>,
    size_scale: f32,
) {
    let texts = find_internal_descendants_with::<UiDocumentControlTextBaseline>(world, root)
        .into_iter()
        .filter_map(|entity| {
            world
                .get::<UiDocumentControlTextBaseline>(entity)
                .cloned()
                .map(|baseline| (entity, baseline))
        })
        .collect::<Vec<_>>();
    let visual = style.properties.text.as_ref();
    let opacity = style.properties.opacity.unwrap_or(1.0);
    for (entity, mut baseline) in texts {
        let mut font = baseline.font.clone();
        if let Some(font_id) = visual.and_then(|text| text.font.as_ref())
            && let Some(handle) = font_handles.get(font_id)
        {
            font.font = handle.clone();
        }
        font.font_size = visual
            .and_then(|text| text.font_size)
            .unwrap_or(baseline.font.font_size * size_scale);
        if let Some(weight) = visual.and_then(|text| text.weight) {
            font.weight = match weight {
                super::UiTextWeight::Regular => FontWeight::NORMAL,
                super::UiTextWeight::Medium => FontWeight::MEDIUM,
                super::UiTextWeight::Bold => FontWeight::BOLD,
            };
        }
        let explicit_color = visual.and_then(|text| text.color).map(ui_color);
        let current_color = world.get::<TextColor>(entity).map(|color| color.0);
        if explicit_color.is_none()
            && current_color.is_some()
            && current_color != baseline.last_applied_color
        {
            baseline.color = current_color.expect("current color was checked above");
        }
        let color = multiply_color_alpha(explicit_color.unwrap_or(baseline.color), opacity);
        baseline.last_applied_color = Some(color);
        let line_height = visual
            .and_then(|text| text.line_height)
            .map(LineHeight::Px)
            .or(baseline.line_height);
        world
            .entity_mut(entity)
            .insert((font, TextColor(color), baseline));
        if let Some(line_height) = line_height {
            world.entity_mut(entity).insert(line_height);
        } else {
            world.entity_mut(entity).remove::<LineHeight>();
        }
    }
}

fn sync_document_control_slot_visibility(world: &mut World, root: Entity, state: UiControlState) {
    for entity in find_internal_descendants_with::<UiDocumentControlSlotMarker>(world, root) {
        let Some(slot) = world
            .get::<UiDocumentControlSlotMarker>(entity)
            .map(|marker| marker.0)
        else {
            continue;
        };
        let visible = match slot {
            UiControlSlot::Error => state == UiControlState::Error,
            UiControlSlot::Empty => state == UiControlState::Empty,
            UiControlSlot::Helper => {
                state != UiControlState::Error && state != UiControlState::Empty
            }
            _ => true,
        };
        world.entity_mut(entity).insert(if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

fn update_document_runtime_images(world: &mut World) {
    let runtime_images = {
        let mut query = world.query::<(Entity, &UiDocumentRuntimeImage)>();
        query
            .iter(world)
            .map(|(entity, image)| (entity, image.clone()))
            .collect::<Vec<_>>()
    };
    for (entity, mut image) in runtime_images {
        let before_state = image.state;
        let before_handle = image.handles.get(&image.asset_id).cloned();
        let override_status = world
            .get_resource::<UiDocumentAssetPreflightOverrides>()
            .and_then(|overrides| overrides.get(&image.document_id, &image.asset_id))
            .cloned();
        let resolved = match override_status {
            Some(UiDocumentAssetPreflightStatus::Pending) => None,
            Some(UiDocumentAssetPreflightStatus::Failed { .. }) => {
                image.state = UiDocumentRuntimeImageState::Failed;
                None
            }
            Some(UiDocumentAssetPreflightStatus::Ready {
                asset: UiDocumentResolvedAsset::Image(handle),
            }) => Some(handle),
            Some(UiDocumentAssetPreflightStatus::Ready { .. }) => {
                image.state = UiDocumentRuntimeImageState::Failed;
                None
            }
            None => {
                let handle = image.handles.get(&image.asset_id).cloned();
                match handle.as_ref().and_then(|handle| {
                    world
                        .get_resource::<AssetServer>()
                        .and_then(|server| server.get_load_state(handle.id()))
                }) {
                    Some(LoadState::Loaded) => handle,
                    Some(LoadState::Failed(_)) => {
                        image.state = UiDocumentRuntimeImageState::Failed;
                        None
                    }
                    _ => None,
                }
            }
        };
        if let Some(handle) = resolved {
            let entry = image
                .entries
                .get(&image.asset_id)
                .expect("runtime image entry is retained");
            let metadata = validate_resolved_asset_metadata(
                entry,
                &UiDocumentResolvedAsset::Image(handle.clone()),
                world.get_resource::<Assets<Image>>(),
            );
            match metadata {
                Ok(actual_bytes) => {
                    if !record_instance_asset_bytes(
                        world,
                        image.instance_id,
                        &image.asset_id,
                        actual_bytes.unwrap_or(0),
                    ) {
                        image.state = UiDocumentRuntimeImageState::Failed;
                    } else {
                        image.handles.insert(image.asset_id.clone(), handle);
                        image.state = UiDocumentRuntimeImageState::Ready;
                    }
                }
                Err(_) => {
                    image.state = UiDocumentRuntimeImageState::Failed;
                }
            }
        } else if image.state != UiDocumentRuntimeImageState::Failed {
            image.state = UiDocumentRuntimeImageState::Loading;
        }
        if image.state != UiDocumentRuntimeImageState::Ready {
            remove_instance_asset_bytes(world, image.instance_id, &image.asset_id);
        }
        let handle_changed = before_handle != image.handles.get(&image.asset_id).cloned();
        if (before_state != image.state
            || handle_changed
            || image.state != UiDocumentRuntimeImageState::Ready)
            && apply_runtime_image_presentation(world, entity, &image).is_err()
        {
            image.state = UiDocumentRuntimeImageState::Failed;
            let _ = apply_runtime_image_presentation(world, entity, &image);
        }
        world.entity_mut(entity).insert(image);
    }
}

fn record_instance_asset_bytes(
    world: &mut World,
    instance_id: UiDocumentInstanceId,
    asset_id: &UiAssetId,
    decoded_bytes: u64,
) -> bool {
    let mut runtime = world.resource_mut::<UiDocumentRuntime>();
    let Some(active) = runtime.instances.get_mut(&instance_id) else {
        return false;
    };
    let current = active
        .asset_decoded_bytes
        .get(asset_id)
        .copied()
        .unwrap_or(0);
    let total = active
        .asset_decoded_bytes
        .values()
        .copied()
        .sum::<u64>()
        .saturating_sub(current)
        .saturating_add(decoded_bytes);
    if total > super::UI_ASSET_MAX_TOTAL_DECODED_BYTES {
        return false;
    }
    active
        .asset_decoded_bytes
        .insert(asset_id.clone(), decoded_bytes);
    true
}

fn remove_instance_asset_bytes(
    world: &mut World,
    instance_id: UiDocumentInstanceId,
    asset_id: &UiAssetId,
) {
    if let Some(active) = world
        .resource_mut::<UiDocumentRuntime>()
        .instances
        .get_mut(&instance_id)
    {
        active.asset_decoded_bytes.remove(asset_id);
    }
}

fn enforce_document_generated_text_styles(world: &mut World) {
    let styles = {
        let mut query = world.query::<(Entity, &UiDocumentGeneratedTextStyle)>();
        query
            .iter(world)
            .map(|(entity, style)| (entity, style.clone()))
            .collect::<Vec<_>>()
    };
    for (entity, mut style) in styles {
        apply_document_generated_text_style(world, entity, &mut style);
        world.entity_mut(entity).insert(style);
    }
}

fn apply_document_generated_text_style(
    world: &mut World,
    entity: Entity,
    style: &mut UiDocumentGeneratedTextStyle,
) {
    if style.explicit_color.is_some() || style.opacity != 1.0 {
        let current = world
            .get::<TextColor>(entity)
            .map(|color| color.0)
            .unwrap_or(style.base_color);
        if style.explicit_color.is_none() && style.last_applied_color != Some(current) {
            style.base_color = current;
        }
        let color = multiply_color_alpha(
            style.explicit_color.unwrap_or(style.base_color),
            style.opacity,
        );
        if let Some(mut current) = world.get_mut::<TextColor>(entity) {
            current.0 = color;
        } else {
            world.entity_mut(entity).insert(TextColor(color));
        }
        style.last_applied_color = Some(color);
    }
    if style.font.is_some() || style.font_size.is_some() || style.weight.is_some() {
        if world.get::<TextFont>(entity).is_none() {
            world.entity_mut(entity).insert(TextFont::default());
        }
        let mut current = world
            .get_mut::<TextFont>(entity)
            .expect("generated text font was inserted above");
        if let Some(font) = &style.font {
            current.font = font.clone();
        }
        if let Some(font_size) = style.font_size {
            current.font_size = font_size;
        }
        if let Some(weight) = style.weight {
            current.weight = weight;
        }
    }
    if let Some(line_height) = style.line_height {
        world.entity_mut(entity).insert(line_height);
    }
}

fn enforce_document_text_constraints(
    mut texts: Query<(&UiDocumentTextConstraint, &mut Text, &mut Node)>,
) {
    for (constraint, mut text, node) in &mut texts {
        apply_text_node_constraint(
            node,
            constraint.max_lines,
            constraint.overflow,
            constraint.line_height_px,
        );
        let constrained =
            constrain_explicit_lines(&text.0, constraint.max_lines, constraint.overflow);
        if text.0 != constrained {
            text.0 = constrained;
        }
    }
}

fn dispatch_document_actions(
    mut button_events: MessageReader<UiButtonEvent>,
    markers: Query<(Entity, &UiDocumentActionMarker)>,
    runtime: Res<UiDocumentRuntime>,
    registry: Res<UiActionRegistry>,
    mut binding_values: ResMut<UiBindingValues>,
    mut dispatches: MessageWriter<UiActionDispatch>,
    mut rejected: MessageWriter<UiActionRejected>,
) {
    for event in button_events.read() {
        if event.kind != UiButtonEventKind::Click {
            continue;
        }
        let Ok((entity, marker)) = markers.get(event.entity) else {
            continue;
        };
        let Some(active) = runtime.instances.get(&marker.instance_id) else {
            continue;
        };
        if active.index.nodes.get(&marker.node_id) != Some(&entity) {
            continue;
        }
        match registry.dispatch(
            &active.validated,
            &UiActionDispatchContext {
                owner: active.index.owner.clone(),
                owner_alive: true,
                source_node: marker.node_id.clone(),
            },
        ) {
            Ok(dispatch) => {
                apply_local_state_dispatch(active, &dispatch, &mut binding_values);
                dispatches.write(dispatch);
            }
            Err(error) => {
                rejected.write(UiActionRejected {
                    action: marker.action_id.clone(),
                    error,
                });
            }
        }
    }
}

fn apply_local_state_dispatch(
    active: &ActiveDocument,
    dispatch: &UiActionDispatch,
    binding_values: &mut UiBindingValues,
) -> bool {
    let UiRegisteredActionKind::UpdateLocalState {
        binding,
        value_param,
    } = &dispatch.kind
    else {
        return false;
    };
    if dispatch.document_id != active.index.document_id || dispatch.owner != active.index.owner {
        return false;
    }
    let Some(declaration) = active.validated.document().bindings.get(binding) else {
        return false;
    };
    if declaration.scope != UiBindingScope::Local {
        return false;
    }
    let Some(UiActionValue::Binding(value)) = dispatch.params.get(value_param) else {
        return false;
    };
    binding_values.set_scoped(
        active.index.document_id.as_str(),
        &active.index.owner,
        binding,
        declaration,
        value.clone(),
    )
}

fn close_key(
    key: DocumentKey,
    commands: &mut Commands,
    runtime: &mut UiDocumentRuntime,
    events: &mut MessageWriter<UiDocumentRuntimeEvent>,
    bindings: Option<&mut UiBindingValues>,
) {
    let pending = runtime
        .pending
        .iter()
        .filter_map(|(id, pending)| (pending.key == key).then_some(*id))
        .collect::<Vec<_>>();
    for request_id in pending {
        cancel_pending(request_id, "UI_DOCUMENT_BUILD_CLOSED", runtime, events);
    }
    if let Some(instance_id) = runtime.active.get(&key).copied() {
        if let Some(active) = runtime.instances.get(&instance_id) {
            commands.entity(active.index.root).try_despawn();
        }
        cleanup_active_instance(instance_id, runtime, events, bindings);
    }
}

fn close_panel(
    owner: &str,
    panel: UiDocumentPanel,
    commands: &mut Commands,
    runtime: &mut UiDocumentRuntime,
    events: &mut MessageWriter<UiDocumentRuntimeEvent>,
    mut bindings: Option<&mut UiBindingValues>,
) {
    let pending = runtime
        .pending
        .iter()
        .filter_map(|(id, pending)| {
            (pending.request.owner == owner && pending.request.panel == panel).then_some(*id)
        })
        .collect::<Vec<_>>();
    for request_id in pending {
        cancel_pending(
            request_id,
            "UI_DOCUMENT_BUILD_PANEL_CLOSED",
            runtime,
            events,
        );
    }
    let instances = runtime
        .instances
        .iter()
        .filter_map(|(id, active)| {
            (active.index.owner == owner && active.index.panel == panel).then_some(*id)
        })
        .collect::<Vec<_>>();
    for instance_id in instances {
        if let Some(active) = runtime.instances.get(&instance_id) {
            commands.entity(active.index.root).try_despawn();
        }
        cleanup_active_instance(instance_id, runtime, events, bindings.as_deref_mut());
    }
}

fn close_owner(
    owner: &str,
    commands: &mut Commands,
    runtime: &mut UiDocumentRuntime,
    events: &mut MessageWriter<UiDocumentRuntimeEvent>,
    mut bindings: Option<&mut UiBindingValues>,
) {
    let pending = runtime
        .pending
        .iter()
        .filter_map(|(id, pending)| (pending.request.owner == owner).then_some(*id))
        .collect::<Vec<_>>();
    for request_id in pending {
        cancel_pending(
            request_id,
            "UI_DOCUMENT_BUILD_OWNER_DESTROYED",
            runtime,
            events,
        );
    }
    let instances = runtime
        .instances
        .iter()
        .filter_map(|(id, active)| (active.index.owner == owner).then_some(*id))
        .collect::<Vec<_>>();
    let documents = instances
        .iter()
        .filter_map(|id| runtime.instances.get(id))
        .map(|active| active.index.document_id.clone())
        .collect::<BTreeSet<_>>();
    for instance_id in instances {
        if let Some(active) = runtime.instances.get(&instance_id) {
            commands.entity(active.index.root).try_despawn();
        }
        cleanup_active_instance(instance_id, runtime, events, None);
    }
    if let Some(bindings) = bindings.as_deref_mut() {
        bindings.clear_owner(owner);
        for document_id in documents {
            clear_document_scope_if_unused(runtime, bindings, &document_id);
        }
    }
}

fn cleanup_replaced_instance(world: &mut World, instance_id: UiDocumentInstanceId) {
    let Some(old) = world
        .resource_mut::<UiDocumentRuntime>()
        .instances
        .remove(&instance_id)
    else {
        return;
    };
    let key = DocumentKey {
        owner: old.index.owner.clone(),
        document_id: old.index.document_id.clone(),
    };
    if world.resource::<UiDocumentRuntime>().active.get(&key) == Some(&instance_id) {
        world
            .resource_mut::<UiDocumentRuntime>()
            .active
            .remove(&key);
    }
    if world.get_entity(old.index.root).is_ok() {
        world.entity_mut(old.index.root).despawn();
    }
    write_record(
        world,
        UiDocumentBuildRecord {
            request_id: old.index.request_id,
            instance_id: Some(instance_id),
            document_id: old.index.document_id,
            owner: old.index.owner,
            generation: old.index.generation,
            state: UiDocumentBuildState::Cleaned,
            elapsed_micros: 0,
            protocol_node_count: old.index.nodes.len(),
            ecs_entity_count: old.index.ecs_entity_count,
            asset_count: old.validated.document().assets.len(),
            failure_stage: Some(UiDocumentFailureStage::Cleanup),
            failure_code: Some("UI_DOCUMENT_INSTANCE_REPLACED".to_owned()),
        },
    );
}

fn cleanup_active_instance(
    instance_id: UiDocumentInstanceId,
    runtime: &mut UiDocumentRuntime,
    events: &mut MessageWriter<UiDocumentRuntimeEvent>,
    bindings: Option<&mut UiBindingValues>,
) {
    let Some(active) = runtime.instances.remove(&instance_id) else {
        return;
    };
    let key = DocumentKey {
        owner: active.index.owner.clone(),
        document_id: active.index.document_id.clone(),
    };
    if runtime.active.get(&key) == Some(&instance_id) {
        runtime.active.remove(&key);
    }
    if let Some(bindings) = bindings {
        bindings.clear_instance(active.index.document_id.as_str(), &active.index.owner);
        clear_document_scope_if_unused(runtime, bindings, &active.index.document_id);
    }
    let record = UiDocumentBuildRecord {
        request_id: active.index.request_id,
        instance_id: Some(instance_id),
        document_id: active.index.document_id,
        owner: active.index.owner,
        generation: active.index.generation,
        state: UiDocumentBuildState::Cleaned,
        elapsed_micros: 0,
        protocol_node_count: active.index.nodes.len(),
        ecs_entity_count: active.index.ecs_entity_count,
        asset_count: active.validated.document().assets.len(),
        failure_stage: Some(UiDocumentFailureStage::Cleanup),
        failure_code: None,
    };
    emit_record(runtime, events, record);
}

fn clear_document_scope_if_unused(
    runtime: &UiDocumentRuntime,
    bindings: &mut UiBindingValues,
    document_id: &UiDocumentId,
) {
    if runtime
        .instances
        .values()
        .all(|active| active.index.document_id != *document_id)
        && runtime
            .pending
            .values()
            .all(|pending| pending.request.document_id != *document_id)
    {
        bindings.clear_document(document_id.as_str());
    }
}

fn cancel_pending(
    request_id: UiDocumentRequestId,
    code: &str,
    runtime: &mut UiDocumentRuntime,
    events: &mut MessageWriter<UiDocumentRuntimeEvent>,
) {
    let Some(pending) = runtime.pending.remove(&request_id) else {
        return;
    };
    let record = record_for_pending(&pending, UiDocumentBuildState::Cancelled)
        .failed(UiDocumentFailureStage::Cancel, code);
    emit_record(runtime, events, record);
}

fn count_entity_tree(world: &World, root: Entity) -> usize {
    let mut count = 0;
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        count += 1;
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter());
        }
    }
    count
}

fn base_record(
    request: &UiDocumentOpenRequest,
    generation: u64,
    state: UiDocumentBuildState,
    protocol_node_count: usize,
    asset_count: usize,
) -> UiDocumentBuildRecord {
    UiDocumentBuildRecord {
        request_id: request.request_id,
        instance_id: None,
        document_id: request.document_id.clone(),
        owner: request.owner.clone(),
        generation,
        state,
        elapsed_micros: 0,
        protocol_node_count,
        ecs_entity_count: 0,
        asset_count,
        failure_stage: None,
        failure_code: None,
    }
}

fn record_for_pending(
    pending: &PendingBuild,
    state: UiDocumentBuildState,
) -> UiDocumentBuildRecord {
    let elapsed = pending
        .started
        .elapsed()
        .as_micros()
        .min(u128::from(u64::MAX)) as u64;
    let mut record = base_record(
        &pending.request,
        pending.generation,
        state,
        pending.prepared.node_count,
        pending.assets.len(),
    );
    record.elapsed_micros = elapsed;
    record
}

impl UiDocumentBuildRecord {
    fn failed(mut self, stage: UiDocumentFailureStage, code: impl Into<String>) -> Self {
        self.failure_stage = Some(stage);
        self.failure_code = Some(code.into());
        self
    }
}

fn emit_record(
    runtime: &mut UiDocumentRuntime,
    events: &mut MessageWriter<UiDocumentRuntimeEvent>,
    record: UiDocumentBuildRecord,
) {
    runtime.store_record(record.clone());
    events.write(UiDocumentRuntimeEvent(record));
}

fn write_record(world: &mut World, record: UiDocumentBuildRecord) {
    world
        .resource_mut::<UiDocumentRuntime>()
        .store_record(record.clone());
    world.write_message(UiDocumentRuntimeEvent(record));
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use bevy::{
        asset::{AssetPlugin, RenderAssetUsages},
        render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    };

    use super::*;
    use crate::framework::ui::{
        core::{UiCurrentOwner, UiMetrics},
        document::{
            UiActionDescriptor, UiActionParamSchema, UiActionParamType, UiBindingValue,
            UiDocumentInputMode, UiDocumentPlatform, UiSafeAreaClass,
        },
        widgets::{
            UiScrollView,
            controls::{
                UiBadgeLabel, UiCheckbox, UiProgressFill, UiProgressLabel, UiSegmentIndicator,
                UiSliderFill, UiSliderTrack, UiSliderValueText, UiStepperValueText, UiTabIndicator,
                UiTabList, UiToggle, update_selection_control_interactions,
                update_stepper_interactions,
            },
        },
    };

    const DOCUMENT_ID: &str = "runtime.transaction";
    const SIMPLE_DOCUMENT: &str = r#"
    {
      "schema_version": 1,
      "document_id": "runtime.transaction",
      "root": {
        "type": "container",
        "id": "runtime.root",
        "children": [
          {
            "type": "text",
            "id": "runtime.title",
            "content": { "literal": "Ready" }
          }
        ]
      }
    }
    "#;
    const ASSET_DOCUMENT: &str = r#"
    {
      "schema_version": 1,
      "document_id": "runtime.transaction",
      "assets": {
        "hero": {
          "kind": "image",
          "source": {
            "kind": "packaged",
            "path": "ui/runtime/hero.png"
          }
        }
      },
      "root": {
        "type": "container",
        "id": "runtime.root",
        "children": [
          {
            "type": "text",
            "id": "runtime.title",
            "content": { "literal": "Replacement" }
          }
        ]
      }
    }
    "#;
    const ACTION_DOCUMENT: &str = r#"
    {
      "schema_version": 1,
      "document_id": "runtime.transaction",
      "root": {
        "type": "button",
        "id": "runtime.button",
        "label": { "literal": "Continue" },
        "on_click": { "action": "runtime.continue" }
      }
    }
    "#;
    const BINDING_DOCUMENT: &str = r#"
    {
      "schema_version": 1,
      "document_id": "runtime.transaction",
      "bindings": {
        "state.local": {
          "scope": "local",
          "value_type": { "kind": "string" }
        },
        "state.shared": {
          "scope": "document",
          "value_type": { "kind": "number" }
        }
      },
      "root": {
        "type": "text",
        "id": "runtime.binding_text",
        "content": {
          "binding_path": "state.local",
          "fallback": "Missing"
        }
      }
    }
    "#;
    const I18N_MISSING_DOCUMENT: &str = r#"
    {
      "schema_version": 1,
      "document_id": "runtime.transaction",
      "root": {
        "type": "text",
        "id": "runtime.missing_i18n",
        "content": {
          "i18n_key": "runtime.catalog_missing",
          "fallback": "Fallback"
        }
      }
    }
    "#;
    const CONSTRAINED_TEXT_DOCUMENT: &str = r#"
    {
      "schema_version": 1,
      "document_id": "runtime.transaction",
      "root": {
        "type": "text",
        "id": "runtime.constrained_text",
        "content": { "literal": "one\ntwo\nthree" },
        "typography": {
          "line_height": { "kind": "pixels", "value": 20.0 },
          "max_lines": 2,
          "overflow": "ellipsis"
        }
      }
    }
    "#;
    const FULL_CONTROL_DOCUMENT: &str = r#"
    {
      "schema_version": 1,
      "document_id": "runtime.transaction",
      "assets": {
        "control_image": {
          "kind": "image",
          "source": { "kind": "packaged", "path": "ui/runtime/control.png" }
        }
      },
      "root": {
        "type": "container",
        "id": "controls.root",
        "children": [
          {
            "type": "button",
            "id": "controls.button",
            "label": { "literal": "Button" },
            "on_click": { "action": "runtime.continue" }
          },
          {
            "type": "text_input",
            "id": "controls.text_input",
            "component": { "slots": {
              "label": { "kind": "text", "content": { "literal": "Input" } },
              "placeholder": { "kind": "text", "content": { "literal": "Type" } }
            } }
          },
          {
            "type": "checkbox",
            "id": "controls.checkbox",
            "component": { "slots": { "label": { "kind": "text", "content": { "literal": "Check" } } } }
          },
          {
            "type": "toggle",
            "id": "controls.toggle",
            "component": { "slots": { "label": { "kind": "text", "content": { "literal": "Toggle" } } } }
          },
          {
            "type": "segmented",
            "id": "controls.segmented",
            "component": { "slots": { "label": { "kind": "text", "content": { "literal": "Segment" } } } },
            "options": [
              { "value": "one", "label": { "literal": "One" } },
              { "value": "two", "label": { "literal": "Two" } }
            ],
            "selected": "one"
          },
          {
            "type": "slider",
            "id": "controls.slider",
            "value": 0.4,
            "min": 0.0,
            "max": 1.0,
            "component": { "slots": { "label": { "kind": "text", "content": { "literal": "Slider" } } } }
          },
          {
            "type": "stepper",
            "id": "controls.stepper",
            "value": 2,
            "min": 0,
            "max": 8,
            "step": 2,
            "component": { "slots": { "label": { "kind": "text", "content": { "literal": "Stepper" } } } }
          },
          { "type": "scroll", "id": "controls.scroll", "row_gap": 4.0 },
          {
            "type": "modal",
            "id": "controls.modal",
            "component": { "slots": {
              "title": { "kind": "text", "content": { "literal": "Title" } },
              "body": { "kind": "text", "content": { "literal": "Body" } }
            } }
          },
          {
            "type": "image_button",
            "id": "controls.image_button",
            "asset": "control_image",
            "component": { "slots": { "label": { "kind": "text", "content": { "literal": "Image" } } } }
          },
          {
            "type": "badge",
            "id": "controls.badge",
            "component": { "slots": { "label": { "kind": "text", "content": { "literal": "Badge" } } } }
          },
          {
            "type": "progress",
            "id": "controls.progress",
            "value": 0.6,
            "component": { "slots": { "label": { "kind": "text", "content": { "literal": "Progress" } } } }
          },
          {
            "type": "tab",
            "id": "controls.tab",
            "value": "overview",
            "component": { "slots": { "label": { "kind": "text", "content": { "literal": "Overview" } } } }
          },
          {
            "type": "tooltip",
            "id": "controls.tooltip",
            "component": {
              "slots": { "body": { "kind": "text", "content": { "literal": "Hint" } } },
              "children": [ { "type": "spacer", "id": "controls.tooltip_target" } ]
            }
          },
          {
            "type": "select",
            "id": "controls.select",
            "component": { "slots": {
              "label": { "kind": "text", "content": { "literal": "Choice" } },
              "placeholder": { "kind": "text", "content": { "literal": "Select" } }
            } },
            "options": [
              { "value": "a", "label": { "literal": "Alpha" } },
              { "value": "b", "label": { "literal": "Beta" } }
            ]
          }
        ]
      }
    }
    "#;
    const DYNAMIC_CONTROL_TEXT_DOCUMENT: &str = r##"
    {
      "schema_version": 1,
      "document_id": "runtime.transaction",
      "assets": {
        "label_font": {
          "kind": "font",
          "source": { "kind": "packaged", "path": "ui/runtime/label.ttf" }
        }
      },
      "bindings": {
        "state.label": {
          "scope": "local",
          "value_type": { "kind": "string" },
          "default": { "kind": "string", "value": "Initial binding" },
          "missing": "use_default"
        }
      },
      "root": {
        "type": "container",
        "id": "dynamic.root",
        "children": [
          {
            "type": "button",
            "id": "dynamic.button",
            "label": { "i18n_key": "runtime.dynamic_button", "fallback": "Button fallback" },
            "on_click": { "action": "runtime.continue" },
            "style": { "inline": {
              "text": {
                "color": { "kind": "literal", "value": "#336699ff" },
                "font": "label_font",
                "font_size": { "kind": "literal", "value": 23.0 }
              },
              "opacity": { "kind": "literal", "value": 0.5 }
            } }
          },
          {
            "type": "checkbox",
            "id": "dynamic.checkbox",
            "component": { "slots": { "label": { "kind": "text", "content": {
              "binding_path": "state.label",
              "fallback": "Binding fallback"
            } } } }
          },
          {
            "type": "text_input",
            "id": "dynamic.text_input",
            "component": { "slots": {
              "label": { "kind": "text", "content": { "literal": "Input" } },
              "placeholder": { "kind": "text", "content": {
                "i18n_key": "runtime.dynamic_placeholder",
                "fallback": "Placeholder fallback"
              } }
            } }
          },
          {
            "type": "select",
            "id": "dynamic.select",
            "component": { "slots": {
              "label": { "kind": "text", "content": { "literal": "Choice" } },
              "placeholder": { "kind": "text", "content": {
                "i18n_key": "runtime.dynamic_select",
                "fallback": "Select fallback"
              } }
            } },
            "options": [
              { "value": "a", "label": {
                "i18n_key": "runtime.dynamic_option",
                "fallback": "Option fallback"
              } }
            ]
          },
          {
            "type": "tooltip",
            "id": "dynamic.tooltip",
            "component": {
              "slots": { "body": { "kind": "text", "content": {
                "i18n_key": "runtime.dynamic_tooltip",
                "fallback": "Tooltip fallback"
              } } },
              "children": [ { "type": "spacer", "id": "dynamic.tooltip_target" } ]
            }
          }
        ]
      }
    }
    "##;
    const LOCAL_ACTION_DOCUMENT: &str = r#"
    {
      "schema_version": 1,
      "document_id": "runtime.transaction",
      "bindings": {
        "state.label": {
          "scope": "local",
          "value_type": { "kind": "string" },
          "default": { "kind": "string", "value": "Before" },
          "missing": "use_default"
        }
      },
      "root": {
        "type": "container",
        "id": "local.root",
        "children": [
          {
            "type": "text",
            "id": "local.text",
            "content": { "binding_path": "state.label", "fallback": "Missing" }
          },
          {
            "type": "button",
            "id": "local.button",
            "label": { "literal": "Update" },
            "on_click": {
              "action": "runtime.update_local",
              "params": {
                "value": {
                  "kind": "binding",
                  "value": { "kind": "string", "value": "After" }
                }
              }
            }
          }
        ]
      }
    }
    "#;
    const CONTROL_FIELD_MAPPING_DOCUMENT: &str = r##"
    {
      "schema_version": 1,
      "document_id": "runtime.transaction",
      "assets": {
        "control_icon": {
          "kind": "icon",
          "source": { "kind": "packaged", "path": "ui/runtime/control.png" }
        }
      },
      "root": {
        "type": "container",
        "id": "mapping.root",
        "children": [
          {
            "type": "button",
            "id": "mapping.button",
            "component": {
              "variant": "destructive",
              "size": "large",
              "states": ["pressed"],
              "state_overrides": {
                "pressed": { "inline": {
                  "background": { "kind": "solid", "color": { "kind": "literal", "value": "#112233ff" } },
                  "text": {
                    "weight": "bold",
                    "line_height": { "kind": "literal", "value": 29.0 }
                  },
                  "opacity": { "kind": "literal", "value": 0.5 }
                } }
              },
              "slots": {
                "label": { "kind": "text", "content": { "literal": "Mapped button" } },
                "leading": { "kind": "icon", "asset": "control_icon" },
                "trailing": { "kind": "icon", "asset": "control_icon" }
              }
            },
            "on_click": { "action": "runtime.continue" },
            "layout": { "min_height": { "px": 91.0 } }
          },
          {
            "type": "text_input",
            "id": "mapping.input",
            "component": {
              "size": "small",
              "states": ["error"],
              "slots": {
                "label": { "kind": "text", "content": { "literal": "Input label" } },
                "placeholder": { "kind": "text", "content": { "literal": "Input placeholder" } },
                "helper": { "kind": "text", "content": { "literal": "Input helper" } },
                "error": { "kind": "text", "content": { "literal": "Input error" } }
              }
            }
          },
          {
            "type": "slider",
            "id": "mapping.slider",
            "component": {
              "states": ["error"],
              "slots": {
                "label": { "kind": "text", "content": { "literal": "Slider" } },
                "helper": { "kind": "text", "content": { "literal": "Slider helper" } },
                "error": { "kind": "text", "content": { "literal": "Slider error" } }
              }
            }
          },
          {
            "type": "modal",
            "id": "mapping.modal",
            "component": {
              "states": ["error"],
              "slots": {
                "title": { "kind": "text", "content": { "literal": "Modal title" } },
                "body": { "kind": "text", "content": { "literal": "Modal body" } },
                "error": { "kind": "text", "content": { "literal": "Modal error" } }
              }
            }
          },
          {
            "type": "tab",
            "id": "mapping.tab",
            "value": "details",
            "component": {
              "variant": "subtle",
              "size": "large",
              "slots": {
                "label": { "kind": "text", "content": { "literal": "Details" } },
                "leading": { "kind": "icon", "asset": "control_icon" }
              }
            }
          },
          {
            "type": "select",
            "id": "mapping.select",
            "component": {
              "states": ["empty"],
              "slots": {
                "label": { "kind": "text", "content": { "literal": "Select label" } },
                "placeholder": { "kind": "text", "content": { "literal": "Select placeholder" } },
                "empty": { "kind": "text", "content": { "literal": "Nothing available" } },
                "error": { "kind": "text", "content": { "literal": "Select error" } }
              }
            },
            "options": [ { "value": "a", "label": { "literal": "Alpha" } } ]
          }
        ]
      }
    }
    "##;
    const DYNAMIC_CONTROL_STATE_DOCUMENT: &str = r##"
    {
      "schema_version": 1,
      "document_id": "runtime.transaction",
      "root": {
        "type": "container",
        "id": "state.root",
        "children": [
          {
            "type": "button",
            "id": "state.button",
            "component": {
              "variant": "secondary",
              "size": "large",
              "state_overrides": {
                "hovered": { "inline": {
                  "background": { "kind": "solid", "color": { "kind": "literal", "value": "#ff0000ff" } },
                  "border": {
                    "width": { "kind": "literal", "value": 3.0 },
                    "color": { "kind": "literal", "value": "#abcdef80" }
                  },
                  "corner_radius": {
                    "top_left": { "kind": "literal", "value": 12.0 },
                    "top_right": { "kind": "literal", "value": 12.0 },
                    "bottom_right": { "kind": "literal", "value": 12.0 },
                    "bottom_left": { "kind": "literal", "value": 12.0 }
                  },
                  "text": {
                    "color": { "kind": "literal", "value": "#00ff00ff" },
                    "font_size": { "kind": "literal", "value": 30.0 },
                    "line_height": { "kind": "literal", "value": 36.0 },
                    "weight": "bold"
                  },
                  "opacity": { "kind": "literal", "value": 0.5 },
                  "shadows": [{
                    "color": { "kind": "literal", "value": "#00000080" },
                    "x_offset": { "kind": "literal", "value": 1.0 },
                    "y_offset": { "kind": "literal", "value": 2.0 },
                    "blur": { "kind": "literal", "value": 4.0 },
                    "spread": { "kind": "literal", "value": 0.0 }
                  }]
                } },
                "pressed": { "inline": {
                  "background": { "kind": "solid", "color": { "kind": "literal", "value": "#0000ffff" } },
                  "text": {
                    "color": { "kind": "literal", "value": "#ffffffff" },
                    "font_size": { "kind": "literal", "value": 26.0 },
                    "weight": "medium"
                  },
                  "opacity": { "kind": "literal", "value": 0.75 }
                } }
              },
              "slots": {
                "label": { "kind": "text", "content": { "literal": "Dynamic" } }
              }
            },
            "on_click": { "action": "runtime.continue" },
            "layout": {
              "padding": {
                "all": { "px": 1.0 },
                "left": { "px": 11.0 },
                "right": { "px": 13.0 },
                "top": { "px": 17.0 },
                "bottom": { "px": 19.0 }
              }
            },
            "style": { "inline": {
              "background": { "kind": "solid", "color": { "kind": "literal", "value": "#102030ff" } },
              "corner_radius": {
                "top_left": { "kind": "literal", "value": 4.0 },
                "top_right": { "kind": "literal", "value": 4.0 },
                "bottom_right": { "kind": "literal", "value": 4.0 },
                "bottom_left": { "kind": "literal", "value": 4.0 }
              },
              "text": { "color": { "kind": "literal", "value": "#e0e0e0ff" } }
            } }
          },
          {
            "type": "text_input",
            "id": "state.input",
            "component": {
              "size": "small",
              "state_overrides": {
                "error": { "inline": {
                  "background": { "kind": "solid", "color": { "kind": "literal", "value": "#aa5500ff" } },
                  "text": {
                    "color": { "kind": "literal", "value": "#ffaaaaee" },
                    "font_size": { "kind": "literal", "value": 25.0 },
                    "weight": "bold"
                  },
                  "opacity": { "kind": "literal", "value": 0.5 }
                } }
              },
              "slots": {
                "label": { "kind": "text", "content": { "literal": "Input" } },
                "placeholder": { "kind": "text", "content": { "literal": "Value" } },
                "helper": { "kind": "text", "content": { "literal": "Helper" } },
                "error": { "kind": "text", "content": { "literal": "Error" } }
              }
            }
          },
          {
            "type": "scroll",
            "id": "state.scroll",
            "component": {
              "size": "large",
              "slots": {
                "empty": { "kind": "text", "content": { "literal": "Empty" } },
                "error": { "kind": "text", "content": { "literal": "Scroll error" } }
              },
              "children": [{
                "type": "text",
                "id": "state.nested_text",
                "content": { "literal": "Nested protocol text" },
                "style": { "inline": {
                  "text": { "font_size": { "kind": "literal", "value": 37.0 } }
                } }
              }]
            }
          }
        ]
      }
    }
    "##;
    const DUPLICATE_LATE_IMAGE_DOCUMENT: &str = r##"
    {
      "schema_version": 1,
      "document_id": "runtime.transaction",
      "assets": {
        "main_image": {
          "kind": "image",
          "source": { "kind": "packaged", "path": "ui/runtime/main.png" }
        },
        "fallback_image": {
          "kind": "image",
          "source": { "kind": "packaged", "path": "ui/runtime/fallback.png" }
        }
      },
      "root": {
        "type": "container",
        "id": "duplicate.root",
        "children": [
          {
            "type": "image",
            "id": "duplicate.first",
            "asset": "main_image",
            "placeholder": "fallback_image",
            "failure": { "kind": "placeholder" }
          },
          {
            "type": "image",
            "id": "duplicate.second",
            "asset": "main_image",
            "placeholder": "fallback_image",
            "failure": { "kind": "placeholder" }
          }
        ]
      }
    }
    "##;
    const LATE_IMAGE_TOTAL_BUDGET_DOCUMENT: &str = r##"
    {
      "schema_version": 1,
      "document_id": "runtime.transaction",
      "assets": {
        "main_image": {
          "kind": "image",
          "source": { "kind": "packaged", "path": "ui/runtime/main.png" }
        },
        "required_0": {
          "kind": "image",
          "source": { "kind": "packaged", "path": "ui/runtime/required-0.png" }
        },
        "required_1": {
          "kind": "icon",
          "source": { "kind": "packaged", "path": "ui/runtime/required-1.png" }
        },
        "required_2": {
          "kind": "icon",
          "source": { "kind": "packaged", "path": "ui/runtime/required-2.png" }
        },
        "required_3": {
          "kind": "icon",
          "source": { "kind": "packaged", "path": "ui/runtime/required-3.png" }
        }
      },
      "root": {
        "type": "container",
        "id": "budget.root",
        "children": [
          {
            "type": "image",
            "id": "budget.image",
            "asset": "main_image",
            "placeholder": "required_0",
            "failure": { "kind": "error_color", "color": "#123456ff" }
          },
          {
            "type": "button",
            "id": "budget.button",
            "component": { "slots": {
              "label": { "kind": "text", "content": { "literal": "Budget" } },
              "leading": { "kind": "icon", "asset": "required_1" },
              "trailing": { "kind": "icon", "asset": "required_2" }
            } },
            "on_click": { "action": "runtime.continue" }
          },
          {
            "type": "tab",
            "id": "budget.tab",
            "value": "budget",
            "component": { "slots": {
              "label": { "kind": "text", "content": { "literal": "Tab" } },
              "leading": { "kind": "icon", "asset": "required_3" }
            } }
          }
        ]
      }
    }
    "##;

    fn test_app() -> App {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>();
        app.insert_resource(UiI18n::test_with_texts(
            "runtime_test",
            &[("runtime.ready", "Ready")],
        ));
        app.add_plugins(UiDocumentRuntimePlugin);
        app
    }

    fn styled_test_app(i18n_entries: &[(&str, &str)]) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Image>();
        app.insert_resource(UiI18n::test_with_texts("runtime_test", i18n_entries));
        app.insert_resource(UiTheme::default());
        app.insert_resource(UiMetrics::default());
        app.insert_resource(UiFontAssets::test_registry());
        app.configure_sets(Update, UiI18nSystems::Refresh)
            .add_systems(
                Update,
                refresh_test_i18n_texts.in_set(UiI18nSystems::Refresh),
            );
        app.add_plugins(UiDocumentRuntimePlugin);
        app
    }

    fn refresh_test_i18n_texts(i18n: Res<UiI18n>, mut texts: Query<(&UiI18nText, &mut Text)>) {
        for (source, mut text) in &mut texts {
            text.0 = i18n.tr(&source.key, source.fallback.clone());
        }
    }

    fn register_route_action(app: &mut App) {
        app.world_mut()
            .resource_mut::<UiActionRegistry>()
            .register(UiActionDescriptor::new(
                UiActionId::from_str("runtime.continue").unwrap(),
                document_id(),
                "runtime_owner",
                UiRegisteredActionKind::Route {
                    target: "game.runtime_continue".to_owned(),
                },
            ))
            .unwrap();
    }

    fn document_id() -> UiDocumentId {
        UiDocumentId::from_str(DOCUMENT_ID).unwrap()
    }

    fn asset_id() -> UiAssetId {
        UiAssetId::from_str("hero").unwrap()
    }

    fn request(id: u64, source: &str) -> UiDocumentOpenRequest {
        request_for_owner(id, source, "runtime_owner")
    }

    fn request_for_owner(id: u64, source: &str, owner: &str) -> UiDocumentOpenRequest {
        UiDocumentOpenRequest {
            request_id: UiDocumentRequestId(id),
            document_id: document_id(),
            owner: owner.to_owned(),
            source: UiDocumentOpenSource::Json(source.to_owned()),
            origin: UiDocumentSourceOrigin::Runtime {
                producer: "runtime_test".to_owned(),
            },
            panel: UiDocumentPanel::Page,
            layer: UiDocumentLayer::Page,
            target_profile: UiTargetProfile::new(
                800.0,
                600.0,
                UiSafeAreaClass::None,
                UiDocumentInputMode::MouseKeyboard,
                UiDocumentPlatform::Windows,
            )
            .unwrap(),
            page_state: UiPageState::initial(),
            owner_alive: true,
            host_bindings: BTreeMap::new(),
        }
    }

    fn state(app: &App, request_id: u64) -> UiDocumentBuildState {
        app.world()
            .resource::<UiDocumentRuntime>()
            .record(UiDocumentRequestId(request_id))
            .unwrap()
            .state
    }

    fn active(app: &App) -> UiDocumentInstanceId {
        app.world()
            .resource::<UiDocumentRuntime>()
            .active_instance("runtime_owner", &document_id())
            .unwrap()
    }

    fn root_count(app: &mut App) -> usize {
        let mut query = app.world_mut().query::<&UiDocumentRuntimeRoot>();
        query.iter(app.world()).count()
    }

    fn node_entity(app: &App, id: &str) -> Entity {
        app.world()
            .resource::<UiDocumentRuntime>()
            .node_entity(active(app), &UiNodeId::from_str(id).unwrap())
            .unwrap()
    }

    fn test_image_handle(app: &mut App, width: u32, height: u32) -> Handle<Image> {
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::new_fill(
                Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &[255, 255, 255, 255],
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            ))
    }

    fn metadata_image_document(declared_size: Option<(u32, u32, u64)>) -> String {
        let declared_size = declared_size.map_or_else(String::new, |(width, height, bytes)| {
            format!(
                ", \"declared_size\": {{ \"width\": {width}, \"height\": {height}, \"decoded_bytes\": {bytes} }}"
            )
        });
        format!(
            r#"{{
              "schema_version": 1,
              "document_id": "runtime.transaction",
              "assets": {{
                "metadata_icon": {{
                  "kind": "icon",
                  "source": {{ "kind": "packaged", "path": "ui/runtime/metadata.png" }}{declared_size}
                }}
              }},
              "root": {{
                "type": "icon",
                "id": "metadata.icon",
                "asset": "metadata_icon"
              }}
            }}"#
        )
    }

    fn fallback_image_document(failure: &str, include_placeholder: bool) -> String {
        let placeholder_entry = if include_placeholder {
            r#",
                "fallback_image": {
                  "kind": "image",
                  "source": { "kind": "packaged", "path": "ui/runtime/fallback.png" }
                }"#
        } else {
            ""
        };
        let placeholder_field = if include_placeholder {
            r#", "placeholder": "fallback_image""#
        } else {
            ""
        };
        format!(
            r#"{{
              "schema_version": 1,
              "document_id": "runtime.transaction",
              "assets": {{
                "main_image": {{
                  "kind": "image",
                  "source": {{ "kind": "packaged", "path": "ui/runtime/main.png" }}
                }}{placeholder_entry}
              }},
              "root": {{
                "type": "image",
                "id": "fallback.image",
                "asset": "main_image"{placeholder_field},
                "failure": {failure}
              }}
            }}"#
        )
    }

    #[test]
    fn ui_document_runtime_success_commits_markers_and_stable_node_index() {
        let mut app = test_app();
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(1, SIMPLE_DOCUMENT)));

        app.update();

        assert_eq!(state(&app, 1), UiDocumentBuildState::Committed);
        let instance_id = active(&app);
        let runtime = app.world().resource::<UiDocumentRuntime>();
        let index = runtime.instance(instance_id).unwrap();
        let record = runtime.record(UiDocumentRequestId(1)).unwrap();
        assert_eq!(record.protocol_node_count, 2);
        assert_eq!(record.ecs_entity_count, index.ecs_entity_count);
        assert_eq!(record.asset_count, 0);
        assert!(record.failure_stage.is_none());
        assert_eq!(index.nodes.len(), 2);
        assert!(index.ecs_entity_count >= 2);
        let root = index.root;
        let title = runtime
            .node_entity(instance_id, &UiNodeId::from_str("runtime.title").unwrap())
            .unwrap();
        let root_marker = app.world().get::<UiDocumentRuntimeRoot>(root).unwrap();
        assert_eq!(root_marker.owner, "runtime_owner");
        assert_eq!(root_marker.panel, UiDocumentPanel::Page);
        assert_eq!(root_marker.layer, UiDocumentLayer::Page);
        assert_eq!(root_marker.instance_id, instance_id);
        let panel = app.world().get::<UiPanelRoot>(root).unwrap();
        assert_eq!(panel.kind, UiPanelKind::Page);
        assert!(panel.owner.is_none());
        assert_eq!(
            app.world().get::<UiDocumentNodeMarker>(title),
            Some(&UiDocumentNodeMarker {
                instance_id,
                node_id: UiNodeId::from_str("runtime.title").unwrap(),
            })
        );
        assert_eq!(
            app.world().get::<Text>(title).map(|text| text.0.as_str()),
            Some("Ready")
        );
    }

    #[test]
    fn ui_document_runtime_validation_failure_leaves_no_entities_or_index() {
        let mut app = test_app();
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(2, "{}")));

        app.update();

        let runtime = app.world().resource::<UiDocumentRuntime>();
        let record = runtime.record(UiDocumentRequestId(2)).unwrap();
        assert_eq!(record.state, UiDocumentBuildState::Failed);
        assert_eq!(
            record.failure_stage,
            Some(UiDocumentFailureStage::StaticValidation)
        );
        assert!(
            runtime
                .active_instance("runtime_owner", &document_id())
                .is_none()
        );
        assert_eq!(runtime.pending_count(), 0);
        assert_eq!(root_count(&mut app), 0);
    }

    #[test]
    fn ui_document_runtime_resource_failure_and_cancel_never_spawn_partial_page() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                asset_id(),
                UiDocumentAssetPreflightStatus::Pending,
            );
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(3, ASSET_DOCUMENT)));
        app.update();
        assert_eq!(state(&app, 3), UiDocumentBuildState::Preflighting);
        assert_eq!(root_count(&mut app), 0);

        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Cancel {
                request_id: UiDocumentRequestId(3),
            });
        app.update();
        assert_eq!(state(&app, 3), UiDocumentBuildState::Cancelled);
        assert_eq!(
            app.world().resource::<UiDocumentRuntime>().pending_count(),
            0
        );

        app.world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                asset_id(),
                UiDocumentAssetPreflightStatus::Failed {
                    code: "UI_DOCUMENT_TEST_ASSET_FAILED".to_owned(),
                },
            );
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(4, ASSET_DOCUMENT)));
        app.update();
        let runtime = app.world().resource::<UiDocumentRuntime>();
        let record = runtime.record(UiDocumentRequestId(4)).unwrap();
        assert_eq!(record.state, UiDocumentBuildState::Failed);
        assert_eq!(
            record.failure_stage,
            Some(UiDocumentFailureStage::ResourcePreflight)
        );
        assert_eq!(
            record.failure_code.as_deref(),
            Some("UI_DOCUMENT_TEST_ASSET_FAILED")
        );
        assert!(runtime.instances.is_empty());
    }

    #[test]
    fn ui_document_runtime_replace_keeps_old_visible_until_assets_are_ready() {
        let mut app = test_app();
        let replacement_image = test_image_handle(&mut app, 1, 1);
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(5, SIMPLE_DOCUMENT)));
        app.update();
        let old_instance = active(&app);
        let old_root = app
            .world()
            .resource::<UiDocumentRuntime>()
            .instance(old_instance)
            .unwrap()
            .root;

        app.world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                asset_id(),
                UiDocumentAssetPreflightStatus::Pending,
            );
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(6, ASSET_DOCUMENT)));
        app.update();
        assert_eq!(state(&app, 6), UiDocumentBuildState::Preflighting);
        assert_eq!(active(&app), old_instance);
        assert!(app.world().get_entity(old_root).is_ok());

        app.world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                asset_id(),
                UiDocumentAssetPreflightStatus::Ready {
                    asset: UiDocumentResolvedAsset::Image(replacement_image),
                },
            );
        app.update();
        let new_instance = active(&app);
        assert_ne!(new_instance, old_instance);
        assert_eq!(state(&app, 6), UiDocumentBuildState::Committed);
        assert!(app.world().get_entity(old_root).is_err());
        assert_eq!(root_count(&mut app), 1);
    }

    #[test]
    fn ui_document_runtime_concurrent_open_is_deterministic_last_request_wins() {
        let mut app = test_app();
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(7, SIMPLE_DOCUMENT)));
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(8, SIMPLE_DOCUMENT)));

        app.update();

        assert_eq!(state(&app, 7), UiDocumentBuildState::Cancelled);
        assert_eq!(state(&app, 8), UiDocumentBuildState::Committed);
        let runtime = app.world().resource::<UiDocumentRuntime>();
        let index = runtime.instance(active(&app)).unwrap();
        assert_eq!(index.request_id, UiDocumentRequestId(8));
        assert_eq!(index.generation, 2);
    }

    #[test]
    fn ui_document_runtime_identical_reopen_is_idempotent() {
        let mut app = test_app();
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(11, SIMPLE_DOCUMENT)));
        app.update();
        let instance = active(&app);
        let root = app
            .world()
            .resource::<UiDocumentRuntime>()
            .instance(instance)
            .unwrap()
            .root;

        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(12, SIMPLE_DOCUMENT)));
        app.update();

        assert_eq!(active(&app), instance);
        assert_eq!(root_count(&mut app), 1);
        let record = app
            .world()
            .resource::<UiDocumentRuntime>()
            .record(UiDocumentRequestId(12))
            .unwrap();
        assert_eq!(record.state, UiDocumentBuildState::Committed);
        assert_eq!(record.instance_id, Some(instance));
        assert_eq!(record.generation, 1);
        assert!(app.world().get_entity(root).is_ok());
    }

    #[test]
    fn ui_document_runtime_latest_idempotent_open_supersedes_pending_replace() {
        let mut app = test_app();
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(16, SIMPLE_DOCUMENT)));
        app.update();
        let instance = active(&app);

        app.world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                asset_id(),
                UiDocumentAssetPreflightStatus::Pending,
            );
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(17, ASSET_DOCUMENT)));
        app.update();
        assert_eq!(state(&app, 17), UiDocumentBuildState::Preflighting);

        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(18, SIMPLE_DOCUMENT)));
        app.update();

        assert_eq!(state(&app, 17), UiDocumentBuildState::Cancelled);
        assert_eq!(state(&app, 18), UiDocumentBuildState::Committed);
        assert_eq!(active(&app), instance);
        assert_eq!(
            app.world().resource::<UiDocumentRuntime>().pending_count(),
            0
        );
    }

    #[test]
    fn ui_document_runtime_duplicate_request_event_preserves_original_record() {
        use bevy::ecs::message::MessageCursor;

        let mut app = test_app();
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(19, SIMPLE_DOCUMENT)));
        app.update();
        let original = app
            .world()
            .resource::<UiDocumentRuntime>()
            .record(UiDocumentRequestId(19))
            .unwrap()
            .clone();
        let mut cursor = MessageCursor::<UiDocumentRuntimeEvent>::default();

        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(19, ASSET_DOCUMENT)));
        app.update();

        assert_eq!(
            app.world()
                .resource::<UiDocumentRuntime>()
                .record(UiDocumentRequestId(19)),
            Some(&original)
        );
        let events = app.world().resource::<Messages<UiDocumentRuntimeEvent>>();
        assert!(cursor.read(events).any(|event| {
            event.0.request_id == UiDocumentRequestId(19)
                && event.0.failure_code.as_deref() == Some("UI_DOCUMENT_REQUEST_ID_DUPLICATE")
        }));
    }

    #[test]
    fn ui_document_runtime_selected_i18n_catalog_rejects_missing_key_before_spawn() {
        let mut app = test_app();
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(
                20,
                I18N_MISSING_DOCUMENT,
            )));

        app.update();

        let record = app
            .world()
            .resource::<UiDocumentRuntime>()
            .record(UiDocumentRequestId(20))
            .unwrap();
        assert_eq!(record.state, UiDocumentBuildState::Failed);
        assert_eq!(
            record.failure_stage,
            Some(UiDocumentFailureStage::HostValidation)
        );
        assert_eq!(
            record.failure_code.as_deref(),
            Some("UI_TEXT_I18N_KEY_MISSING")
        );
        assert_eq!(root_count(&mut app), 0);
    }

    #[test]
    fn ui_document_runtime_text_constraints_and_binding_formats_are_explicit() {
        use super::super::{UiBindingValue, UiBindingVisibility};

        assert_eq!(
            format_binding_value(
                &UiBindingValue::Number(-12345.5),
                &UiTextFormat::Number {
                    min_fraction_digits: 2,
                    max_fraction_digits: 4,
                    grouping: true,
                },
            ),
            "-12,345.50"
        );
        assert_eq!(
            format_binding_value(
                &UiBindingValue::Number(0.125),
                &UiTextFormat::Percent {
                    min_fraction_digits: 1,
                    max_fraction_digits: 3,
                },
            ),
            "12.5%"
        );
        assert_eq!(
            format_binding_value(
                &UiBindingValue::Visibility(UiBindingVisibility::Inherited),
                &UiTextFormat::Plain,
            ),
            "inherited"
        );

        let mut app = test_app();
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(
                21,
                CONSTRAINED_TEXT_DOCUMENT,
            )));
        app.update();
        let entity = app
            .world()
            .resource::<UiDocumentRuntime>()
            .node_entity(
                active(&app),
                &UiNodeId::from_str("runtime.constrained_text").unwrap(),
            )
            .unwrap();
        assert_eq!(app.world().get::<Text>(entity).unwrap().0, "one\ntwo…");
        let node = app.world().get::<Node>(entity).unwrap();
        assert_eq!(node.max_height, px(40));
        assert_eq!(node.overflow, Overflow::clip());
    }

    #[test]
    fn ui_document_runtime_button_dispatch_uses_registered_source_node() {
        use crate::framework::ui::document::{UiActionDescriptor, UiRegisteredActionKind};
        use bevy::ecs::message::MessageCursor;

        let mut app = test_app();
        app.world_mut()
            .resource_mut::<UiActionRegistry>()
            .register(UiActionDescriptor::new(
                UiActionId::from_str("runtime.continue").unwrap(),
                document_id(),
                "runtime_owner",
                UiRegisteredActionKind::Route {
                    target: "game.runtime_continue".to_owned(),
                },
            ))
            .unwrap();
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(13, ACTION_DOCUMENT)));
        app.update();
        let instance = active(&app);
        let button = app
            .world()
            .resource::<UiDocumentRuntime>()
            .node_entity(instance, &UiNodeId::from_str("runtime.button").unwrap())
            .unwrap();
        let mut cursor = MessageCursor::<UiActionDispatch>::default();
        app.world_mut().write_message(UiButtonEvent {
            entity: button,
            kind: UiButtonEventKind::Click,
            button: None,
        });

        app.update();

        let dispatches = app.world().resource::<Messages<UiActionDispatch>>();
        let dispatches = cursor.read(dispatches).collect::<Vec<_>>();
        assert_eq!(dispatches.len(), 1);
        assert_eq!(dispatches[0].source_node.as_str(), "runtime.button");
        assert_eq!(dispatches[0].owner, "runtime_owner");
        assert!(matches!(
            dispatches[0].kind,
            UiRegisteredActionKind::Route { .. }
        ));
    }

    #[test]
    fn ui_document_runtime_binding_cleanup_is_instance_and_document_aware() {
        use crate::framework::ui::document::{UiBindingScope, UiBindingValue};

        let mut app = test_app();
        app.init_resource::<UiBindingValues>();
        let shared_path = UiBindingPath::from_str("state.shared").unwrap();
        let local_path = UiBindingPath::from_str("state.local").unwrap();
        let validated = UiDocument::parse_and_validate_json(BINDING_DOCUMENT).unwrap();
        let local = validated.document().bindings[&local_path].clone();
        let shared = validated.document().bindings[&shared_path].clone();
        let mut owner_a = request(14, BINDING_DOCUMENT);
        owner_a.host_bindings.insert(
            UiHostBindingKey::new(UiBindingScope::Document, shared_path.clone()),
            UiBindingType::Number,
        );
        let mut owner_b = request(15, BINDING_DOCUMENT);
        owner_b.owner = "runtime_owner_b".to_owned();
        owner_b.host_bindings = owner_a.host_bindings.clone();
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(owner_a));
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(owner_b));
        app.update();
        {
            let mut values = app.world_mut().resource_mut::<UiBindingValues>();
            assert!(values.set_scoped(
                DOCUMENT_ID,
                "runtime_owner",
                &local_path,
                &local,
                UiBindingValue::String("a".to_owned()),
            ));
            assert!(values.set_scoped(
                DOCUMENT_ID,
                "runtime_owner_b",
                &local_path,
                &local,
                UiBindingValue::String("b".to_owned()),
            ));
            assert!(values.set_scoped(
                DOCUMENT_ID,
                "runtime_owner",
                &shared_path,
                &shared,
                UiBindingValue::Number(7.0),
            ));
        }

        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Close {
                owner: "runtime_owner".to_owned(),
                document_id: document_id(),
            });
        app.update();
        {
            let values = app.world().resource::<UiBindingValues>();
            assert!(
                values
                    .scoped_value(DOCUMENT_ID, "runtime_owner", &local_path, &local)
                    .is_none()
            );
            assert_eq!(
                values.scoped_value(DOCUMENT_ID, "runtime_owner_b", &local_path, &local),
                Some(UiBindingValue::String("b".to_owned()))
            );
            assert_eq!(
                values.scoped_value(DOCUMENT_ID, "runtime_owner_b", &shared_path, &shared),
                Some(UiBindingValue::Number(7.0))
            );
        }

        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Close {
                owner: "runtime_owner_b".to_owned(),
                document_id: document_id(),
            });
        app.update();
        assert!(
            app.world()
                .resource::<UiBindingValues>()
                .scoped_value(DOCUMENT_ID, "runtime_owner_b", &shared_path, &shared)
                .is_none()
        );
    }

    #[test]
    fn ui_document_runtime_close_and_external_despawn_cleanup_entities_and_index() {
        let mut app = test_app();
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(9, SIMPLE_DOCUMENT)));
        app.update();
        let first_instance = active(&app);
        let first_root = app
            .world()
            .resource::<UiDocumentRuntime>()
            .instance(first_instance)
            .unwrap()
            .root;
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Close {
                owner: "runtime_owner".to_owned(),
                document_id: document_id(),
            });
        app.update();
        assert!(app.world().get_entity(first_root).is_err());
        assert!(
            app.world()
                .resource::<UiDocumentRuntime>()
                .instance(first_instance)
                .is_none()
        );
        assert_eq!(state(&app, 9), UiDocumentBuildState::Cleaned);

        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(10, SIMPLE_DOCUMENT)));
        app.update();
        let second_instance = active(&app);
        let second_index = app
            .world()
            .resource::<UiDocumentRuntime>()
            .instance(second_instance)
            .unwrap()
            .clone();
        let title = second_index.nodes[&UiNodeId::from_str("runtime.title").unwrap()];
        app.world_mut().entity_mut(title).despawn();
        app.update();
        let runtime = app.world().resource::<UiDocumentRuntime>();
        assert!(runtime.instance(second_instance).is_none());
        assert!(
            runtime
                .active_instance("runtime_owner", &document_id())
                .is_none()
        );
        assert_eq!(state(&app, 10), UiDocumentBuildState::Cleaned);
        assert!(app.world().get_entity(second_index.root).is_err());
    }

    #[test]
    fn ui_document_runtime_builds_all_control_kinds_with_complete_internal_trees() {
        let mut app = styled_test_app(&[]);
        register_route_action(&mut app);
        let control_image = test_image_handle(&mut app, 1, 1);
        app.world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                UiAssetId::from_str("control_image").unwrap(),
                UiDocumentAssetPreflightStatus::Ready {
                    asset: UiDocumentResolvedAsset::Image(control_image),
                },
            );
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(
                40,
                FULL_CONTROL_DOCUMENT,
            )));

        app.update();

        assert_eq!(state(&app, 40), UiDocumentBuildState::Committed);
        let runtime = app.world().resource::<UiDocumentRuntime>();
        assert_eq!(runtime.instance(active(&app)).unwrap().nodes.len(), 17);

        let button = node_entity(&app, "controls.button");
        let text_input = node_entity(&app, "controls.text_input");
        let checkbox = node_entity(&app, "controls.checkbox");
        let toggle = node_entity(&app, "controls.toggle");
        let segmented = node_entity(&app, "controls.segmented");
        let slider = node_entity(&app, "controls.slider");
        let stepper = node_entity(&app, "controls.stepper");
        let scroll = node_entity(&app, "controls.scroll");
        let modal = node_entity(&app, "controls.modal");
        let image_button = node_entity(&app, "controls.image_button");
        let badge = node_entity(&app, "controls.badge");
        let progress = node_entity(&app, "controls.progress");
        let tab_entity = node_entity(&app, "controls.tab");
        let tooltip = node_entity(&app, "controls.tooltip");
        let select = node_entity(&app, "controls.select");

        assert!(app.world().get::<Button>(button).is_some());
        assert!(app.world().get::<UiTextInput>(text_input).is_some());
        assert!(app.world().get::<UiCheckbox>(checkbox).is_some());
        assert!(app.world().get::<UiToggle>(toggle).is_some());
        assert!(app.world().get::<UiSegmentedControl>(segmented).is_some());
        assert!(app.world().get::<UiSlider>(slider).is_some());
        assert!(app.world().get::<UiStepper>(stepper).is_some());
        assert!(app.world().get::<UiScrollView>(scroll).is_some());
        assert_eq!(find_descendants_with::<Text>(app.world(), modal).len(), 2);
        assert!(app.world().get::<ImageNode>(image_button).is_some());
        assert!(app.world().get::<UiBadge>(badge).is_some());
        assert!(app.world().get::<UiProgress>(progress).is_some());
        assert!(app.world().get::<UiTab>(tab_entity).is_some());
        assert!(app.world().get::<UiTooltip>(tooltip).is_some());
        assert!(app.world().get::<UiDropdown>(select).is_some());

        assert_eq!(
            find_descendants_with::<UiSegmentOption>(app.world(), segmented).len(),
            2
        );
        assert_eq!(
            find_descendants_with::<UiSegmentIndicator>(app.world(), segmented).len(),
            2
        );
        assert_eq!(
            find_descendants_with::<UiSliderTrack>(app.world(), slider).len(),
            1
        );
        assert_eq!(
            find_descendants_with::<UiSliderFill>(app.world(), slider).len(),
            1
        );
        assert_eq!(
            find_descendants_with::<UiSliderValueText>(app.world(), slider).len(),
            1
        );
        assert_eq!(
            find_descendants_with::<UiStepperDecrementButton>(app.world(), stepper).len(),
            1
        );
        assert_eq!(
            find_descendants_with::<UiStepperIncrementButton>(app.world(), stepper).len(),
            1
        );
        assert_eq!(
            find_descendants_with::<UiStepperValueText>(app.world(), stepper).len(),
            1
        );
        assert_eq!(
            find_descendants_with::<UiBadgeLabel>(app.world(), badge).len(),
            1
        );
        assert_eq!(
            find_descendants_with::<UiProgressFill>(app.world(), progress).len(),
            1
        );
        assert_eq!(
            find_descendants_with::<UiProgressLabel>(app.world(), progress).len(),
            1
        );
        assert_eq!(
            find_descendants_with::<UiTabIndicator>(app.world(), tab_entity).len(),
            1
        );
        assert_eq!(
            find_descendants_with::<UiTabLabel>(app.world(), tab_entity).len(),
            1
        );
        assert!(
            app.world()
                .get::<UiTabList>(node_entity(&app, "controls.root"))
                .is_some()
        );
    }

    #[test]
    fn ui_document_runtime_generated_numeric_and_selection_controls_handle_real_clicks() {
        let mut app = styled_test_app(&[]);
        app.init_resource::<UiCurrentOwner>()
            .add_message::<crate::framework::ui::widgets::UiControlEvent>()
            .add_systems(
                Update,
                (
                    update_selection_control_interactions,
                    update_stepper_interactions,
                ),
            );
        register_route_action(&mut app);
        let control_image = test_image_handle(&mut app, 1, 1);
        app.world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                UiAssetId::from_str("control_image").unwrap(),
                UiDocumentAssetPreflightStatus::Ready {
                    asset: UiDocumentResolvedAsset::Image(control_image),
                },
            );
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(
                41,
                FULL_CONTROL_DOCUMENT,
            )));
        app.update();

        let stepper = node_entity(&app, "controls.stepper");
        let increment = find_descendant_with::<UiStepperIncrementButton>(app.world(), stepper)
            .expect("generated stepper must expose its increment button");
        app.world_mut().write_message(UiButtonEvent {
            entity: increment,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.update();
        assert_eq!(app.world().get::<UiStepper>(stepper).unwrap().value, 4);

        let segmented = node_entity(&app, "controls.segmented");
        let option_two = find_descendants_with::<UiSegmentOption>(app.world(), segmented)
            .into_iter()
            .find(|entity| app.world().get::<UiSegmentOption>(*entity).unwrap().value == "two")
            .unwrap();
        app.world_mut().write_message(UiButtonEvent {
            entity: option_two,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.update();
        assert!(
            app.world()
                .get::<UiSegmentOptionSelected>(option_two)
                .is_some()
        );
        assert!(
            app.world()
                .get::<UiControlFlags>(option_two)
                .unwrap()
                .selected
        );
    }

    #[test]
    fn ui_document_runtime_control_labels_refresh_typed_sources_and_resolved_style() {
        let mut app = styled_test_app(&[
            ("runtime.dynamic_button", "Button one"),
            ("runtime.dynamic_placeholder", "Placeholder one"),
            ("runtime.dynamic_select", "Select one"),
            ("runtime.dynamic_option", "Option one"),
            ("runtime.dynamic_tooltip", "Tooltip one"),
        ]);
        register_route_action(&mut app);
        let expected_font = Handle::<Font>::default();
        app.world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                UiAssetId::from_str("label_font").unwrap(),
                UiDocumentAssetPreflightStatus::Ready {
                    asset: UiDocumentResolvedAsset::Font(expected_font.clone()),
                },
            );
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(
                42,
                DYNAMIC_CONTROL_TEXT_DOCUMENT,
            )));
        app.update();

        let button = node_entity(&app, "dynamic.button");
        let button_label = find_descendant_with::<UiButtonStyleLabel>(app.world(), button).unwrap();
        let checkbox = node_entity(&app, "dynamic.checkbox");
        let checkbox_label =
            find_descendant_with::<UiSelectionText>(app.world(), checkbox).unwrap();
        assert_eq!(
            app.world().get::<Text>(button_label).unwrap().0,
            "Button one"
        );
        assert!(app.world().get::<UiI18nText>(button_label).is_some());
        assert_eq!(
            app.world().get::<Text>(checkbox_label).unwrap().0,
            "Initial binding"
        );
        assert!(
            app.world()
                .get::<UiDocumentBoundText>(checkbox_label)
                .is_some()
        );
        let button_color = app
            .world()
            .get::<TextColor>(button_label)
            .unwrap()
            .0
            .to_srgba();
        assert!((button_color.red - 0.2).abs() < 0.001);
        assert!((button_color.green - 0.4).abs() < 0.001);
        assert!((button_color.blue - 0.6).abs() < 0.001);
        assert!((button_color.alpha - 0.5).abs() < 0.001);
        let button_font = app.world().get::<TextFont>(button_label).unwrap();
        assert_eq!(button_font.font, expected_font);
        assert_eq!(button_font.font_size, 23.0);

        let path = UiBindingPath::from_str("state.label").unwrap();
        let declaration = UiDocument::parse_and_validate_json(DYNAMIC_CONTROL_TEXT_DOCUMENT)
            .unwrap()
            .document()
            .bindings[&path]
            .clone();
        assert!(
            app.world_mut()
                .resource_mut::<UiBindingValues>()
                .set_scoped(
                    DOCUMENT_ID,
                    "runtime_owner",
                    &path,
                    &declaration,
                    UiBindingValue::String("Updated binding".to_owned()),
                )
        );
        app.insert_resource(UiI18n::test_with_texts(
            "runtime_test_next",
            &[
                ("runtime.dynamic_button", "Button two"),
                ("runtime.dynamic_placeholder", "Placeholder two"),
                ("runtime.dynamic_select", "Select two"),
                ("runtime.dynamic_option", "Option two"),
                ("runtime.dynamic_tooltip", "Tooltip two"),
            ],
        ));
        app.update();

        assert_eq!(
            app.world().get::<Text>(button_label).unwrap().0,
            "Button two"
        );
        assert_eq!(
            app.world().get::<Text>(checkbox_label).unwrap().0,
            "Updated binding"
        );
        let text_input = node_entity(&app, "dynamic.text_input");
        assert_eq!(
            app.world()
                .get::<UiTextInputPlaceholder>(text_input)
                .unwrap()
                .0,
            "Placeholder two"
        );
        let input_text = find_descendant_with::<UiTextInputText>(app.world(), text_input).unwrap();
        let plain_span = find_descendants_with::<TextSpan>(app.world(), input_text)
            .into_iter()
            .find(|entity| {
                app.world().get::<UiTextInputTextPart>(*entity) == Some(&UiTextInputTextPart::Plain)
            })
            .unwrap();
        assert_eq!(
            app.world().get::<TextSpan>(plain_span).unwrap().as_str(),
            "Placeholder two"
        );
        let select = node_entity(&app, "dynamic.select");
        let select_label = find_descendant_with::<UiDropdownLabel>(app.world(), select).unwrap();
        assert_eq!(
            app.world().get::<Text>(select_label).unwrap().0,
            "Select two"
        );
        let dropdown = app.world().get::<UiDropdown>(select).unwrap();
        assert_eq!(dropdown.options[0].label, "Option two");
        let tooltip = node_entity(&app, "dynamic.tooltip");
        assert_eq!(
            app.world().get::<UiTooltip>(tooltip).unwrap().text,
            "Tooltip two"
        );
    }

    #[test]
    fn ui_document_runtime_local_action_updates_binding_but_external_dispatch_cannot() {
        let mut app = styled_test_app(&[]);
        let binding = UiBindingPath::from_str("state.label").unwrap();
        app.world_mut()
            .resource_mut::<UiActionRegistry>()
            .register(
                UiActionDescriptor::new(
                    UiActionId::from_str("runtime.update_local").unwrap(),
                    document_id(),
                    "runtime_owner",
                    UiRegisteredActionKind::UpdateLocalState {
                        binding: binding.clone(),
                        value_param: "value".to_owned(),
                    },
                )
                .with_param(
                    "value",
                    UiActionParamSchema::required(UiActionParamType::Binding(
                        UiBindingType::String,
                    )),
                ),
            )
            .unwrap();
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(
                43,
                LOCAL_ACTION_DOCUMENT,
            )));
        app.update();
        let text = node_entity(&app, "local.text");
        let button = node_entity(&app, "local.button");
        assert_eq!(app.world().get::<Text>(text).unwrap().0, "Before");

        app.world_mut().write_message(UiButtonEvent {
            entity: button,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.update();
        assert_eq!(app.world().get::<Text>(text).unwrap().0, "After");

        app.world_mut().write_message(UiActionDispatch {
            action: UiActionId::from_str("runtime.update_local").unwrap(),
            document_id: document_id(),
            owner: "spoofed_owner".to_owned(),
            source_node: UiNodeId::from_str("local.button").unwrap(),
            kind: UiRegisteredActionKind::UpdateLocalState {
                binding: binding.clone(),
                value_param: "value".to_owned(),
            },
            params: BTreeMap::from([(
                "value".to_owned(),
                UiActionValue::Binding(UiBindingValue::String("Spoofed".to_owned())),
            )]),
        });
        app.update();
        assert_eq!(app.world().get::<Text>(text).unwrap().0, "After");
    }

    #[test]
    fn ui_document_runtime_invalid_latest_open_supersedes_pending_replacement() {
        let mut app = test_app();
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(44, SIMPLE_DOCUMENT)));
        app.update();
        let original = active(&app);
        app.world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                asset_id(),
                UiDocumentAssetPreflightStatus::Pending,
            );
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(45, ASSET_DOCUMENT)));
        app.update();
        assert_eq!(state(&app, 45), UiDocumentBuildState::Preflighting);

        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(46, "{}")));
        app.update();
        assert_eq!(state(&app, 45), UiDocumentBuildState::Cancelled);
        assert_eq!(state(&app, 46), UiDocumentBuildState::Failed);
        assert_eq!(active(&app), original);
        assert_eq!(
            app.world().resource::<UiDocumentRuntime>().pending_count(),
            0
        );

        let late_image = test_image_handle(&mut app, 1, 1);
        app.world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                asset_id(),
                UiDocumentAssetPreflightStatus::Ready {
                    asset: UiDocumentResolvedAsset::Image(late_image),
                },
            );
        app.update();
        assert_eq!(active(&app), original);
    }

    #[test]
    fn ui_document_runtime_successful_replace_records_cleanup_and_preserves_local_binding() {
        use bevy::ecs::message::MessageCursor;

        let mut app = styled_test_app(&[]);
        let binding = UiBindingPath::from_str("state.label").unwrap();
        app.world_mut()
            .resource_mut::<UiActionRegistry>()
            .register(
                UiActionDescriptor::new(
                    UiActionId::from_str("runtime.update_local").unwrap(),
                    document_id(),
                    "runtime_owner",
                    UiRegisteredActionKind::UpdateLocalState {
                        binding: binding.clone(),
                        value_param: "value".to_owned(),
                    },
                )
                .with_param(
                    "value",
                    UiActionParamSchema::required(UiActionParamType::Binding(
                        UiBindingType::String,
                    )),
                ),
            )
            .unwrap();
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(
                47,
                LOCAL_ACTION_DOCUMENT,
            )));
        app.update();
        let old_instance = active(&app);
        let declaration = UiDocument::parse_and_validate_json(LOCAL_ACTION_DOCUMENT)
            .unwrap()
            .document()
            .bindings[&binding]
            .clone();
        assert!(
            app.world_mut()
                .resource_mut::<UiBindingValues>()
                .set_scoped(
                    DOCUMENT_ID,
                    "runtime_owner",
                    &binding,
                    &declaration,
                    UiBindingValue::String("Keep me".to_owned()),
                )
        );
        let mut cursor = MessageCursor::<UiDocumentRuntimeEvent>::default();

        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(48, SIMPLE_DOCUMENT)));
        app.update();

        assert_ne!(active(&app), old_instance);
        let old_record = app
            .world()
            .resource::<UiDocumentRuntime>()
            .record(UiDocumentRequestId(47))
            .unwrap();
        assert_eq!(old_record.state, UiDocumentBuildState::Cleaned);
        assert_eq!(
            old_record.failure_code.as_deref(),
            Some("UI_DOCUMENT_INSTANCE_REPLACED")
        );
        assert_eq!(
            app.world().resource::<UiBindingValues>().scoped_value(
                DOCUMENT_ID,
                "runtime_owner",
                &binding,
                &declaration,
            ),
            Some(UiBindingValue::String("Keep me".to_owned()))
        );
        let events = app.world().resource::<Messages<UiDocumentRuntimeEvent>>();
        assert!(cursor.read(events).any(|event| {
            event.0.instance_id == Some(old_instance)
                && event.0.state == UiDocumentBuildState::Cleaned
                && event.0.failure_code.as_deref() == Some("UI_DOCUMENT_INSTANCE_REPLACED")
        }));
    }

    #[test]
    fn ui_document_runtime_maps_allowed_slots_variant_size_and_state_override() {
        let mut app = styled_test_app(&[]);
        register_route_action(&mut app);
        let icon = test_image_handle(&mut app, 2, 2);
        app.world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                UiAssetId::from_str("control_icon").unwrap(),
                UiDocumentAssetPreflightStatus::Ready {
                    asset: UiDocumentResolvedAsset::Image(icon),
                },
            );
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(
                60,
                CONTROL_FIELD_MAPPING_DOCUMENT,
            )));
        app.update();
        assert_eq!(state(&app, 60), UiDocumentBuildState::Committed);

        let button = node_entity(&app, "mapping.button");
        assert_eq!(
            app.world().get::<UiDocumentControlPresentation>(button),
            Some(&UiDocumentControlPresentation {
                variant: UiComponentVariant::Destructive,
                size: UiComponentSize::Large,
                state: UiControlState::Pressed,
            })
        );
        let button_slots =
            find_descendants_with::<UiDocumentControlSlotMarker>(app.world(), button);
        assert_eq!(button_slots.len(), 2);
        assert!(button_slots.iter().any(|entity| {
            app.world().get::<UiDocumentControlSlotMarker>(*entity)
                == Some(&UiDocumentControlSlotMarker(UiControlSlot::Leading))
                && app.world().get::<ImageNode>(*entity).is_some()
        }));
        assert!(button_slots.iter().any(|entity| {
            app.world().get::<UiDocumentControlSlotMarker>(*entity)
                == Some(&UiDocumentControlSlotMarker(UiControlSlot::Trailing))
                && app.world().get::<ImageNode>(*entity).is_some()
        }));
        let background = app
            .world()
            .get::<BackgroundColor>(button)
            .unwrap()
            .0
            .to_srgba();
        assert!((background.red - 17.0 / 255.0).abs() < 0.001);
        assert!((background.alpha - 0.5).abs() < 0.001);
        assert_eq!(app.world().get::<Node>(button).unwrap().min_height, px(91));
        let label = find_descendant_with::<UiButtonStyleLabel>(app.world(), button).unwrap();
        assert_eq!(
            app.world().get::<TextFont>(label).unwrap().weight,
            FontWeight::BOLD
        );
        assert_eq!(
            app.world().get::<LineHeight>(label),
            Some(&LineHeight::Px(29.0))
        );

        let input = node_entity(&app, "mapping.input");
        assert_eq!(
            app.world()
                .get::<UiDocumentControlPresentation>(input)
                .unwrap()
                .size,
            UiComponentSize::Small
        );
        let input_slots = find_descendants_with::<UiDocumentControlSlotMarker>(app.world(), input);
        assert!(input_slots.iter().any(|entity| {
            app.world().get::<UiDocumentControlSlotMarker>(*entity)
                == Some(&UiDocumentControlSlotMarker(UiControlSlot::Label))
        }));
        assert!(input_slots.iter().any(|entity| {
            app.world().get::<UiDocumentControlSlotMarker>(*entity)
                == Some(&UiDocumentControlSlotMarker(UiControlSlot::Error))
                && app.world().get::<Text>(*entity).unwrap().0 == "Input error"
        }));
        let helper = input_slots
            .iter()
            .copied()
            .find(|entity| {
                app.world().get::<UiDocumentControlSlotMarker>(*entity)
                    == Some(&UiDocumentControlSlotMarker(UiControlSlot::Helper))
            })
            .unwrap();
        assert_eq!(
            app.world().get::<Visibility>(helper),
            Some(&Visibility::Hidden)
        );

        for (node_id, expected) in [
            ("mapping.slider", "Slider error"),
            ("mapping.modal", "Modal error"),
        ] {
            let entity = node_entity(&app, node_id);
            let error = find_descendants_with::<UiDocumentControlSlotMarker>(app.world(), entity)
                .into_iter()
                .find(|child| {
                    app.world().get::<UiDocumentControlSlotMarker>(*child)
                        == Some(&UiDocumentControlSlotMarker(UiControlSlot::Error))
                })
                .unwrap();
            assert_eq!(app.world().get::<Text>(error).unwrap().0, expected);
        }

        let tab_entity = node_entity(&app, "mapping.tab");
        assert_eq!(
            app.world()
                .get::<UiDocumentControlPresentation>(tab_entity)
                .unwrap()
                .variant,
            UiComponentVariant::Subtle
        );
        assert!(
            find_descendants_with::<UiDocumentControlSlotMarker>(app.world(), tab_entity)
                .iter()
                .any(
                    |child| app.world().get::<UiDocumentControlSlotMarker>(*child)
                        == Some(&UiDocumentControlSlotMarker(UiControlSlot::Leading))
                )
        );
        app.world_mut()
            .entity_mut(tab_entity)
            .insert(Interaction::Hovered);
        app.update();
        assert_eq!(app.world().get::<Node>(button).unwrap().min_height, px(91));
        let expected_tab_background = app
            .world()
            .resource::<UiTheme>()
            .colors
            .secondary_button
            .hovered
            .with_alpha(0.55)
            .to_srgba();
        let actual_tab_background = app
            .world()
            .get::<BackgroundColor>(tab_entity)
            .unwrap()
            .0
            .to_srgba();
        assert!((actual_tab_background.red - expected_tab_background.red).abs() < 0.001);
        assert!((actual_tab_background.green - expected_tab_background.green).abs() < 0.001);
        assert!((actual_tab_background.blue - expected_tab_background.blue).abs() < 0.001);
        assert!((actual_tab_background.alpha - expected_tab_background.alpha).abs() < 0.001);

        let select = node_entity(&app, "mapping.select");
        let select_label = find_descendant_with::<UiDropdownLabel>(app.world(), select).unwrap();
        assert_eq!(
            app.world().get::<Text>(select_label).unwrap().0,
            "Nothing available"
        );
        assert!(
            find_descendants_with::<UiDocumentControlSlotMarker>(app.world(), select)
                .iter()
                .any(
                    |child| app.world().get::<UiDocumentControlSlotMarker>(*child)
                        == Some(&UiDocumentControlSlotMarker(UiControlSlot::Label))
                )
        );
    }

    #[test]
    fn ui_document_runtime_reconciles_live_state_styles_slots_and_layout_boundaries() {
        let mut app = styled_test_app(&[]);
        register_route_action(&mut app);
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(
                70,
                DYNAMIC_CONTROL_STATE_DOCUMENT,
            )));
        app.update();
        assert_eq!(state(&app, 70), UiDocumentBuildState::Committed);

        let button = node_entity(&app, "state.button");
        let label = find_descendant_with::<UiButtonStyleLabel>(app.world(), button).unwrap();
        let nested = node_entity(&app, "state.nested_text");
        let input = node_entity(&app, "state.input");
        let input_slots = find_descendants_with::<UiDocumentControlSlotMarker>(app.world(), input);
        let helper = input_slots
            .iter()
            .copied()
            .find(|entity| {
                app.world().get::<UiDocumentControlSlotMarker>(*entity)
                    == Some(&UiDocumentControlSlotMarker(UiControlSlot::Helper))
            })
            .unwrap();
        let error = input_slots
            .iter()
            .copied()
            .find(|entity| {
                app.world().get::<UiDocumentControlSlotMarker>(*entity)
                    == Some(&UiDocumentControlSlotMarker(UiControlSlot::Error))
            })
            .unwrap();

        let expected_large_label = app.world().resource::<UiTheme>().text.button * 1.15;
        assert!(
            (app.world().get::<TextFont>(label).unwrap().font_size - expected_large_label).abs()
                < 0.001
        );
        assert_eq!(app.world().get::<TextFont>(nested).unwrap().font_size, 37.0);
        assert_control_padding(app.world(), button);
        assert_eq!(
            app.world().get::<Visibility>(helper),
            Some(&Visibility::Inherited)
        );
        assert_eq!(
            app.world().get::<Visibility>(error),
            Some(&Visibility::Hidden)
        );
        let helper_base_size = app
            .world()
            .get::<UiDocumentControlTextBaseline>(helper)
            .unwrap()
            .font
            .font_size;
        assert!(
            (app.world().get::<TextFont>(helper).unwrap().font_size - helper_base_size * 0.85)
                .abs()
                < 0.001
        );

        app.world_mut()
            .entity_mut(button)
            .insert(Interaction::Hovered);
        app.update();
        assert_eq!(
            app.world()
                .get::<UiDocumentControlCurrentState>(button)
                .unwrap()
                .0,
            UiControlState::Hovered
        );
        let hovered_background = app
            .world()
            .get::<BackgroundColor>(button)
            .unwrap()
            .0
            .to_srgba();
        assert!((hovered_background.red - 1.0).abs() < 0.001);
        assert!((hovered_background.alpha - 0.5).abs() < 0.001);
        let hovered_text = app.world().get::<TextColor>(label).unwrap().0.to_srgba();
        assert!((hovered_text.green - 1.0).abs() < 0.001);
        assert!((hovered_text.alpha - 0.5).abs() < 0.001);
        assert_eq!(app.world().get::<TextFont>(label).unwrap().font_size, 30.0);
        assert_eq!(
            app.world().get::<TextFont>(label).unwrap().weight,
            FontWeight::BOLD
        );
        assert_eq!(
            app.world().get::<LineHeight>(label),
            Some(&LineHeight::Px(36.0))
        );
        assert_eq!(
            app.world().get::<Node>(button).unwrap().border_radius,
            BorderRadius::all(px(12))
        );
        assert!(app.world().get::<BoxShadow>(button).is_some());
        assert_eq!(
            app.world()
                .get::<UiDocumentResolvedStyleMarker>(button)
                .unwrap()
                .0
                .properties
                .text
                .as_ref()
                .and_then(|text| text.font_size),
            Some(30.0)
        );
        assert_control_padding(app.world(), button);
        assert_eq!(app.world().get::<TextFont>(nested).unwrap().font_size, 37.0);

        app.world_mut()
            .entity_mut(button)
            .insert(Interaction::Pressed);
        app.update();
        let pressed_background = app
            .world()
            .get::<BackgroundColor>(button)
            .unwrap()
            .0
            .to_srgba();
        assert!((pressed_background.blue - 1.0).abs() < 0.001);
        assert!((pressed_background.alpha - 0.75).abs() < 0.001);
        assert_eq!(app.world().get::<TextFont>(label).unwrap().font_size, 26.0);
        assert_eq!(
            app.world().get::<TextFont>(label).unwrap().weight,
            FontWeight::MEDIUM
        );

        app.world_mut()
            .entity_mut(button)
            .remove::<FocusedButton>()
            .insert(Interaction::None);
        app.update();
        assert_eq!(
            app.world()
                .get::<UiDocumentControlCurrentState>(button)
                .unwrap()
                .0,
            UiControlState::Normal
        );
        let normal_background = app
            .world()
            .get::<BackgroundColor>(button)
            .unwrap()
            .0
            .to_srgba();
        assert!((normal_background.red - 16.0 / 255.0).abs() < 0.001);
        assert!((normal_background.green - 32.0 / 255.0).abs() < 0.001);
        assert!((normal_background.blue - 48.0 / 255.0).abs() < 0.001);
        assert!((normal_background.alpha - 1.0).abs() < 0.001);
        assert!(
            (app.world().get::<TextFont>(label).unwrap().font_size - expected_large_label).abs()
                < 0.001
        );
        assert_eq!(
            app.world().get::<Node>(button).unwrap().border_radius,
            BorderRadius::all(px(4))
        );
        assert!(app.world().get::<BoxShadow>(button).is_none());
        assert_control_padding(app.world(), button);
        assert_eq!(app.world().get::<TextFont>(nested).unwrap().font_size, 37.0);

        app.world_mut().entity_mut(input).insert(UiControlFlags {
            error: true,
            ..default()
        });
        app.update();
        assert_eq!(
            app.world()
                .get::<UiDocumentControlCurrentState>(input)
                .unwrap()
                .0,
            UiControlState::Error
        );
        assert_eq!(
            app.world().get::<Visibility>(helper),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            app.world().get::<Visibility>(error),
            Some(&Visibility::Inherited)
        );
        assert_eq!(app.world().get::<TextFont>(error).unwrap().font_size, 25.0);
        assert_eq!(
            app.world().get::<TextFont>(error).unwrap().weight,
            FontWeight::BOLD
        );
        let error_color = app.world().get::<TextColor>(error).unwrap().0.to_srgba();
        assert!((error_color.red - 1.0).abs() < 0.001);
        assert!((error_color.alpha - (238.0 / 255.0 * 0.5)).abs() < 0.001);

        app.world_mut()
            .entity_mut(input)
            .insert(UiControlFlags::default());
        app.update();
        assert_eq!(
            app.world()
                .get::<UiDocumentControlCurrentState>(input)
                .unwrap()
                .0,
            UiControlState::Normal
        );
        assert_eq!(
            app.world().get::<Visibility>(helper),
            Some(&Visibility::Inherited)
        );
        assert_eq!(
            app.world().get::<Visibility>(error),
            Some(&Visibility::Hidden)
        );
        assert!(
            (app.world().get::<TextFont>(helper).unwrap().font_size - helper_base_size * 0.85)
                .abs()
                < 0.001
        );
    }

    fn assert_control_padding(world: &World, entity: Entity) {
        let padding = world.get::<Node>(entity).unwrap().padding;
        assert_eq!(padding.left, px(11));
        assert_eq!(padding.right, px(13));
        assert_eq!(padding.top, px(17));
        assert_eq!(padding.bottom, px(19));
    }

    #[test]
    fn ui_document_runtime_generated_text_opacity_tracks_state_color_without_accumulation() {
        let mut app = styled_test_app(&[]);
        register_route_action(&mut app);
        let icon = test_image_handle(&mut app, 2, 2);
        app.world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                UiAssetId::from_str("control_icon").unwrap(),
                UiDocumentAssetPreflightStatus::Ready {
                    asset: UiDocumentResolvedAsset::Image(icon),
                },
            );
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(
                61,
                CONTROL_FIELD_MAPPING_DOCUMENT,
            )));
        app.update();
        let button = node_entity(&app, "mapping.button");
        let label = find_descendant_with::<UiButtonStyleLabel>(app.world(), button).unwrap();

        app.world_mut()
            .entity_mut(label)
            .insert(TextColor(Color::srgb(1.0, 0.0, 0.0)));
        app.update();
        let first = app.world().get::<TextColor>(label).unwrap().0.to_srgba();
        assert!((first.red - 1.0).abs() < 0.001);
        assert!((first.alpha - 0.5).abs() < 0.001);
        app.update();
        let second = app.world().get::<TextColor>(label).unwrap().0.to_srgba();
        assert!((second.red - 1.0).abs() < 0.001);
        assert!((second.alpha - 0.5).abs() < 0.001);
    }

    #[test]
    fn ui_document_runtime_rejects_unsupported_nonzero_letter_spacing_before_spawn() {
        let source = SIMPLE_DOCUMENT.replace(
            "\"content\": { \"literal\": \"Ready\" }",
            "\"content\": { \"literal\": \"Ready\" }, \"style\": { \"inline\": { \"text\": { \"letter_spacing\": { \"kind\": \"literal\", \"value\": 1.0 } } } }",
        );
        let mut app = test_app();
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(62, &source)));
        app.update();
        let record = app
            .world()
            .resource::<UiDocumentRuntime>()
            .record(UiDocumentRequestId(62))
            .unwrap();
        assert_eq!(record.state, UiDocumentBuildState::Failed);
        assert_eq!(
            record.failure_code.as_deref(),
            Some("UI_DOCUMENT_TEXT_LETTER_SPACING_UNSUPPORTED")
        );
        assert_eq!(root_count(&mut app), 0);
    }

    #[test]
    fn ui_document_runtime_checks_actual_image_metadata_and_hard_budgets() {
        let valid_source = metadata_image_document(Some((2, 2, 16)));
        let mut app = test_app();
        let valid = test_image_handle(&mut app, 2, 2);
        app.world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                UiAssetId::from_str("metadata_icon").unwrap(),
                UiDocumentAssetPreflightStatus::Ready {
                    asset: UiDocumentResolvedAsset::Image(valid),
                },
            );
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(63, &valid_source)));
        app.update();
        assert_eq!(state(&app, 63), UiDocumentBuildState::Committed);

        let mismatch_source = metadata_image_document(Some((3, 2, 24)));
        let mut mismatch = test_app();
        let actual = test_image_handle(&mut mismatch, 2, 2);
        mismatch
            .world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                UiAssetId::from_str("metadata_icon").unwrap(),
                UiDocumentAssetPreflightStatus::Ready {
                    asset: UiDocumentResolvedAsset::Image(actual),
                },
            );
        mismatch
            .world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(
                64,
                &mismatch_source,
            )));
        mismatch.update();
        assert_eq!(state(&mismatch, 64), UiDocumentBuildState::Failed);
        assert_eq!(
            mismatch
                .world()
                .resource::<UiDocumentRuntime>()
                .record(UiDocumentRequestId(64))
                .unwrap()
                .failure_code
                .as_deref(),
            Some("UI_DOCUMENT_ASSET_METADATA_MISMATCH")
        );
        assert_eq!(root_count(&mut mismatch), 0);

        let over_dimension_source = metadata_image_document(None);
        let mut over_dimension = test_app();
        let actual = test_image_handle(&mut over_dimension, 4097, 1);
        over_dimension
            .world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                UiAssetId::from_str("metadata_icon").unwrap(),
                UiDocumentAssetPreflightStatus::Ready {
                    asset: UiDocumentResolvedAsset::Image(actual),
                },
            );
        over_dimension
            .world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(
                65,
                &over_dimension_source,
            )));
        over_dimension.update();
        assert_eq!(
            over_dimension
                .world()
                .resource::<UiDocumentRuntime>()
                .record(UiDocumentRequestId(65))
                .unwrap()
                .failure_code
                .as_deref(),
            Some("UI_DOCUMENT_ASSET_ACTUAL_DIMENSION_BUDGET_EXCEEDED")
        );

        let over_bytes_source = metadata_image_document(None);
        let mut over_bytes = test_app();
        let actual = test_image_handle(&mut over_bytes, 2048, 2049);
        over_bytes
            .world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                UiAssetId::from_str("metadata_icon").unwrap(),
                UiDocumentAssetPreflightStatus::Ready {
                    asset: UiDocumentResolvedAsset::Image(actual),
                },
            );
        over_bytes
            .world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(
                66,
                &over_bytes_source,
            )));
        over_bytes.update();
        assert_eq!(
            over_bytes
                .world()
                .resource::<UiDocumentRuntime>()
                .record(UiDocumentRequestId(66))
                .unwrap()
                .failure_code
                .as_deref(),
            Some("UI_DOCUMENT_ASSET_ACTUAL_BYTES_BUDGET_EXCEEDED")
        );
    }

    #[test]
    fn ui_document_runtime_image_fallback_commits_and_late_ready_switches_main_asset() {
        let source = fallback_image_document(r#"{ "kind": "placeholder" }"#, true);
        let mut app = test_app();
        let placeholder = test_image_handle(&mut app, 2, 2);
        app.world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                UiAssetId::from_str("main_image").unwrap(),
                UiDocumentAssetPreflightStatus::Pending,
            );
        app.world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                UiAssetId::from_str("fallback_image").unwrap(),
                UiDocumentAssetPreflightStatus::Ready {
                    asset: UiDocumentResolvedAsset::Image(placeholder.clone()),
                },
            );
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(67, &source)));
        app.update();
        assert_eq!(state(&app, 67), UiDocumentBuildState::Committed);
        let image_entity = node_entity(&app, "fallback.image");
        assert_eq!(
            app.world().get::<ImageNode>(image_entity).unwrap().image,
            placeholder
        );
        assert_eq!(
            app.world()
                .get::<UiDocumentRuntimeImage>(image_entity)
                .unwrap()
                .state,
            UiDocumentRuntimeImageState::Loading
        );

        let main = test_image_handle(&mut app, 3, 2);
        app.world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                UiAssetId::from_str("main_image").unwrap(),
                UiDocumentAssetPreflightStatus::Ready {
                    asset: UiDocumentResolvedAsset::Image(main.clone()),
                },
            );
        app.update();
        assert_eq!(
            app.world().get::<ImageNode>(image_entity).unwrap().image,
            main
        );
        assert_eq!(
            app.world()
                .get::<UiDocumentRuntimeImage>(image_entity)
                .unwrap()
                .state,
            UiDocumentRuntimeImageState::Ready
        );
    }

    #[test]
    fn ui_document_runtime_late_asset_ledger_is_unique_per_asset_and_instance() {
        let mut app = test_app();
        let fallback = test_image_handle(&mut app, 2, 2);
        app.world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                UiAssetId::from_str("main_image").unwrap(),
                UiDocumentAssetPreflightStatus::Pending,
            );
        app.world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                UiAssetId::from_str("fallback_image").unwrap(),
                UiDocumentAssetPreflightStatus::Ready {
                    asset: UiDocumentResolvedAsset::Image(fallback),
                },
            );
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request_for_owner(
                71,
                DUPLICATE_LATE_IMAGE_DOCUMENT,
                "runtime_owner_a",
            )));
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request_for_owner(
                72,
                DUPLICATE_LATE_IMAGE_DOCUMENT,
                "runtime_owner_b",
            )));
        app.update();
        assert_eq!(state(&app, 71), UiDocumentBuildState::Committed);
        assert_eq!(state(&app, 72), UiDocumentBuildState::Committed);

        let main = test_image_handle(&mut app, 3, 2);
        let main_bytes = app
            .world()
            .resource::<Assets<Image>>()
            .get(&main)
            .unwrap()
            .data
            .as_ref()
            .unwrap()
            .len() as u64;
        app.world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                UiAssetId::from_str("main_image").unwrap(),
                UiDocumentAssetPreflightStatus::Ready {
                    asset: UiDocumentResolvedAsset::Image(main),
                },
            );
        app.update();

        let runtime = app.world().resource::<UiDocumentRuntime>();
        for owner in ["runtime_owner_a", "runtime_owner_b"] {
            let instance = runtime.active_instance(owner, &document_id()).unwrap();
            let active = runtime.instances.get(&instance).unwrap();
            assert_eq!(active.asset_decoded_bytes.len(), 2);
            assert_eq!(
                active
                    .asset_decoded_bytes
                    .get(&UiAssetId::from_str("main_image").unwrap()),
                Some(&main_bytes)
            );
            assert_eq!(
                active.asset_decoded_bytes.values().sum::<u64>(),
                16 + main_bytes
            );
        }
    }

    #[test]
    fn ui_document_runtime_late_asset_budget_includes_required_assets() {
        let mut app = test_app();
        register_route_action(&mut app);
        let required = test_image_handle(&mut app, 2048, 2048);
        let main_id = UiAssetId::from_str("main_image").unwrap();
        app.world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                main_id.clone(),
                UiDocumentAssetPreflightStatus::Pending,
            );
        for index in 0..4 {
            app.world_mut()
                .resource_mut::<UiDocumentAssetPreflightOverrides>()
                .set(
                    document_id(),
                    UiAssetId::from_str(&format!("required_{index}")).unwrap(),
                    UiDocumentAssetPreflightStatus::Ready {
                        asset: UiDocumentResolvedAsset::Image(required.clone()),
                    },
                );
        }
        app.world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(
                73,
                LATE_IMAGE_TOTAL_BUDGET_DOCUMENT,
            )));
        app.update();
        assert_eq!(state(&app, 73), UiDocumentBuildState::Committed);
        let instance = active(&app);
        {
            let runtime = app.world().resource::<UiDocumentRuntime>();
            let ledger = &runtime
                .instances
                .get(&instance)
                .unwrap()
                .asset_decoded_bytes;
            assert_eq!(ledger.len(), 4);
            assert_eq!(
                ledger.values().sum::<u64>(),
                super::super::UI_ASSET_MAX_TOTAL_DECODED_BYTES
            );
        }

        let main = test_image_handle(&mut app, 1, 1);
        app.world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                main_id.clone(),
                UiDocumentAssetPreflightStatus::Ready {
                    asset: UiDocumentResolvedAsset::Image(main),
                },
            );
        app.update();

        let image = node_entity(&app, "budget.image");
        assert_eq!(
            app.world()
                .get::<UiDocumentRuntimeImage>(image)
                .unwrap()
                .state,
            UiDocumentRuntimeImageState::Failed
        );
        let failure_color = app
            .world()
            .get::<BackgroundColor>(image)
            .unwrap()
            .0
            .to_srgba();
        assert!((failure_color.red - 18.0 / 255.0).abs() < 0.001);
        assert!((failure_color.green - 52.0 / 255.0).abs() < 0.001);
        assert!((failure_color.blue - 86.0 / 255.0).abs() < 0.001);
        let runtime = app.world().resource::<UiDocumentRuntime>();
        let ledger = &runtime
            .instances
            .get(&instance)
            .unwrap()
            .asset_decoded_bytes;
        assert_eq!(ledger.len(), 4);
        assert!(!ledger.contains_key(&main_id));
        assert_eq!(
            ledger.values().sum::<u64>(),
            super::super::UI_ASSET_MAX_TOTAL_DECODED_BYTES
        );
    }

    #[test]
    fn ui_document_runtime_failed_image_uses_error_color_or_hide_without_failing_page() {
        let error_source = fallback_image_document(
            r##"{ "kind": "error_color", "color": "#aabbccff" }"##,
            false,
        );
        let mut error_app = test_app();
        error_app
            .world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                UiAssetId::from_str("main_image").unwrap(),
                UiDocumentAssetPreflightStatus::Failed {
                    code: "TEST_MAIN_FAILED".to_owned(),
                },
            );
        error_app
            .world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(68, &error_source)));
        error_app.update();
        assert_eq!(state(&error_app, 68), UiDocumentBuildState::Committed);
        let entity = node_entity(&error_app, "fallback.image");
        let color = error_app
            .world()
            .get::<BackgroundColor>(entity)
            .unwrap()
            .0
            .to_srgba();
        assert!((color.red - 170.0 / 255.0).abs() < 0.001);
        assert!(error_app.world().get::<ImageNode>(entity).is_none());

        let hide_source = fallback_image_document(r#"{ "kind": "hide" }"#, false);
        let mut hide_app = test_app();
        hide_app
            .world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>()
            .set(
                document_id(),
                UiAssetId::from_str("main_image").unwrap(),
                UiDocumentAssetPreflightStatus::Failed {
                    code: "TEST_MAIN_FAILED".to_owned(),
                },
            );
        hide_app
            .world_mut()
            .write_message(UiDocumentRuntimeCommand::Open(request(69, &hide_source)));
        hide_app.update();
        assert_eq!(state(&hide_app, 69), UiDocumentBuildState::Committed);
        let entity = node_entity(&hide_app, "fallback.image");
        assert_eq!(
            hide_app.world().get::<Visibility>(entity),
            Some(&Visibility::Hidden)
        );
        assert!(hide_app.world().get::<ImageNode>(entity).is_none());
    }
}
