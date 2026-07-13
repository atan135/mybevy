use super::{
    CURRENT_SCHEMA_VERSION, MIN_SUPPORTED_SCHEMA_VERSION, UiContentFieldError, UiControlFieldError,
    UiDocument, UiDocumentId, UiLayoutFieldError, UiNode, UiNodeId, UiVisualFieldError,
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
}

impl UiDocumentError {
    pub const fn code(&self) -> &'static str {
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
        }
    }
}

impl std::error::Error for UiDocumentError {}

#[derive(Clone, Debug)]
pub struct ValidatedUiDocument {
    document: UiDocument,
    node_paths: BTreeMap<UiNodeId, String>,
}

impl ValidatedUiDocument {
    pub fn parse_json(source: &str) -> Result<Self, UiDocumentError> {
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
