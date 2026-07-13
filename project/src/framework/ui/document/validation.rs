use super::{
    CURRENT_SCHEMA_VERSION, MIN_SUPPORTED_SCHEMA_VERSION, UI_DOCUMENT_MAX_BYTES,
    UiBindingActionError, UiContentFieldError, UiControlFieldError, UiDocument, UiDocumentId,
    UiDocumentValidationResult, UiLayoutFieldError, UiNode, UiNodeId, UiResponsiveStateError,
    UiValidationDiagnostic, UiValidationPhase, UiValidationReport, UiVisualFieldError,
    analyze_document_budget, parse_duplicate_checked_json,
};
use bevy::prelude::Component;
use serde::Serialize;
use std::{collections::BTreeMap, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiDocumentError {
    Parse {
        message: String,
    },
    InvalidSchemaVersion,
    FutureSchemaVersion {
        found: u32,
        current: u32,
    },
    UnsupportedSchemaVersion {
        found: u32,
        minimum: u32,
    },
    DuplicateNodeId {
        node_id: UiNodeId,
        first_path: String,
        duplicate_path: String,
    },
    InvalidLayout {
        errors: Vec<UiLayoutFieldError>,
    },
    InvalidVisual {
        errors: Vec<UiVisualFieldError>,
    },
    InvalidContent {
        errors: Vec<UiContentFieldError>,
    },
    InvalidControl {
        errors: Vec<UiControlFieldError>,
    },
    InvalidBindingAction {
        errors: Vec<super::UiBindingActionError>,
    },
    InvalidResponsiveState {
        errors: Vec<super::UiResponsiveStateError>,
    },
    ValidationReport {
        report: UiValidationReport,
    },
}

impl UiDocumentError {
    pub fn code(&self) -> &str {
        match self {
            Self::Parse { .. } => "UI_DOCUMENT_PARSE_FAILED",
            Self::InvalidSchemaVersion => "UI_SCHEMA_VERSION_INVALID",
            Self::FutureSchemaVersion { .. } => "UI_SCHEMA_FUTURE_VERSION",
            Self::UnsupportedSchemaVersion { .. } => "UI_SCHEMA_VERSION_UNSUPPORTED",
            Self::DuplicateNodeId { .. } => "UI_NODE_ID_DUPLICATE",
            Self::InvalidLayout { .. } => "UI_LAYOUT_INVALID",
            Self::InvalidVisual { .. } => "UI_VISUAL_INVALID",
            Self::InvalidContent { .. } => "UI_CONTENT_INVALID",
            Self::InvalidControl { .. } => "UI_CONTROL_INVALID",
            Self::InvalidBindingAction { .. } => "UI_BINDING_ACTION_INVALID",
            Self::InvalidResponsiveState { .. } => "UI_RESPONSIVE_STATE_INVALID",
            Self::ValidationReport { report } => report
                .diagnostics
                .first()
                .map_or("UI_DOCUMENT_VALIDATION_FAILED", |diagnostic| {
                    diagnostic.code.as_str()
                }),
        }
    }
}

impl fmt::Display for UiDocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse { message } => formatter.write_str(message),
            Self::InvalidSchemaVersion => {
                formatter.write_str("schema_version must be a positive 32-bit integer")
            }
            Self::FutureSchemaVersion { found, current } => write!(
                formatter,
                "schema version {found} is newer than current version {current}"
            ),
            Self::UnsupportedSchemaVersion { found, minimum } => write!(
                formatter,
                "schema version {found} is older than minimum supported version {minimum}"
            ),
            Self::DuplicateNodeId {
                node_id,
                first_path,
                duplicate_path,
            } => write!(
                formatter,
                "node id `{node_id}` is duplicated at {duplicate_path}; first defined at {first_path}"
            ),
            Self::InvalidLayout { errors } => {
                write!(
                    formatter,
                    "document contains {} invalid layout field(s)",
                    errors.len()
                )
            }
            Self::InvalidVisual { errors } => write!(
                formatter,
                "document contains {} invalid visual or asset field(s)",
                errors.len()
            ),
            Self::InvalidContent { errors } => write!(
                formatter,
                "document contains {} invalid content field(s)",
                errors.len()
            ),
            Self::InvalidControl { errors } => write!(
                formatter,
                "document contains {} invalid control field(s)",
                errors.len()
            ),
            Self::InvalidBindingAction { errors } => write!(
                formatter,
                "document contains {} invalid binding or action field(s)",
                errors.len()
            ),
            Self::InvalidResponsiveState { errors } => write!(
                formatter,
                "document contains {} invalid responsive or state field(s)",
                errors.len()
            ),
            Self::ValidationReport { report } => write!(
                formatter,
                "document validation failed with {} diagnostic(s)",
                report.diagnostics.len()
            ),
        }
    }
}

impl std::error::Error for UiDocumentError {}

impl UiDocument {
    pub fn validate_json(source: &str) -> UiDocumentValidationResult {
        validate_json_document(source)
    }

    pub fn validate_json_bytes(source: &[u8]) -> UiDocumentValidationResult {
        if source.len() > UI_DOCUMENT_MAX_BYTES {
            let mut report = UiValidationReport::new(source.len());
            report.push(UiValidationDiagnostic::error(
                "UI_DOCUMENT_BYTES_BUDGET_EXCEEDED",
                UiValidationPhase::Budget,
                "$",
                None,
                "$",
            ));
            report.finish();
            return UiDocumentValidationResult::new(report, None);
        }
        match std::str::from_utf8(source) {
            Ok(source) => validate_json_document(source),
            Err(_) => {
                let mut report = UiValidationReport::new(source.len());
                report.push(
                    UiValidationDiagnostic::error(
                        "UI_DOCUMENT_UTF8_INVALID",
                        UiValidationPhase::Syntax,
                        "$",
                        None,
                        "$",
                    )
                    .with_copy(
                        "The JSON source is not valid UTF-8.",
                        "Encode the complete JSON document as UTF-8 before validation.",
                    ),
                );
                report.finish();
                UiDocumentValidationResult::new(report, None)
            }
        }
    }
}

fn validate_json_document(source: &str) -> UiDocumentValidationResult {
    let mut report = UiValidationReport::new(source.len());
    if source.len() > UI_DOCUMENT_MAX_BYTES {
        report.push(UiValidationDiagnostic::error(
            "UI_DOCUMENT_BYTES_BUDGET_EXCEEDED",
            UiValidationPhase::Budget,
            "$",
            None,
            "$",
        ));
        report.finish();
        return UiDocumentValidationResult::new(report, None);
    }

    let raw = match parse_duplicate_checked_json(source) {
        Ok(raw) => raw,
        Err(error) => {
            let (code, message) = match error.classify() {
                serde_json::error::Category::Eof => (
                    "UI_DOCUMENT_JSON_INCOMPLETE",
                    "The JSON document ended before a complete value was parsed.",
                ),
                serde_json::error::Category::Syntax => (
                    "UI_DOCUMENT_JSON_SYNTAX_INVALID",
                    "The source is not syntactically valid JSON.",
                ),
                serde_json::error::Category::Data => (
                    "UI_DOCUMENT_JSON_VALUE_INVALID",
                    "A JSON value cannot be represented by the document parser.",
                ),
                serde_json::error::Category::Io => (
                    "UI_DOCUMENT_JSON_READ_FAILED",
                    "The in-memory JSON source could not be read.",
                ),
            };
            report.push(
                UiValidationDiagnostic::error(code, UiValidationPhase::Syntax, "$", None, "$")
                    .with_copy(
                        format!(
                            "{message} Line {}, column {}.",
                            error.line(),
                            error.column()
                        ),
                        "Produce one complete UTF-8 JSON object without comments or trailing data.",
                    ),
            );
            report.finish();
            return UiDocumentValidationResult::new(report, None);
        }
    };

    for path in &raw.duplicate_paths {
        report.push(
            UiValidationDiagnostic::error(
                "UI_DOCUMENT_DUPLICATE_OBJECT_KEY",
                UiValidationPhase::Syntax,
                "$",
                None,
                path,
            )
            .with_copy(
                "A JSON object contains a duplicate key.",
                "Keep exactly one value for this key; duplicate slot names are not allowed.",
            ),
        );
    }
    report.truncated |= raw.duplicates_truncated;
    if !raw.duplicate_paths.is_empty() {
        report.finish();
        return UiDocumentValidationResult::new(report, None);
    }

    let version = raw
        .value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .and_then(|version| u32::try_from(version).ok())
        .filter(|version| *version > 0);
    match version {
        None => report.push(UiValidationDiagnostic::error(
            "UI_SCHEMA_VERSION_INVALID",
            UiValidationPhase::Structure,
            "$",
            None,
            "$.schema_version",
        )),
        Some(version) if version > CURRENT_SCHEMA_VERSION => {
            report.push(UiValidationDiagnostic::error(
                "UI_SCHEMA_FUTURE_VERSION",
                UiValidationPhase::Structure,
                "$",
                None,
                "$.schema_version",
            ))
        }
        Some(version) if version < MIN_SUPPORTED_SCHEMA_VERSION => {
            report.push(UiValidationDiagnostic::error(
                "UI_SCHEMA_VERSION_UNSUPPORTED",
                UiValidationPhase::Structure,
                "$",
                None,
                "$.schema_version",
            ))
        }
        Some(_) => {}
    }

    report.extend(
        super::validate_content_json_shape(&raw.value)
            .into_iter()
            .map(|error| {
                UiValidationDiagnostic::error(
                    error.code,
                    UiValidationPhase::Structure,
                    "$",
                    None,
                    error.path,
                )
            }),
    );

    let document = match serde_json::from_value::<UiDocument>(raw.value.clone()) {
        Ok(document) => document,
        Err(_) => {
            report.push(
                UiValidationDiagnostic::error(
                    "UI_DOCUMENT_STRUCTURE_INVALID",
                    UiValidationPhase::Structure,
                    "$",
                    None,
                    "$",
                )
                .with_copy(
                    "The JSON value does not match the closed UiDocument v1 structure.",
                    "Use the generated v1 JSON Schema to remove unknown fields and correct field types.",
                ),
            );
            report.finish();
            return UiDocumentValidationResult::new(report, None);
        }
    };
    report.budget_profile = document.metadata.budget_profile.clone();

    let (node_paths, duplicate_nodes) = collect_node_paths(&document.root);
    for (node_id, first_path, duplicate_path) in duplicate_nodes {
        report.push(
            UiValidationDiagnostic::error(
                "UI_NODE_ID_DUPLICATE",
                UiValidationPhase::Structure,
                duplicate_path.clone(),
                Some(node_id),
                format!("{duplicate_path}.id"),
            )
            .with_copy(
                format!("The node ID was already declared at {first_path}."),
                "Give every protocol node a unique stable ID.",
            ),
        );
    }

    let budget = analyze_document_budget(source.len(), &raw.value, &document);
    report.budget_usage = budget.usage;
    report.extend(budget.violations.into_iter().map(|violation| {
        diagnostic_at_path(
            violation.code,
            violation.phase,
            violation.path,
            violation.node_id,
            &node_paths,
        )
    }));

    if report.budget_usage.permits_full_validation() {
        extend_full_validation_report(&mut report, &document, &node_paths);
    }

    report.finish();
    let validated = report.valid.then(|| ValidatedUiDocument {
        document,
        node_paths,
    });
    UiDocumentValidationResult::new(report, validated)
}

fn collect_node_paths(
    root: &UiNode,
) -> (BTreeMap<UiNodeId, String>, Vec<(UiNodeId, String, String)>) {
    let mut node_paths: BTreeMap<UiNodeId, String> = BTreeMap::new();
    let mut duplicates = Vec::new();
    let mut pending = vec![(root, "$.root".to_owned())];
    while let Some((node, path)) = pending.pop() {
        if let Some(first_path) = node_paths.get(node.id()) {
            duplicates.push((node.id().clone(), first_path.clone(), path.clone()));
        } else {
            node_paths.insert(node.id().clone(), path.clone());
        }
        for (index, child) in node.children().iter().enumerate().rev() {
            pending.push((child, node.child_path(&path, index)));
        }
    }
    (node_paths, duplicates)
}

fn extend_full_validation_report(
    report: &mut UiValidationReport,
    document: &UiDocument,
    node_paths: &BTreeMap<UiNodeId, String>,
) {
    let mut layout_errors = Vec::new();
    collect_node_layout_errors(&document.root, "$.root", &mut layout_errors);
    for (state_index, state) in document.states.iter().enumerate() {
        index_override_layouts(
            &state.overrides,
            &format!("$.states[{state_index}].overrides"),
            &mut layout_errors,
        );
    }
    for (variant_index, variant) in document.responsive.iter().enumerate() {
        index_override_layouts(
            &variant.overrides,
            &format!("$.responsive[{variant_index}].overrides"),
            &mut layout_errors,
        );
    }
    report.extend(layout_errors.into_iter().map(|error| {
        diagnostic_at_path(
            error.code,
            UiValidationPhase::Structure,
            error.path,
            None,
            node_paths,
        )
    }));

    report.extend(
        super::validate_responsive_state_document(document, node_paths)
            .into_iter()
            .map(|error| responsive_diagnostic(error, node_paths)),
    );
    report.extend(document.validate_content().into_iter().map(|error| {
        diagnostic_at_path(
            error.code,
            UiValidationPhase::Structure,
            error.path,
            None,
            node_paths,
        )
    }));

    let mut control_errors = Vec::new();
    validate_node_controls(&document.root, "$.root", &mut control_errors);
    report.extend(control_errors.into_iter().map(|error| {
        diagnostic_at_path(
            error.code,
            UiValidationPhase::Structure,
            error.path,
            Some(error.node_id),
            node_paths,
        )
    }));

    let mut visual_errors = document.validate_style_tables();
    visual_errors.extend(document.validate_assets());
    index_node_styles(document, &document.root, "$.root", &mut visual_errors);
    for (state_index, state) in document.states.iter().enumerate() {
        index_override_styles(
            document,
            &state.overrides,
            &format!("$.states[{state_index}].overrides"),
            &mut visual_errors,
        );
    }
    for (variant_index, variant) in document.responsive.iter().enumerate() {
        index_override_styles(
            document,
            &variant.overrides,
            &format!("$.responsive[{variant_index}].overrides"),
            &mut visual_errors,
        );
    }
    report.extend(visual_errors.into_iter().map(|error| {
        let phase = phase_for_code(error.code);
        diagnostic_at_path(error.code, phase, error.path, None, node_paths)
    }));

    report.extend(
        super::validate_binding_action_document(document)
            .into_iter()
            .map(|error| binding_diagnostic(error, node_paths)),
    );
}

fn collect_node_layout_errors(node: &UiNode, path: &str, errors: &mut Vec<UiLayoutFieldError>) {
    errors.extend(
        node.layout()
            .validate_fields()
            .into_iter()
            .map(|error| UiLayoutFieldError {
                code: error.code,
                path: format!("{path}.layout.{}", error.path),
            }),
    );
    for (index, child) in node.children().iter().enumerate() {
        collect_node_layout_errors(child, &node.child_path(path, index), errors);
    }
}

fn responsive_diagnostic(
    error: UiResponsiveStateError,
    node_paths: &BTreeMap<UiNodeId, String>,
) -> UiValidationDiagnostic {
    diagnostic_at_path(
        error.code,
        phase_for_code(error.code),
        error.path,
        error.node_id,
        node_paths,
    )
}

fn binding_diagnostic(
    error: UiBindingActionError,
    node_paths: &BTreeMap<UiNodeId, String>,
) -> UiValidationDiagnostic {
    diagnostic_at_path(
        error.code,
        phase_for_code(error.code),
        error.path,
        error.node_id,
        node_paths,
    )
}

fn diagnostic_at_path(
    code: impl Into<String>,
    phase: UiValidationPhase,
    field_path: String,
    node_id: Option<UiNodeId>,
    node_paths: &BTreeMap<UiNodeId, String>,
) -> UiValidationDiagnostic {
    let document_path = node_id
        .as_ref()
        .and_then(|node_id| node_paths.get(node_id))
        .cloned()
        .or_else(|| {
            node_paths
                .values()
                .filter(|path| path_is_prefix(path, &field_path))
                .max_by_key(|path| path.len())
                .cloned()
        })
        .unwrap_or_else(|| "$".to_owned());
    let node_id = node_id.or_else(|| {
        node_paths
            .iter()
            .filter(|(_, path)| path_is_prefix(path, &field_path))
            .max_by_key(|(_, path)| path.len())
            .map(|(node_id, _)| node_id.clone())
    });
    UiValidationDiagnostic::error(code, phase, document_path, node_id, field_path)
}

fn path_is_prefix(node_path: &str, field_path: &str) -> bool {
    field_path == node_path
        || field_path
            .strip_prefix(node_path)
            .is_some_and(|suffix| suffix.starts_with('.') || suffix.starts_with('['))
}

fn phase_for_code(code: &str) -> UiValidationPhase {
    if code.contains("BUDGET") {
        UiValidationPhase::Budget
    } else if code.contains("FORBIDDEN")
        || code.contains("NOT_ALLOWED")
        || code == "UI_ASSET_PATH_INVALID"
        || code == "UI_ASSET_SOURCE_INVALID"
        || code == "UI_MATERIAL_NOT_ALLOWLISTED"
    {
        UiValidationPhase::Capability
    } else if code.contains("UNKNOWN")
        || code.contains("NOT_FOUND")
        || code.contains("CYCLE")
        || code.contains("KIND_MISMATCH")
        || code == "UI_BINDING_PATH_UNDECLARED"
    {
        UiValidationPhase::Reference
    } else {
        UiValidationPhase::Structure
    }
}

fn report_requires_direct_error(report: &UiValidationReport) -> bool {
    report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "UI_DOCUMENT_DUPLICATE_OBJECT_KEY"
            || matches!(
                diagnostic.code.as_str(),
                "UI_DOCUMENT_BYTES_BUDGET_EXCEEDED"
                    | "UI_DOCUMENT_NODE_COUNT_BUDGET_EXCEEDED"
                    | "UI_DOCUMENT_TREE_DEPTH_BUDGET_EXCEEDED"
                    | "UI_DOCUMENT_CHILDREN_BUDGET_EXCEEDED"
                    | "UI_DOCUMENT_RESPONSIVE_BUDGET_EXCEEDED"
                    | "UI_DOCUMENT_OVERRIDE_BUDGET_EXCEEDED"
            )
    })
}

#[derive(Clone, Debug)]
pub struct ValidatedUiDocument {
    document: UiDocument,
    node_paths: BTreeMap<UiNodeId, String>,
}

impl ValidatedUiDocument {
    pub fn parse_json(source: &str) -> Result<Self, UiDocumentError> {
        let result = UiDocument::validate_json(source);
        if result.report.valid {
            let report = result.report.clone();
            return result
                .into_validated()
                .ok_or(UiDocumentError::ValidationReport { report });
        }
        if report_requires_direct_error(&result.report) {
            return Err(UiDocumentError::ValidationReport {
                report: result.report,
            });
        }
        match Self::parse_json_legacy(source) {
            Err(error) => Err(error),
            Ok(_) => Err(UiDocumentError::ValidationReport {
                report: result.report,
            }),
        }
    }

    fn parse_json_legacy(source: &str) -> Result<Self, UiDocumentError> {
        let value: serde_json::Value =
            serde_json::from_str(source).map_err(|error| UiDocumentError::Parse {
                message: error.to_string(),
            })?;
        let version = value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .and_then(|version| u32::try_from(version).ok())
            .filter(|version| *version > 0)
            .ok_or(UiDocumentError::InvalidSchemaVersion)?;
        validate_version(version)?;
        let content_shape_errors = super::validate_content_json_shape(&value);
        if !content_shape_errors.is_empty() {
            return Err(UiDocumentError::InvalidContent {
                errors: content_shape_errors,
            });
        }
        let document = serde_json::from_value(value).map_err(|error| UiDocumentError::Parse {
            message: error.to_string(),
        })?;
        Self::new(document)
    }

    pub fn new(document: UiDocument) -> Result<Self, UiDocumentError> {
        validate_version(document.schema_version)?;
        let budget_report = typed_document_budget_report(&document)?;
        if let Some(report) = budget_report
            .as_ref()
            .filter(|report| report_requires_direct_error(report))
        {
            return Err(UiDocumentError::ValidationReport {
                report: report.clone(),
            });
        }
        let mut node_paths = BTreeMap::new();
        let mut layout_errors = Vec::new();
        index_node(
            &document.root,
            "$.root",
            &mut node_paths,
            &mut layout_errors,
        )?;
        for (state_index, state) in document.states.iter().enumerate() {
            index_override_layouts(
                &state.overrides,
                &format!("$.states[{state_index}].overrides"),
                &mut layout_errors,
            );
        }
        for (variant_index, variant) in document.responsive.iter().enumerate() {
            index_override_layouts(
                &variant.overrides,
                &format!("$.responsive[{variant_index}].overrides"),
                &mut layout_errors,
            );
        }
        if !layout_errors.is_empty() {
            return Err(UiDocumentError::InvalidLayout {
                errors: layout_errors,
            });
        }
        let responsive_state_errors =
            super::validate_responsive_state_document(&document, &node_paths);
        if !responsive_state_errors.is_empty() {
            return Err(UiDocumentError::InvalidResponsiveState {
                errors: responsive_state_errors,
            });
        }
        let content_errors = document.validate_content();
        if !content_errors.is_empty() {
            return Err(UiDocumentError::InvalidContent {
                errors: content_errors,
            });
        }
        let mut control_errors = Vec::new();
        validate_node_controls(&document.root, "$.root", &mut control_errors);
        if !control_errors.is_empty() {
            return Err(UiDocumentError::InvalidControl {
                errors: control_errors,
            });
        }
        let mut visual_errors = document.validate_style_tables();
        visual_errors.extend(document.validate_assets());
        index_node_styles(&document, &document.root, "$.root", &mut visual_errors);
        for (state_index, state) in document.states.iter().enumerate() {
            index_override_styles(
                &document,
                &state.overrides,
                &format!("$.states[{state_index}].overrides"),
                &mut visual_errors,
            );
        }
        for (variant_index, variant) in document.responsive.iter().enumerate() {
            index_override_styles(
                &document,
                &variant.overrides,
                &format!("$.responsive[{variant_index}].overrides"),
                &mut visual_errors,
            );
        }
        if !visual_errors.is_empty() {
            return Err(UiDocumentError::InvalidVisual {
                errors: visual_errors,
            });
        }
        let binding_action_errors = super::validate_binding_action_document(&document);
        if !binding_action_errors.is_empty() {
            return Err(UiDocumentError::InvalidBindingAction {
                errors: binding_action_errors,
            });
        }
        if let Some(report) = budget_report {
            return Err(UiDocumentError::ValidationReport { report });
        }
        Ok(Self {
            document,
            node_paths,
        })
    }

    pub fn document(&self) -> &UiDocument {
        &self.document
    }

    pub fn into_document(self) -> UiDocument {
        self.document
    }

    pub fn node_path(&self, node_id: &UiNodeId) -> Option<&str> {
        self.node_paths.get(node_id).map(String::as_str)
    }

    pub fn document_marker(&self) -> UiDocumentMarker {
        UiDocumentMarker {
            document_id: self.document.document_id.clone(),
            schema_version: self.document.schema_version,
        }
    }

    pub fn node_marker(&self, node_id: &UiNodeId) -> Option<UiNodeMarker> {
        self.node_paths.contains_key(node_id).then(|| UiNodeMarker {
            document_id: self.document.document_id.clone(),
            node_id: node_id.clone(),
        })
    }

    pub fn audit_metadata(&self, node_id: &UiNodeId) -> Option<UiDocumentAuditMetadata> {
        self.node_paths
            .get(node_id)
            .map(|path| UiDocumentAuditMetadata {
                document_id: self.document.document_id.clone(),
                schema_version: self.document.schema_version,
                node_id: node_id.clone(),
                document_path: path.clone(),
            })
    }
}

fn typed_document_budget_report(
    document: &UiDocument,
) -> Result<Option<UiValidationReport>, UiDocumentError> {
    let raw = serde_json::to_value(document).map_err(|_| UiDocumentError::Parse {
        message: "document cannot be represented as canonical JSON".to_owned(),
    })?;
    let (node_paths, _) = collect_node_paths(&document.root);
    let budget = analyze_document_budget(super::UI_DOCUMENT_SOURCE_BYTES_UNKNOWN, &raw, document);
    if budget.violations.is_empty() {
        return Ok(None);
    }
    let mut report = UiValidationReport::new(super::UI_DOCUMENT_SOURCE_BYTES_UNKNOWN);
    report.budget_profile = document.metadata.budget_profile.clone();
    report.budget_usage = budget.usage;
    report.extend(budget.violations.into_iter().map(|violation| {
        diagnostic_at_path(
            violation.code,
            violation.phase,
            violation.path,
            violation.node_id,
            &node_paths,
        )
    }));
    report.finish();
    Ok(Some(report))
}

fn index_node_styles(
    document: &UiDocument,
    node: &UiNode,
    path: &str,
    errors: &mut Vec<UiVisualFieldError>,
) {
    let style_path = format!("{path}.style");
    if let Err(error) = document.resolve_style(node.style(), &style_path) {
        errors.push(error);
    }
    errors.extend(document.validate_style_asset_refs(node.style(), &style_path));
    if let Some(component) = node.component() {
        for (state, style) in &component.state_overrides {
            let state_path = format!("{path}.component.state_overrides.{state}");
            if let Err(error) = document.resolve_style(style, &state_path) {
                errors.push(error);
            }
            errors.extend(document.validate_style_asset_refs(style, &state_path));
        }
    }
    for (index, child) in node.children().iter().enumerate() {
        index_node_styles(document, child, &node.child_path(path, index), errors);
    }
}

fn validate_node_controls(node: &UiNode, path: &str, errors: &mut Vec<UiControlFieldError>) {
    super::validate_control_node(node, path, errors);
    for (index, child) in node.children().iter().enumerate() {
        validate_node_controls(child, &node.child_path(path, index), errors);
    }
}

fn index_override_styles(
    document: &UiDocument,
    overrides: &[super::UiNodeOverride],
    path: &str,
    errors: &mut Vec<UiVisualFieldError>,
) {
    for (index, node_override) in overrides.iter().enumerate() {
        let Some(patch) = &node_override.set.style else {
            continue;
        };
        let style = super::UiStyle {
            component: patch.component.clone(),
            role: patch.role.clone(),
            text_role: patch.text_role.clone(),
            inline: patch.inline.clone().unwrap_or_default(),
        };
        let style_path = format!("{path}[{index}].set.style");
        if let Err(error) = document.resolve_style(&style, &style_path) {
            errors.push(error);
        }
        errors.extend(document.validate_style_asset_refs(&style, &style_path));
    }
}

fn index_override_layouts(
    overrides: &[super::UiNodeOverride],
    path: &str,
    layout_errors: &mut Vec<UiLayoutFieldError>,
) {
    for (index, node_override) in overrides.iter().enumerate() {
        let Some(layout) = &node_override.set.layout else {
            continue;
        };
        layout_errors.extend(layout.validate_fields().into_iter().map(|error| {
            UiLayoutFieldError {
                code: error.code,
                path: format!("{path}[{index}].set.layout.{}", error.path),
            }
        }));
    }
}

fn validate_version(version: u32) -> Result<(), UiDocumentError> {
    if version > CURRENT_SCHEMA_VERSION {
        Err(UiDocumentError::FutureSchemaVersion {
            found: version,
            current: CURRENT_SCHEMA_VERSION,
        })
    } else if version < MIN_SUPPORTED_SCHEMA_VERSION {
        Err(UiDocumentError::UnsupportedSchemaVersion {
            found: version,
            minimum: MIN_SUPPORTED_SCHEMA_VERSION,
        })
    } else {
        Ok(())
    }
}

fn index_node(
    node: &UiNode,
    path: &str,
    node_paths: &mut BTreeMap<UiNodeId, String>,
    layout_errors: &mut Vec<UiLayoutFieldError>,
) -> Result<(), UiDocumentError> {
    if let Some(first_path) = node_paths.insert(node.id().clone(), path.to_owned()) {
        return Err(UiDocumentError::DuplicateNodeId {
            node_id: node.id().clone(),
            first_path,
            duplicate_path: path.to_owned(),
        });
    }
    layout_errors.extend(node.layout().validate_fields().into_iter().map(|error| {
        UiLayoutFieldError {
            code: error.code,
            path: format!("{path}.layout.{}", error.path),
        }
    }));
    for (index, child) in node.children().iter().enumerate() {
        index_node(
            child,
            &node.child_path(path, index),
            node_paths,
            layout_errors,
        )?;
    }
    Ok(())
}

#[derive(Clone, Debug, Component, Eq, PartialEq)]
pub struct UiDocumentMarker {
    pub document_id: UiDocumentId,
    pub schema_version: u32,
}

#[derive(Clone, Debug, Component, Eq, PartialEq)]
pub struct UiNodeMarker {
    pub document_id: UiDocumentId,
    pub node_id: UiNodeId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UiDocumentAuditMetadata {
    pub document_id: UiDocumentId,
    pub schema_version: u32,
    pub node_id: UiNodeId,
    pub document_path: String,
}
