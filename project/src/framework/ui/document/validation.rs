use super::{
    CURRENT_SCHEMA_VERSION, MIN_SUPPORTED_SCHEMA_VERSION, UiDocument, UiDocumentId, UiNode,
    UiNodeId,
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
}

impl UiDocumentError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Parse { .. } => "UI_DOCUMENT_PARSE_FAILED",
            Self::InvalidSchemaVersion => "UI_SCHEMA_VERSION_INVALID",
            Self::FutureSchemaVersion { .. } => "UI_SCHEMA_FUTURE_VERSION",
            Self::UnsupportedSchemaVersion { .. } => "UI_SCHEMA_VERSION_UNSUPPORTED",
            Self::DuplicateNodeId { .. } => "UI_NODE_ID_DUPLICATE",
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
        let document = serde_json::from_value(value).map_err(|error| UiDocumentError::Parse {
            message: error.to_string(),
        })?;
        Self::new(document)
    }

    pub fn new(document: UiDocument) -> Result<Self, UiDocumentError> {
        validate_version(document.schema_version)?;
        let mut node_paths = BTreeMap::new();
        index_node(&document.root, "$.root", &mut node_paths)?;
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
) -> Result<(), UiDocumentError> {
    if let Some(first_path) = node_paths.insert(node.id().clone(), path.to_owned()) {
        return Err(UiDocumentError::DuplicateNodeId {
            node_id: node.id().clone(),
            first_path,
            duplicate_path: path.to_owned(),
        });
    }
    for (index, child) in node.children().iter().enumerate() {
        index_node(child, &format!("{path}.children[{index}]"), node_paths)?;
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
