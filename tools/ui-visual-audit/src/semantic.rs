use crate::comparison::{
    ArtifactReport, ComparisonError, ComparisonErrorCode, ComparisonExitCode,
    create_output_directory, resolve_allowed_input_roots, resolve_allowed_root, resolve_input_file,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const SEMANTIC_AUDIT_ALGORITHM_VERSION: &str = "ui_semantic_audit_v1";
pub const SEMANTIC_AUDIT_CONFIG_SCHEMA_VERSION: u32 = 1;
pub const SEMANTIC_TREE_SCHEMA_VERSION: u32 = 3;
pub const SEMANTIC_AUDIT_REPORT_SCHEMA_VERSION: u32 = 3;
pub const SEMANTIC_AUDIT_REPORT_FILENAME: &str = "semantic-audit-report.json";
pub const SEMANTIC_AUDIT_PEAK_MEMORY_BUDGET_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_SEMANTIC_FINDINGS: usize = 1_024;
pub const MAX_SEMANTIC_OVERLAP_CANDIDATES: usize = 8_192;

const MAX_METADATA_BYTES: u64 = 8 * 1024 * 1024;
const MAX_CONFIG_BYTES: u64 = 64 * 1024;
const MAX_NODES: usize = 50_000;
const MAX_PANELS: usize = 1_024;
const MAX_STRING_BYTES: usize = 512;
const MAX_LIKELY_FILES: usize = 16;
// Covers each owned finding plus its concurrently retained pretty-printed JSON bytes.
const ESTIMATED_FINDING_BYTES: u64 = 32 * 1024;
const ESTIMATED_TEXT_CANDIDATE_BYTES: u64 = 128;

#[derive(Clone, Debug, PartialEq)]
pub struct SemanticAuditRequest {
    pub repository_root: PathBuf,
    pub allowed_input_roots: Vec<PathBuf>,
    pub allowed_output_root: PathBuf,
    pub metadata: PathBuf,
    pub config: PathBuf,
    pub output_directory: PathBuf,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticAuditConfig {
    pub schema_version: u32,
    pub algorithm_version: String,
    pub minimum_touch_width: f64,
    pub minimum_touch_height: f64,
    pub geometry_epsilon: f64,
    pub text_overlap_minimum_area: f64,
    pub require_safe_area_for_roles: Vec<SemanticNodeRole>,
}

#[derive(Clone, Debug, Deserialize)]
struct RuntimeAuditMetadata {
    device: String,
    semantic_tree: SemanticTree,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticTree {
    pub schema_version: u32,
    pub coordinate_space: String,
    pub rect_convention: String,
    pub rounding: String,
    pub target_root_id: String,
    pub viewport: SemanticRect,
    pub safe_area: SemanticRect,
    pub nodes: Vec<SemanticNode>,
    pub panels: Vec<SemanticPanel>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticRect {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl SemanticRect {
    fn width(self) -> f64 {
        self.max_x - self.min_x
    }

    fn height(self) -> f64 {
        self.max_y - self.min_y
    }

    fn is_valid(self) -> bool {
        [self.min_x, self.min_y, self.max_x, self.max_y]
            .iter()
            .all(|value| value.is_finite())
            && self.min_x <= self.max_x
            && self.min_y <= self.max_y
    }

    fn intersection(self, other: Self) -> Option<Self> {
        let value = Self {
            min_x: self.min_x.max(other.min_x),
            min_y: self.min_y.max(other.min_y),
            max_x: self.max_x.min(other.max_x),
            max_y: self.max_y.min(other.max_y),
        };
        (value.width() > 0.0 && value.height() > 0.0).then_some(value)
    }

    fn contains(self, other: Self, epsilon: f64) -> bool {
        other.min_x >= self.min_x - epsilon
            && other.min_y >= self.min_y - epsilon
            && other.max_x <= self.max_x + epsilon
            && other.max_y <= self.max_y + epsilon
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticNodeRole {
    Layout,
    Text,
    CriticalText,
    Button,
    IconButton,
    TextInput,
    Scroll,
    Image,
    Modal,
    Loading,
    Floating,
    Toast,
}

impl SemanticNodeRole {
    fn is_actionable(self) -> bool {
        matches!(self, Self::Button | Self::IconButton | Self::TextInput)
    }

    fn is_text(self) -> bool {
        matches!(self, Self::Text | Self::CriticalText)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentitySource {
    DeclarativeNode,
    NamedHierarchy,
    HierarchyFallback,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticNode {
    pub stable_id: String,
    pub identity_source: IdentitySource,
    pub capture_entity: String,
    pub entity_name: Option<String>,
    pub stack_index: u32,
    pub parent_id: Option<String>,
    pub depth: u32,
    pub role: SemanticNodeRole,
    pub visible: bool,
    pub fully_clipped: bool,
    pub bounds: SemanticRect,
    pub clip_bounds: SemanticRect,
    pub measured_text_bounds: Option<SemanticRect>,
    pub text_nonempty: bool,
    pub has_visible_label: bool,
    pub interaction: String,
    pub disabled: bool,
    pub loading: bool,
    pub focused: bool,
    pub scroll: Option<SemanticScroll>,
    pub document_id: Option<String>,
    pub node_id: Option<String>,
    pub source_path: Option<String>,
    pub panel_id: Option<String>,
    pub likely_files: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticScroll {
    pub viewport_height: f64,
    pub content_height: f64,
    pub max_offset: f64,
    pub current_offset: f64,
    pub content_reachable: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPanel {
    pub stable_id: String,
    pub capture_entity: String,
    pub entity_name: Option<String>,
    pub likely_files: Vec<String>,
    pub kind: SemanticPanelKind,
    pub layer_policy: SemanticLayerPolicy,
    pub visible: bool,
    pub z_index: i32,
    pub has_focusable_descendants: bool,
    pub focused_descendant: bool,
    pub focused_stable_id: Option<String>,
    pub active_focus_scope: bool,
    pub focus_scope_enforced: bool,
    pub focus_suppressed: bool,
    pub blocks_lower_input: bool,
    pub pickable_blocks_lower: bool,
    pub input_block_reason: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticPanelKind {
    Page,
    Hud,
    Floating,
    Modal,
    BlockingOverlay,
    Toast,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticLayerPolicy {
    Base,
    Floating,
    Modal,
    TransientAboveModal,
    Blocking,
    Toast,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticFindingCode {
    TextOverlap,
    CriticalTextClipped,
    SafeAreaOverflow,
    ScrollContentUnreachable,
    SemanticNodeZeroSize,
    TouchTargetTooSmall,
    VisibleLabelMissing,
    DisabledStateInconsistent,
    LoadingStateInconsistent,
    OverlayZOrderInvalid,
    OverlayFocusScopeInvalid,
    OverlayInputBlockingInvalid,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticSeverity {
    HardFailure,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticLocation {
    pub stable_id: String,
    pub capture_entity: String,
    pub entity_name: Option<String>,
    pub document_id: Option<String>,
    pub node_id: Option<String>,
    pub source_path: Option<String>,
    pub panel_id: Option<String>,
    pub likely_files: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticFinding {
    pub code: SemanticFindingCode,
    pub severity: SemanticSeverity,
    pub message: String,
    pub primary: SemanticLocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_stable_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_rect: Option<SemanticRect>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticAuditStatus {
    Passed,
    SemanticFailed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticInputReport {
    pub path: String,
    pub byte_length: u64,
    pub metadata_sha256: String,
    pub device_profile: String,
    pub target_root_id: String,
    pub node_count: usize,
    pub panel_count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticRuleSummary {
    pub evaluated_visible_nodes: usize,
    pub skipped_invisible_nodes: usize,
    pub skipped_fully_clipped_nodes: usize,
    pub skipped_layout_only_nodes: usize,
    pub hard_failure_count: usize,
    pub findings_by_code: BTreeMap<SemanticFindingCode, usize>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticSeparationContract {
    pub semantic_hard_failure: bool,
    pub visual_similarity_consumed: bool,
    pub local_visual_scores_consumed: bool,
    pub can_visual_score_offset_hard_failure: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPerformanceReport {
    pub estimated_peak_memory_bytes: u64,
    pub budget_bytes: u64,
    pub memory_basis: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticAuditReport {
    pub schema_version: u32,
    pub algorithm_version: String,
    pub status: SemanticAuditStatus,
    pub input: SemanticInputReport,
    pub coordinate_space: String,
    pub rect_convention: String,
    pub rounding: String,
    pub rules: SemanticRuleSummary,
    pub separation: SemanticSeparationContract,
    pub findings: Vec<SemanticFinding>,
    pub performance: SemanticPerformanceReport,
    pub artifacts: Vec<ArtifactReport>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SemanticAuditOutcome {
    pub report: SemanticAuditReport,
    pub exit_code: ComparisonExitCode,
}

pub fn audit_semantics(
    request: &SemanticAuditRequest,
) -> Result<SemanticAuditOutcome, ComparisonError> {
    let repository_root = fs::canonicalize(&request.repository_root).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::RepositoryRootInvalid,
            format!("repository root cannot be resolved: {error}"),
        )
        .at_path(&request.repository_root)
    })?;
    if !repository_root.is_dir() {
        return Err(ComparisonError::input(
            ComparisonErrorCode::RepositoryRootInvalid,
            "repository root is not a directory",
        ));
    }
    let input_roots = resolve_allowed_input_roots(&repository_root, &request.allowed_input_roots)?;
    let output_root = resolve_allowed_root(
        &repository_root,
        &request.allowed_output_root,
        ComparisonErrorCode::AllowedOutputRootInvalid,
        "allowed output root",
    )?;
    let metadata_path = resolve_input_file(&repository_root, &input_roots, &request.metadata)?;
    let config_path = resolve_input_file(&repository_root, &input_roots, &request.config)?;
    let (metadata, metadata_len, metadata_sha256) = load_metadata(&metadata_path)?;
    let config = load_config(&config_path)?;
    validate_protocol(&metadata, &config)?;
    let estimated_memory = estimate_memory(metadata_len, &metadata.semantic_tree)?;
    if estimated_memory > SEMANTIC_AUDIT_PEAK_MEMORY_BUDGET_BYTES {
        return Err(ComparisonError::input(
            ComparisonErrorCode::SemanticMetadataTooLarge,
            format!(
                "semantic audit estimated peak {estimated_memory} exceeds the {}-byte budget",
                SEMANTIC_AUDIT_PEAK_MEMORY_BUDGET_BYTES
            ),
        ));
    }
    let output_directory =
        create_output_directory(&repository_root, &output_root, &request.output_directory)?;
    let report_path = output_directory.join(SEMANTIC_AUDIT_REPORT_FILENAME);
    if report_path == metadata_path || report_path == config_path {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ArtifactNameConflict,
            "semantic report would overwrite an input file",
        )
        .at_path(&report_path));
    }

    let (mut findings, rules) = evaluate_semantics(&metadata.semantic_tree, &config)?;
    findings.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then_with(|| left.primary.stable_id.cmp(&right.primary.stable_id))
            .then_with(|| left.related_stable_id.cmp(&right.related_stable_id))
    });
    let failed = !findings.is_empty();
    let report = SemanticAuditReport {
        schema_version: SEMANTIC_AUDIT_REPORT_SCHEMA_VERSION,
        algorithm_version: SEMANTIC_AUDIT_ALGORITHM_VERSION.to_owned(),
        status: if failed {
            SemanticAuditStatus::SemanticFailed
        } else {
            SemanticAuditStatus::Passed
        },
        input: SemanticInputReport {
            path: metadata_path.display().to_string(),
            byte_length: metadata_len,
            metadata_sha256,
            device_profile: metadata.device,
            target_root_id: metadata.semantic_tree.target_root_id.clone(),
            node_count: metadata.semantic_tree.nodes.len(),
            panel_count: metadata.semantic_tree.panels.len(),
        },
        coordinate_space: metadata.semantic_tree.coordinate_space.clone(),
        rect_convention: metadata.semantic_tree.rect_convention.clone(),
        rounding: metadata.semantic_tree.rounding.clone(),
        rules,
        separation: SemanticSeparationContract {
            semantic_hard_failure: failed,
            visual_similarity_consumed: false,
            local_visual_scores_consumed: false,
            can_visual_score_offset_hard_failure: false,
        },
        findings,
        performance: SemanticPerformanceReport {
            estimated_peak_memory_bytes: estimated_memory,
            budget_bytes: SEMANTIC_AUDIT_PEAK_MEMORY_BUDGET_BYTES,
            memory_basis: format!(
                "input_bytes_x3_plus_nodes_x2048_plus_panels_x1024_plus_nodes_x{}_text_candidates_plus_{}_findings_x{}",
                ESTIMATED_TEXT_CANDIDATE_BYTES, MAX_SEMANTIC_FINDINGS, ESTIMATED_FINDING_BYTES
            ),
        },
        artifacts: vec![ArtifactReport {
            artifact_type: "semantic_audit_report".to_owned(),
            path: report_path.display().to_string(),
        }],
    };
    persist_report(&report_path, &report)?;
    Ok(SemanticAuditOutcome {
        report,
        exit_code: if failed {
            ComparisonExitCode::ThresholdFailure
        } else {
            ComparisonExitCode::Success
        },
    })
}

fn load_metadata(path: &Path) -> Result<(RuntimeAuditMetadata, u64, String), ComparisonError> {
    let bytes = read_limited(
        path,
        MAX_METADATA_BYTES,
        ComparisonErrorCode::SemanticMetadataTooLarge,
    )?;
    let metadata = serde_json::from_slice(&bytes).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::SemanticMetadataInvalid,
            format!("semantic metadata is invalid: {error}"),
        )
        .at_path(path)
    })?;
    let length = bytes.len() as u64;
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    Ok((metadata, length, sha256))
}

fn load_config(path: &Path) -> Result<SemanticAuditConfig, ComparisonError> {
    let bytes = read_limited(path, MAX_CONFIG_BYTES, ComparisonErrorCode::ConfigTooLarge)?;
    serde_json::from_slice(&bytes).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::SemanticConfigInvalid,
            format!("semantic config is invalid: {error}"),
        )
        .at_path(path)
    })
}

fn read_limited(
    path: &Path,
    maximum: u64,
    code: ComparisonErrorCode,
) -> Result<Vec<u8>, ComparisonError> {
    let file = fs::File::open(path).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::ConfigReadFailed,
            format!("input metadata cannot be opened: {error}"),
        )
        .at_path(path)
    })?;
    let mut bytes = Vec::new();
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            ComparisonError::input(
                ComparisonErrorCode::ConfigReadFailed,
                format!("input metadata cannot be read: {error}"),
            )
            .at_path(path)
        })?;
    if bytes.len() as u64 > maximum {
        return Err(ComparisonError::input(
            code,
            format!("input length exceeds the {maximum}-byte limit"),
        )
        .at_path(path));
    }
    Ok(bytes)
}

fn validate_protocol(
    metadata: &RuntimeAuditMetadata,
    config: &SemanticAuditConfig,
) -> Result<(), ComparisonError> {
    if config.schema_version != SEMANTIC_AUDIT_CONFIG_SCHEMA_VERSION
        || config.algorithm_version != SEMANTIC_AUDIT_ALGORITHM_VERSION
        || !config.minimum_touch_width.is_finite()
        || config.minimum_touch_width <= 0.0
        || !config.minimum_touch_height.is_finite()
        || config.minimum_touch_height <= 0.0
        || !config.geometry_epsilon.is_finite()
        || !(0.0..=4.0).contains(&config.geometry_epsilon)
        || !config.text_overlap_minimum_area.is_finite()
        || config.text_overlap_minimum_area < 0.0
    {
        return Err(ComparisonError::input(
            ComparisonErrorCode::SemanticConfigInvalid,
            "semantic config version, algorithm, or numeric thresholds are invalid",
        ));
    }
    let tree = &metadata.semantic_tree;
    if tree.schema_version != SEMANTIC_TREE_SCHEMA_VERSION
        || tree.coordinate_space != "logical_pixels"
        || tree.rect_convention != "half_open"
        || tree.rounding != "nearest_1_64_half_away_from_zero"
        || !tree.viewport.is_valid()
        || !tree.safe_area.is_valid()
        || !tree.viewport.contains(tree.safe_area, 0.0)
        || tree.nodes.len() > MAX_NODES
        || tree.panels.len() > MAX_PANELS
    {
        return Err(ComparisonError::input(
            ComparisonErrorCode::SemanticMetadataInvalid,
            "semantic tree protocol, coordinate contract, bounds, or collection limit is invalid",
        ));
    }
    validate_id(&tree.target_root_id)?;
    let mut ids = HashSet::new();
    for node in &tree.nodes {
        validate_node(node)?;
        if !ids.insert(node.stable_id.as_str()) {
            return Err(ComparisonError::input(
                ComparisonErrorCode::SemanticIdentityInvalid,
                format!("duplicate semantic stable id {}", node.stable_id),
            ));
        }
    }
    if !ids.contains(tree.target_root_id.as_str()) {
        return Err(ComparisonError::input(
            ComparisonErrorCode::SemanticTargetRootMissing,
            "semantic target root is not present in nodes",
        ));
    }
    for node in &tree.nodes {
        if node
            .parent_id
            .as_deref()
            .is_some_and(|parent| !ids.contains(parent))
        {
            return Err(ComparisonError::input(
                ComparisonErrorCode::SemanticIdentityInvalid,
                format!("semantic node {} references missing parent", node.stable_id),
            ));
        }
    }
    let mut panel_ids = HashSet::new();
    let mut active_focus_scopes = 0_usize;
    for panel in &tree.panels {
        validate_id(&panel.stable_id)?;
        validate_string(&panel.capture_entity)?;
        validate_string(&panel.input_block_reason)?;
        if panel.stable_id.contains(&panel.capture_entity)
            || panel.likely_files.len() > MAX_LIKELY_FILES
        {
            return Err(ComparisonError::input(
                ComparisonErrorCode::SemanticMetadataInvalid,
                format!("semantic panel {} contains invalid limits", panel.stable_id),
            ));
        }
        for value in panel.entity_name.iter().chain(panel.likely_files.iter()) {
            validate_string(value)?;
        }
        if !panel_ids.insert(panel.stable_id.as_str()) {
            return Err(ComparisonError::input(
                ComparisonErrorCode::SemanticIdentityInvalid,
                format!("duplicate semantic panel id {}", panel.stable_id),
            ));
        }
        let compatible_policy = matches!(
            (panel.kind, panel.layer_policy),
            (
                SemanticPanelKind::Page | SemanticPanelKind::Hud,
                SemanticLayerPolicy::Base
            ) | (SemanticPanelKind::Floating, SemanticLayerPolicy::Floating)
                | (SemanticPanelKind::Modal, SemanticLayerPolicy::Modal)
                | (
                    SemanticPanelKind::BlockingOverlay,
                    SemanticLayerPolicy::Blocking
                )
                | (SemanticPanelKind::Toast, SemanticLayerPolicy::Toast)
        ) || (panel.kind == SemanticPanelKind::Floating
            && panel.layer_policy == SemanticLayerPolicy::TransientAboveModal
            && matches!(panel.stable_id.as_str(), "dropdown" | "tooltip"));
        if !compatible_policy {
            return Err(ComparisonError::input(
                ComparisonErrorCode::SemanticMetadataInvalid,
                format!(
                    "panel {} has an incompatible or unapproved layer policy",
                    panel.stable_id
                ),
            ));
        }
        if panel
            .focused_stable_id
            .as_deref()
            .is_some_and(|focused| !ids.contains(focused))
        {
            return Err(ComparisonError::input(
                ComparisonErrorCode::SemanticIdentityInvalid,
                format!(
                    "panel {} references an unknown focused stable id",
                    panel.stable_id
                ),
            ));
        }
        active_focus_scopes += usize::from(panel.visible && panel.active_focus_scope);
    }
    for node in &tree.nodes {
        if node
            .panel_id
            .as_deref()
            .is_some_and(|panel_id| !panel_ids.contains(panel_id))
        {
            return Err(ComparisonError::input(
                ComparisonErrorCode::SemanticIdentityInvalid,
                format!("semantic node {} references missing panel", node.stable_id),
            ));
        }
    }
    if active_focus_scopes > 1 {
        return Err(ComparisonError::input(
            ComparisonErrorCode::SemanticMetadataInvalid,
            "semantic metadata declares multiple active focus scopes",
        ));
    }
    Ok(())
}

fn validate_node(node: &SemanticNode) -> Result<(), ComparisonError> {
    validate_id(&node.stable_id)?;
    validate_string(&node.capture_entity)?;
    validate_string(&node.interaction)?;
    if node.stable_id.contains(&node.capture_entity)
        || !node.bounds.is_valid()
        || !node.clip_bounds.is_valid()
        || node
            .measured_text_bounds
            .is_some_and(|rect| !rect.is_valid())
        || node.likely_files.len() > MAX_LIKELY_FILES
    {
        return Err(ComparisonError::input(
            ComparisonErrorCode::SemanticMetadataInvalid,
            format!(
                "semantic node {} contains invalid bounds or limits",
                node.stable_id
            ),
        ));
    }
    for value in [
        node.entity_name.as_deref(),
        node.document_id.as_deref(),
        node.node_id.as_deref(),
        node.source_path.as_deref(),
        node.panel_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    .chain(node.likely_files.iter().map(String::as_str))
    {
        validate_string(value)?;
    }
    if node.identity_source == IdentitySource::DeclarativeNode
        && (node.document_id.is_none() || node.node_id.is_none() || node.source_path.is_none())
    {
        return Err(ComparisonError::input(
            ComparisonErrorCode::SemanticIdentityInvalid,
            format!("declarative node {} lacks source identity", node.stable_id),
        ));
    }
    if let Some(scroll) = &node.scroll
        && (!scroll.viewport_height.is_finite()
            || !scroll.content_height.is_finite()
            || !scroll.max_offset.is_finite()
            || !scroll.current_offset.is_finite()
            || scroll.viewport_height < 0.0
            || scroll.content_height < 0.0
            || scroll.max_offset < 0.0
            || scroll.current_offset < 0.0)
    {
        return Err(ComparisonError::input(
            ComparisonErrorCode::SemanticMetadataInvalid,
            format!(
                "semantic node {} has invalid scroll metrics",
                node.stable_id
            ),
        ));
    }
    Ok(())
}

fn validate_id(value: &str) -> Result<(), ComparisonError> {
    validate_string(value)?;
    if value.trim() != value || value.contains(char::is_whitespace) {
        return Err(ComparisonError::input(
            ComparisonErrorCode::SemanticIdentityInvalid,
            "semantic identity contains whitespace",
        ));
    }
    Ok(())
}

fn validate_string(value: &str) -> Result<(), ComparisonError> {
    if value.is_empty() || value.len() > MAX_STRING_BYTES || value.contains(['\0', '\n', '\r']) {
        return Err(ComparisonError::input(
            ComparisonErrorCode::SemanticMetadataInvalid,
            "semantic metadata string is empty, oversized, or contains control characters",
        ));
    }
    Ok(())
}

fn estimate_memory(input_bytes: u64, tree: &SemanticTree) -> Result<u64, ComparisonError> {
    estimate_memory_for_counts(input_bytes, tree.nodes.len(), tree.panels.len())
}

fn estimate_memory_for_counts(
    input_bytes: u64,
    node_count: usize,
    panel_count: usize,
) -> Result<u64, ComparisonError> {
    input_bytes
        .checked_mul(3)
        .and_then(|value| value.checked_add((node_count as u64).checked_mul(2048)?))
        .and_then(|value| value.checked_add((panel_count as u64).checked_mul(1024)?))
        .and_then(|value| {
            value.checked_add((node_count as u64).checked_mul(ESTIMATED_TEXT_CANDIDATE_BYTES)?)
        })
        .and_then(|value| {
            value.checked_add((MAX_SEMANTIC_FINDINGS as u64).checked_mul(ESTIMATED_FINDING_BYTES)?)
        })
        .ok_or_else(|| {
            ComparisonError::input(
                ComparisonErrorCode::SemanticMetadataTooLarge,
                "semantic audit memory estimate overflowed",
            )
        })
}

struct FindingCollector {
    findings: Vec<SemanticFinding>,
}

impl FindingCollector {
    fn new() -> Self {
        Self {
            findings: Vec::with_capacity(MAX_SEMANTIC_FINDINGS),
        }
    }

    fn push(&mut self, finding: SemanticFinding) -> Result<(), ComparisonError> {
        if self.findings.len() >= MAX_SEMANTIC_FINDINGS {
            return Err(ComparisonError::input(
                ComparisonErrorCode::SemanticFindingsLimitExceeded,
                format!("semantic audit exceeded the fixed {MAX_SEMANTIC_FINDINGS}-finding limit"),
            ));
        }
        self.findings.push(finding);
        Ok(())
    }

    fn as_slice(&self) -> &[SemanticFinding] {
        &self.findings
    }

    fn into_findings(self) -> Vec<SemanticFinding> {
        self.findings
    }
}

fn evaluate_semantics(
    tree: &SemanticTree,
    config: &SemanticAuditConfig,
) -> Result<(Vec<SemanticFinding>, SemanticRuleSummary), ComparisonError> {
    let mut findings = FindingCollector::new();
    let mut evaluated = 0;
    let mut skipped_invisible = 0;
    let mut skipped_clipped = 0;
    let mut skipped_layout = 0;
    let nodes_by_id = tree
        .nodes
        .iter()
        .map(|node| (node.stable_id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    let safe_roles = config
        .require_safe_area_for_roles
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();

    for node in &tree.nodes {
        if !node.visible {
            skipped_invisible += 1;
            continue;
        }
        if node.role == SemanticNodeRole::Layout {
            skipped_layout += 1;
            continue;
        }
        let semantic_size_required = node.role.is_actionable()
            || node.role == SemanticNodeRole::CriticalText
            || node.role == SemanticNodeRole::Scroll
            || node.role == SemanticNodeRole::Image
            || matches!(
                node.role,
                SemanticNodeRole::Modal
                    | SemanticNodeRole::Loading
                    | SemanticNodeRole::Floating
                    | SemanticNodeRole::Toast
            )
            || (node.role == SemanticNodeRole::Text && node.text_nonempty);
        if semantic_size_required
            && (node.bounds.width() <= config.geometry_epsilon
                || node.bounds.height() <= config.geometry_epsilon)
        {
            push_node_finding(
                &mut findings,
                SemanticFindingCode::SemanticNodeZeroSize,
                node,
                "semantic node has an effectively zero extent",
                None,
                Some(node.bounds),
            )?;
        }
        if node.fully_clipped {
            skipped_clipped += 1;
            continue;
        }
        evaluated += 1;
        if safe_roles.contains(&node.role)
            && !tree
                .safe_area
                .contains(node.bounds, config.geometry_epsilon)
        {
            push_node_finding(
                &mut findings,
                SemanticFindingCode::SafeAreaOverflow,
                node,
                "semantic node extends outside the declared safe area",
                None,
                Some(node.bounds),
            )?;
        }
        if node.role == SemanticNodeRole::CriticalText
            && node
                .measured_text_bounds
                .is_some_and(|text| !node.clip_bounds.contains(text, config.geometry_epsilon))
        {
            push_node_finding(
                &mut findings,
                SemanticFindingCode::CriticalTextClipped,
                node,
                "critical text measurement extends outside its effective clip",
                None,
                node.measured_text_bounds,
            )?;
        }
        if node.role.is_actionable() {
            if node.bounds.width() + config.geometry_epsilon < config.minimum_touch_width
                || node.bounds.height() + config.geometry_epsilon < config.minimum_touch_height
            {
                push_node_finding(
                    &mut findings,
                    SemanticFindingCode::TouchTargetTooSmall,
                    node,
                    "actionable control is smaller than the configured touch target",
                    None,
                    Some(node.bounds),
                )?;
            }
            if !node.has_visible_label {
                push_node_finding(
                    &mut findings,
                    SemanticFindingCode::VisibleLabelMissing,
                    node,
                    "actionable control lacks visible or accessible label evidence",
                    None,
                    Some(node.bounds),
                )?;
            }
            if node.disabled && (node.focused || node.interaction != "none") {
                push_node_finding(
                    &mut findings,
                    SemanticFindingCode::DisabledStateInconsistent,
                    node,
                    "disabled control remains focused or interactive",
                    None,
                    Some(node.bounds),
                )?;
            }
            if node.loading && (node.focused || node.interaction != "none") {
                push_node_finding(
                    &mut findings,
                    SemanticFindingCode::LoadingStateInconsistent,
                    node,
                    "loading control remains focused or interactive",
                    None,
                    Some(node.bounds),
                )?;
            }
        }
        if node.role == SemanticNodeRole::Scroll
            && node.scroll.as_ref().is_some_and(|scroll| {
                scroll.content_height > scroll.viewport_height + config.geometry_epsilon
                    && (!scroll.content_reachable
                        || scroll.current_offset > scroll.max_offset + config.geometry_epsilon)
            })
        {
            push_node_finding(
                &mut findings,
                SemanticFindingCode::ScrollContentUnreachable,
                node,
                "scroll content exceeds its viewport but its full range is unreachable",
                None,
                Some(node.bounds),
            )?;
        }
    }

    evaluate_text_overlaps(
        tree,
        &nodes_by_id,
        config.text_overlap_minimum_area,
        &mut findings,
    )?;
    evaluate_panels(tree, &mut findings)?;
    let mut findings_by_code = BTreeMap::new();
    for finding in findings.as_slice() {
        *findings_by_code.entry(finding.code).or_default() += 1;
    }
    Ok((
        findings.into_findings(),
        SemanticRuleSummary {
            evaluated_visible_nodes: evaluated,
            skipped_invisible_nodes: skipped_invisible,
            skipped_fully_clipped_nodes: skipped_clipped,
            skipped_layout_only_nodes: skipped_layout,
            hard_failure_count: findings_by_code.values().sum(),
            findings_by_code,
        },
    ))
}

#[derive(Clone, Copy)]
struct VisibleText<'a> {
    node: &'a SemanticNode,
    rect: SemanticRect,
}

#[derive(Clone, Copy)]
enum SweepAxis {
    X,
    Y,
}

impl SweepAxis {
    fn start(self, rect: SemanticRect) -> f64 {
        match self {
            Self::X => rect.min_x,
            Self::Y => rect.min_y,
        }
    }

    fn end(self, rect: SemanticRect) -> f64 {
        match self {
            Self::X => rect.max_x,
            Self::Y => rect.max_y,
        }
    }

    fn secondary_start(self, rect: SemanticRect) -> f64 {
        match self {
            Self::X => rect.min_y,
            Self::Y => rect.min_x,
        }
    }
}

fn projected_candidate_pairs(texts: &[VisibleText<'_>], axis: SweepAxis) -> u64 {
    let mut ordered = texts.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.node
            .panel_id
            .cmp(&right.node.panel_id)
            .then_with(|| axis.start(left.rect).total_cmp(&axis.start(right.rect)))
            .then_with(|| axis.end(left.rect).total_cmp(&axis.end(right.rect)))
            .then_with(|| left.node.stable_id.cmp(&right.node.stable_id))
    });

    let mut total = 0_u64;
    let mut group_start = 0_usize;
    while group_start < ordered.len() {
        let panel_id = &ordered[group_start].node.panel_id;
        let group_end = ordered[group_start..]
            .iter()
            .position(|text| text.node.panel_id != *panel_id)
            .map_or(ordered.len(), |offset| group_start + offset);
        let mut ends = ordered[group_start..group_end]
            .iter()
            .map(|text| axis.end(text.rect))
            .collect::<Vec<_>>();
        ends.sort_by(f64::total_cmp);
        let mut expired = 0_usize;
        for (offset, text) in ordered[group_start..group_end].iter().enumerate() {
            let start = axis.start(text.rect);
            while expired < ends.len() && ends[expired] <= start {
                expired += 1;
            }
            let active = offset.saturating_sub(expired);
            total = total.saturating_add(active as u64);
        }
        group_start = group_end;
    }
    total
}

fn evaluate_text_overlaps(
    tree: &SemanticTree,
    nodes_by_id: &BTreeMap<&str, &SemanticNode>,
    minimum_area: f64,
    findings: &mut FindingCollector,
) -> Result<usize, ComparisonError> {
    let mut texts = tree
        .nodes
        .iter()
        .filter(|node| {
            node.visible && !node.fully_clipped && node.role.is_text() && node.text_nonempty
        })
        .filter_map(|node| {
            node.measured_text_bounds
                .and_then(|bounds| bounds.intersection(node.clip_bounds))
                .map(|rect| VisibleText { node, rect })
        })
        .collect::<Vec<_>>();
    let x_candidates = projected_candidate_pairs(&texts, SweepAxis::X);
    let y_candidates = projected_candidate_pairs(&texts, SweepAxis::Y);
    let projected_candidates = x_candidates.min(y_candidates);
    if projected_candidates > MAX_SEMANTIC_OVERLAP_CANDIDATES as u64 {
        return Err(ComparisonError::input(
            ComparisonErrorCode::SemanticOverlapCandidateLimitExceeded,
            format!(
                "semantic text overlap projected {projected_candidates} candidates, exceeding the fixed {MAX_SEMANTIC_OVERLAP_CANDIDATES}-candidate limit"
            ),
        ));
    }
    let sweep_axis = if x_candidates <= y_candidates {
        SweepAxis::X
    } else {
        SweepAxis::Y
    };
    texts.sort_by(|left, right| {
        left.node
            .panel_id
            .cmp(&right.node.panel_id)
            .then_with(|| {
                sweep_axis
                    .start(left.rect)
                    .total_cmp(&sweep_axis.start(right.rect))
            })
            .then_with(|| {
                sweep_axis
                    .end(left.rect)
                    .total_cmp(&sweep_axis.end(right.rect))
            })
            .then_with(|| {
                sweep_axis
                    .secondary_start(left.rect)
                    .total_cmp(&sweep_axis.secondary_start(right.rect))
            })
            .then_with(|| left.node.stable_id.cmp(&right.node.stable_id))
    });

    let mut active = Vec::<VisibleText<'_>>::new();
    let mut candidate_pairs = 0_usize;
    for current in texts {
        active.retain(|candidate| {
            candidate.node.panel_id == current.node.panel_id
                && sweep_axis.end(candidate.rect) > sweep_axis.start(current.rect)
        });
        for candidate in &active {
            if candidate_pairs >= MAX_SEMANTIC_OVERLAP_CANDIDATES {
                return Err(ComparisonError::input(
                    ComparisonErrorCode::SemanticOverlapCandidateLimitExceeded,
                    format!(
                        "semantic text overlap exceeded the fixed {MAX_SEMANTIC_OVERLAP_CANDIDATES}-candidate limit"
                    ),
                ));
            }
            candidate_pairs = candidate_pairs.saturating_add(1);
            if is_ancestor(candidate.node, current.node, nodes_by_id)
                || is_ancestor(current.node, candidate.node, nodes_by_id)
            {
                continue;
            }
            let Some(overlap) = candidate.rect.intersection(current.rect) else {
                continue;
            };
            if overlap.width() * overlap.height() <= minimum_area {
                continue;
            }
            let (primary, related) = if candidate.node.stable_id <= current.node.stable_id {
                (candidate.node, current.node)
            } else {
                (current.node, candidate.node)
            };
            push_node_finding(
                findings,
                SemanticFindingCode::TextOverlap,
                primary,
                "visible text measurements overlap",
                Some(related.stable_id.clone()),
                Some(overlap),
            )?;
        }
        active.push(current);
    }
    Ok(candidate_pairs)
}

fn evaluate_panels(
    tree: &SemanticTree,
    findings: &mut FindingCollector,
) -> Result<(), ComparisonError> {
    let panels = tree
        .panels
        .iter()
        .filter(|panel| panel.visible)
        .collect::<Vec<_>>();
    for panel in &panels {
        let location = panel_location(panel);
        match panel.kind {
            SemanticPanelKind::Modal | SemanticPanelKind::BlockingOverlay => {
                if panel.active_focus_scope
                    && (!panel.focus_scope_enforced
                        || (panel.has_focusable_descendants && !panel.focused_descendant)
                        || (!panel.has_focusable_descendants && !panel.focus_suppressed))
                {
                    findings.push(SemanticFinding {
                        code: SemanticFindingCode::OverlayFocusScopeInvalid,
                        severity: SemanticSeverity::HardFailure,
                        message: "modal or loading overlay does not contain and restrict focus"
                            .to_owned(),
                        primary: location.clone(),
                        related_stable_id: None,
                        evidence_rect: None,
                    })?;
                }
                if !panel.blocks_lower_input
                    || !panel.pickable_blocks_lower
                    || panel.input_block_reason == "none"
                {
                    findings.push(SemanticFinding {
                        code: SemanticFindingCode::OverlayInputBlockingInvalid,
                        severity: SemanticSeverity::HardFailure,
                        message: "modal or loading overlay does not block lower input".to_owned(),
                        primary: location,
                        related_stable_id: None,
                        evidence_rect: None,
                    })?;
                }
            }
            SemanticPanelKind::Floating => {
                if panel.active_focus_scope
                    && (!panel.focus_scope_enforced
                        || (panel.has_focusable_descendants && !panel.focused_descendant)
                        || (!panel.has_focusable_descendants && !panel.focus_suppressed))
                {
                    findings.push(SemanticFinding {
                        code: SemanticFindingCode::OverlayFocusScopeInvalid,
                        severity: SemanticSeverity::HardFailure,
                        message: "focused floating panel does not enforce its focus scope"
                            .to_owned(),
                        primary: location.clone(),
                        related_stable_id: None,
                        evidence_rect: None,
                    })?;
                }
                if panel.stable_id == "tooltip" && panel.pickable_blocks_lower {
                    findings.push(SemanticFinding {
                        code: SemanticFindingCode::OverlayInputBlockingInvalid,
                        severity: SemanticSeverity::HardFailure,
                        message: "tooltip subtree unexpectedly blocks lower picking".to_owned(),
                        primary: location,
                        related_stable_id: None,
                        evidence_rect: None,
                    })?;
                }
            }
            SemanticPanelKind::Toast => {
                if panel.blocks_lower_input
                    || panel.pickable_blocks_lower
                    || panel.focus_scope_enforced
                    || panel.active_focus_scope
                {
                    findings.push(SemanticFinding {
                        code: SemanticFindingCode::OverlayInputBlockingInvalid,
                        severity: SemanticSeverity::HardFailure,
                        message: "toast unexpectedly blocks input or captures focus".to_owned(),
                        primary: location,
                        related_stable_id: None,
                        evidence_rect: None,
                    })?;
                }
            }
            SemanticPanelKind::Page | SemanticPanelKind::Hud => {}
        }
    }
    for upper in &panels {
        for lower in &panels {
            if upper.stable_id == lower.stable_id {
                continue;
            }
            if layer_rank(upper.layer_policy) > layer_rank(lower.layer_policy)
                && upper.z_index <= lower.z_index
            {
                findings.push(SemanticFinding {
                    code: SemanticFindingCode::OverlayZOrderInvalid,
                    severity: SemanticSeverity::HardFailure,
                    message: "higher semantic overlay layer does not have a greater z-index"
                        .to_owned(),
                    primary: panel_location(upper),
                    related_stable_id: Some(lower.stable_id.clone()),
                    evidence_rect: None,
                })?;
            }
        }
    }
    Ok(())
}

fn layer_rank(policy: SemanticLayerPolicy) -> u8 {
    match policy {
        SemanticLayerPolicy::Base => 0,
        SemanticLayerPolicy::Floating => 1,
        SemanticLayerPolicy::Modal => 2,
        SemanticLayerPolicy::TransientAboveModal => 3,
        SemanticLayerPolicy::Blocking => 4,
        SemanticLayerPolicy::Toast => 5,
    }
}

fn panel_location(panel: &SemanticPanel) -> SemanticLocation {
    SemanticLocation {
        stable_id: panel.stable_id.clone(),
        capture_entity: panel.capture_entity.clone(),
        entity_name: panel.entity_name.clone(),
        document_id: None,
        node_id: None,
        source_path: None,
        panel_id: Some(panel.stable_id.clone()),
        likely_files: panel.likely_files.clone(),
    }
}

fn is_ancestor(
    possible_ancestor: &SemanticNode,
    node: &SemanticNode,
    nodes: &BTreeMap<&str, &SemanticNode>,
) -> bool {
    let mut current = node.parent_id.as_deref();
    let mut remaining = node.depth.saturating_add(1);
    while let Some(id) = current {
        if id == possible_ancestor.stable_id {
            return true;
        }
        if remaining == 0 {
            return false;
        }
        current = nodes.get(id).and_then(|parent| parent.parent_id.as_deref());
        remaining -= 1;
    }
    false
}

fn push_node_finding(
    findings: &mut FindingCollector,
    code: SemanticFindingCode,
    node: &SemanticNode,
    message: &str,
    related_stable_id: Option<String>,
    evidence_rect: Option<SemanticRect>,
) -> Result<(), ComparisonError> {
    findings.push(SemanticFinding {
        code,
        severity: SemanticSeverity::HardFailure,
        message: message.to_owned(),
        primary: SemanticLocation {
            stable_id: node.stable_id.clone(),
            capture_entity: node.capture_entity.clone(),
            entity_name: node.entity_name.clone(),
            document_id: node.document_id.clone(),
            node_id: node.node_id.clone(),
            source_path: node.source_path.clone(),
            panel_id: node.panel_id.clone(),
            likely_files: node.likely_files.clone(),
        },
        related_stable_id,
        evidence_rect,
    })
}

fn persist_report(path: &Path, report: &SemanticAuditReport) -> Result<(), ComparisonError> {
    let mut bytes = serde_json::to_vec_pretty(report).map_err(|error| {
        ComparisonError::internal(
            ComparisonErrorCode::ArtifactWriteFailed,
            format!("semantic report cannot be serialized: {error}"),
        )
    })?;
    bytes.push(b'\n');
    let temporary = path.with_extension("json.tmp");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| artifact_error(&temporary, error))?;
    if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
        let _ = fs::remove_file(&temporary);
        return Err(artifact_error(&temporary, error));
    }
    drop(file);
    if let Err(error) = fs::hard_link(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(artifact_error(path, error));
    }
    if let Err(error) = fs::remove_file(&temporary) {
        let _ = fs::remove_file(path);
        return Err(artifact_error(&temporary, error));
    }
    Ok(())
}

fn artifact_error(path: &Path, error: std::io::Error) -> ComparisonError {
    ComparisonError::internal(
        ComparisonErrorCode::ArtifactWriteFailed,
        format!("semantic audit artifact cannot be written: {error}"),
    )
    .at_path(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, width: f64, height: f64) -> SemanticRect {
        SemanticRect {
            min_x: x,
            min_y: y,
            max_x: x + width,
            max_y: y + height,
        }
    }

    fn text_node(index: usize, bounds: SemanticRect) -> SemanticNode {
        SemanticNode {
            stable_id: format!("panel:page/root/text[{index}]"),
            identity_source: IdentitySource::HierarchyFallback,
            capture_entity: format!("{index}v1#test"),
            entity_name: Some(format!("Text{index}")),
            stack_index: index as u32,
            parent_id: None,
            depth: 0,
            role: SemanticNodeRole::Text,
            visible: true,
            fully_clipped: false,
            bounds,
            clip_bounds: bounds,
            measured_text_bounds: Some(bounds),
            text_nonempty: true,
            has_visible_label: true,
            interaction: "none".to_owned(),
            disabled: false,
            loading: false,
            focused: false,
            scroll: None,
            document_id: None,
            node_id: None,
            source_path: None,
            panel_id: Some("page".to_owned()),
            likely_files: vec!["project/src/game/screens/page.rs".to_owned()],
        }
    }

    fn tree_with_nodes(nodes: Vec<SemanticNode>) -> SemanticTree {
        SemanticTree {
            schema_version: SEMANTIC_TREE_SCHEMA_VERSION,
            coordinate_space: "logical_pixels".to_owned(),
            rect_convention: "half_open".to_owned(),
            rounding: "nearest_1_64_half_away_from_zero".to_owned(),
            target_root_id: nodes
                .first()
                .map_or_else(|| "root".to_owned(), |node| node.stable_id.clone()),
            viewport: rect(0.0, 0.0, 1_000_000.0, 1_000_000.0),
            safe_area: rect(0.0, 0.0, 1_000_000.0, 1_000_000.0),
            nodes,
            panels: Vec::new(),
        }
    }

    fn test_finding(index: usize) -> SemanticFinding {
        SemanticFinding {
            code: SemanticFindingCode::TextOverlap,
            severity: SemanticSeverity::HardFailure,
            message: "test".to_owned(),
            primary: SemanticLocation {
                stable_id: format!("node-{index}"),
                capture_entity: "capture".to_owned(),
                entity_name: Some("Text".to_owned()),
                document_id: None,
                node_id: None,
                source_path: None,
                panel_id: Some("page".to_owned()),
                likely_files: Vec::new(),
            },
            related_stable_id: None,
            evidence_rect: None,
        }
    }

    #[test]
    fn half_open_rect_intersection_excludes_touching_edges() {
        assert_eq!(
            rect(0.0, 0.0, 10.0, 10.0).intersection(rect(10.0, 0.0, 5.0, 5.0)),
            None
        );
        assert_eq!(
            rect(0.0, 0.0, 10.0, 10.0).intersection(rect(9.0, 9.0, 3.0, 3.0)),
            Some(rect(9.0, 9.0, 1.0, 1.0))
        );
    }

    #[test]
    fn memory_estimate_is_checked_and_pins_the_public_budget_boundary() {
        let fixed_findings = (MAX_SEMANTIC_FINDINGS as u64) * ESTIMATED_FINDING_BYTES;
        let per_node = 2_048 + ESTIMATED_TEXT_CANDIDATE_BYTES;
        let boundary_nodes =
            ((SEMANTIC_AUDIT_PEAK_MEMORY_BUDGET_BYTES - fixed_findings) / per_node) as usize;
        assert_eq!(estimate_memory_for_counts(0, 0, 0).unwrap(), fixed_findings);
        assert!(
            estimate_memory_for_counts(0, boundary_nodes, 0).unwrap()
                <= SEMANTIC_AUDIT_PEAK_MEMORY_BUDGET_BYTES
        );
        assert!(
            estimate_memory_for_counts(0, boundary_nodes + 1, 0).unwrap()
                > SEMANTIC_AUDIT_PEAK_MEMORY_BUDGET_BYTES
        );
        assert_eq!(
            estimate_memory_for_counts(u64::MAX, usize::MAX, usize::MAX)
                .unwrap_err()
                .failure
                .code,
            ComparisonErrorCode::SemanticMetadataTooLarge
        );
    }

    #[test]
    fn finding_collector_accepts_the_boundary_and_rejects_the_next_finding() {
        let mut findings = FindingCollector::new();
        for index in 0..MAX_SEMANTIC_FINDINGS {
            findings.push(test_finding(index)).unwrap();
        }
        assert_eq!(findings.as_slice().len(), MAX_SEMANTIC_FINDINGS);
        let error = findings
            .push(test_finding(MAX_SEMANTIC_FINDINGS))
            .unwrap_err();
        assert_eq!(
            error.failure.code,
            ComparisonErrorCode::SemanticFindingsLimitExceeded
        );
    }

    #[test]
    fn sweep_skips_candidate_pairs_for_many_non_overlapping_texts_on_either_axis() {
        for vertical in [false, true] {
            let nodes = (0..20_000)
                .map(|index| {
                    let offset = index as f64 * 20.0;
                    let bounds = if vertical {
                        rect(20.0, offset, 10.0, 10.0)
                    } else {
                        rect(offset, 20.0, 10.0, 10.0)
                    };
                    text_node(index, bounds)
                })
                .collect::<Vec<_>>();
            let tree = tree_with_nodes(nodes);
            let nodes_by_id = tree
                .nodes
                .iter()
                .map(|node| (node.stable_id.as_str(), node))
                .collect::<BTreeMap<_, _>>();
            let mut findings = FindingCollector::new();
            let candidate_pairs =
                evaluate_text_overlaps(&tree, &nodes_by_id, 1.0, &mut findings).unwrap();
            assert_eq!(candidate_pairs, 0);
            assert!(findings.as_slice().is_empty());
        }
    }

    #[test]
    fn overlapping_texts_fail_with_the_same_stable_limit_error() {
        let nodes = (0..47)
            .map(|index| text_node(index, rect(20.0, 20.0, 100.0, 30.0)))
            .collect::<Vec<_>>();
        let first = tree_with_nodes(nodes.clone());
        let mut reversed_nodes = nodes;
        reversed_nodes.reverse();
        let second = tree_with_nodes(reversed_nodes);

        let evaluate = |tree: &SemanticTree| {
            let nodes_by_id = tree
                .nodes
                .iter()
                .map(|node| (node.stable_id.as_str(), node))
                .collect::<BTreeMap<_, _>>();
            evaluate_text_overlaps(tree, &nodes_by_id, 1.0, &mut FindingCollector::new())
                .unwrap_err()
        };
        let first_error = evaluate(&first);
        let second_error = evaluate(&second);
        assert_eq!(first_error.failure, second_error.failure);
        assert_eq!(
            first_error.failure.code,
            ComparisonErrorCode::SemanticFindingsLimitExceeded
        );
    }

    #[test]
    fn interleaved_non_overlaps_hit_the_candidate_cap_before_quadratic_scanning() {
        let group_size = 130_usize;
        let mut nodes = Vec::with_capacity(group_size * 2);
        for index in 0..group_size {
            nodes.push(text_node(
                index,
                rect(0.0, index as f64 * 20.0, 1_000.0, 10.0),
            ));
            nodes.push(text_node(
                group_size + index,
                rect(10_000.0 + index as f64 * 20.0, 100_000.0, 10.0, 1_000.0),
            ));
        }
        let tree = tree_with_nodes(nodes);
        let nodes_by_id = tree
            .nodes
            .iter()
            .map(|node| (node.stable_id.as_str(), node))
            .collect::<BTreeMap<_, _>>();
        let error = evaluate_text_overlaps(&tree, &nodes_by_id, 1.0, &mut FindingCollector::new())
            .unwrap_err();
        assert_eq!(
            error.failure.code,
            ComparisonErrorCode::SemanticOverlapCandidateLimitExceeded
        );
    }

    #[test]
    fn panel_pairs_share_the_finding_limit() {
        let mut tree = tree_with_nodes(Vec::new());
        tree.panels = (0..100)
            .map(|index| {
                let toast = index < 50;
                SemanticPanel {
                    stable_id: format!("panel-{index:03}"),
                    capture_entity: format!("panel-entity-{index:03}"),
                    entity_name: Some(format!("Panel{index:03}")),
                    likely_files: vec![if toast {
                        "project/src/framework/ui/overlays/toast.rs".to_owned()
                    } else {
                        "project/src/game/screens/mod.rs".to_owned()
                    }],
                    kind: if toast {
                        SemanticPanelKind::Toast
                    } else {
                        SemanticPanelKind::Page
                    },
                    layer_policy: if toast {
                        SemanticLayerPolicy::Toast
                    } else {
                        SemanticLayerPolicy::Base
                    },
                    visible: true,
                    z_index: if toast { 0 } else { 1 },
                    has_focusable_descendants: false,
                    focused_descendant: false,
                    focused_stable_id: None,
                    active_focus_scope: false,
                    focus_scope_enforced: false,
                    focus_suppressed: true,
                    blocks_lower_input: false,
                    pickable_blocks_lower: false,
                    input_block_reason: "none".to_owned(),
                }
            })
            .collect();
        let error = evaluate_panels(&tree, &mut FindingCollector::new()).unwrap_err();
        assert_eq!(
            error.failure.code,
            ComparisonErrorCode::SemanticFindingsLimitExceeded
        );
    }

    #[test]
    fn transaction_preserves_existing_final_and_temporary_files() {
        let temporary = tempfile::tempdir().unwrap();
        let final_path = temporary.path().join(SEMANTIC_AUDIT_REPORT_FILENAME);
        fs::write(&final_path, b"existing").unwrap();
        let error = persist_report(
            &final_path,
            &SemanticAuditReport {
                schema_version: SEMANTIC_AUDIT_REPORT_SCHEMA_VERSION,
                algorithm_version: SEMANTIC_AUDIT_ALGORITHM_VERSION.to_owned(),
                status: SemanticAuditStatus::Passed,
                input: SemanticInputReport {
                    path: "fixture".to_owned(),
                    byte_length: 1,
                    metadata_sha256: "0".repeat(64),
                    device_profile: "compact".to_owned(),
                    target_root_id: "root".to_owned(),
                    node_count: 0,
                    panel_count: 0,
                },
                coordinate_space: "logical_pixels".to_owned(),
                rect_convention: "half_open".to_owned(),
                rounding: "nearest_1_64_half_away_from_zero".to_owned(),
                rules: SemanticRuleSummary {
                    evaluated_visible_nodes: 0,
                    skipped_invisible_nodes: 0,
                    skipped_fully_clipped_nodes: 0,
                    skipped_layout_only_nodes: 0,
                    hard_failure_count: 0,
                    findings_by_code: BTreeMap::new(),
                },
                separation: SemanticSeparationContract {
                    semantic_hard_failure: false,
                    visual_similarity_consumed: false,
                    local_visual_scores_consumed: false,
                    can_visual_score_offset_hard_failure: false,
                },
                findings: Vec::new(),
                performance: SemanticPerformanceReport {
                    estimated_peak_memory_bytes: 1,
                    budget_bytes: SEMANTIC_AUDIT_PEAK_MEMORY_BUDGET_BYTES,
                    memory_basis: "test".to_owned(),
                },
                artifacts: Vec::new(),
            },
        )
        .unwrap_err();
        assert_eq!(error.failure.code, ComparisonErrorCode::ArtifactWriteFailed);
        assert_eq!(fs::read(&final_path).unwrap(), b"existing");

        fs::remove_file(&final_path).unwrap();
        let temp_path = final_path.with_extension("json.tmp");
        fs::write(&temp_path, b"unknown").unwrap();
        let error = persist_report(
            &final_path,
            &serde_json::from_str::<SemanticAuditReport>(
                r#"{"schema_version":3,"algorithm_version":"ui_semantic_audit_v1","status":"passed","input":{"path":"f","byte_length":1,"metadata_sha256":"0000000000000000000000000000000000000000000000000000000000000000","device_profile":"compact","target_root_id":"root","node_count":0,"panel_count":0},"coordinate_space":"logical_pixels","rect_convention":"half_open","rounding":"nearest_1_64_half_away_from_zero","rules":{"evaluated_visible_nodes":0,"skipped_invisible_nodes":0,"skipped_fully_clipped_nodes":0,"skipped_layout_only_nodes":0,"hard_failure_count":0,"findings_by_code":{}},"separation":{"semantic_hard_failure":false,"visual_similarity_consumed":false,"local_visual_scores_consumed":false,"can_visual_score_offset_hard_failure":false},"findings":[],"performance":{"estimated_peak_memory_bytes":1,"budget_bytes":67108864,"memory_basis":"test"},"artifacts":[]}"#,
            )
            .unwrap(),
        )
        .unwrap_err();
        assert_eq!(error.failure.code, ComparisonErrorCode::ArtifactWriteFailed);
        assert_eq!(fs::read(&temp_path).unwrap(), b"unknown");
        assert!(!final_path.exists());
    }
}
