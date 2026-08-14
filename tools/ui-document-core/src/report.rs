use super::{UiDocumentBudgetUsage, UiNodeId, ValidatedUiDocument};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{DeserializeSeed, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Value};
use std::{cell::RefCell, collections::BTreeSet, fmt};

pub const UI_DOCUMENT_MAX_DIAGNOSTICS: usize = 100;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiValidationSeverity {
    Error,
    Warning,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiValidationPhase {
    Syntax,
    Structure,
    Reference,
    Capability,
    Budget,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiValidationDiagnostic {
    pub code: String,
    pub severity: UiValidationSeverity,
    pub phase: UiValidationPhase,
    pub document_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<UiNodeId>,
    pub field_path: String,
    pub message: String,
    pub suggestion: String,
}

impl UiValidationDiagnostic {
    pub(crate) fn error(
        code: impl Into<String>,
        phase: UiValidationPhase,
        document_path: impl Into<String>,
        node_id: Option<UiNodeId>,
        field_path: impl Into<String>,
    ) -> Self {
        let code = code.into();
        let (message, suggestion) = diagnostic_copy(&code);
        Self {
            code,
            severity: UiValidationSeverity::Error,
            phase,
            document_path: document_path.into(),
            node_id,
            field_path: field_path.into(),
            message: message.to_owned(),
            suggestion: suggestion.to_owned(),
        }
    }

    pub(crate) fn with_copy(
        mut self,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        self.message = message.into();
        self.suggestion = suggestion.into();
        self
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiValidationReport {
    pub report_version: u32,
    pub budget_profile: String,
    pub valid: bool,
    pub truncated: bool,
    pub max_diagnostics: usize,
    pub budget_usage: UiDocumentBudgetUsage,
    pub diagnostics: Vec<UiValidationDiagnostic>,
}

impl UiValidationReport {
    pub(crate) fn new(source_bytes: usize) -> Self {
        Self {
            report_version: 1,
            budget_profile: super::UI_DOCUMENT_BUDGET_PROFILE.to_owned(),
            valid: true,
            truncated: false,
            max_diagnostics: UI_DOCUMENT_MAX_DIAGNOSTICS,
            budget_usage: UiDocumentBudgetUsage {
                source_bytes,
                ..Default::default()
            },
            diagnostics: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, diagnostic: UiValidationDiagnostic) {
        if self.diagnostics.len() < UI_DOCUMENT_MAX_DIAGNOSTICS {
            self.diagnostics.push(diagnostic);
        } else {
            self.truncated = true;
        }
    }

    pub(crate) fn extend(&mut self, diagnostics: impl IntoIterator<Item = UiValidationDiagnostic>) {
        for diagnostic in diagnostics {
            self.push(diagnostic);
        }
    }

    pub(crate) fn finish(&mut self) {
        self.diagnostics.sort_by(|left, right| {
            (
                left.phase,
                &left.field_path,
                &left.code,
                left.node_id.as_ref(),
                &left.document_path,
            )
                .cmp(&(
                    right.phase,
                    &right.field_path,
                    &right.code,
                    right.node_id.as_ref(),
                    &right.document_path,
                ))
        });
        self.diagnostics.dedup_by(|left, right| {
            left.code == right.code
                && left.phase == right.phase
                && left.field_path == right.field_path
                && left.node_id == right.node_id
                && left.document_path == right.document_path
        });
        if self.diagnostics.len() > UI_DOCUMENT_MAX_DIAGNOSTICS {
            self.diagnostics.truncate(UI_DOCUMENT_MAX_DIAGNOSTICS);
            self.truncated = true;
        }
        self.valid = !self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == UiValidationSeverity::Error);
    }

    pub fn has_code(&self, code: &str) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code)
    }
}

#[derive(Debug)]
pub struct UiDocumentValidationResult {
    pub report: UiValidationReport,
    validated: Option<ValidatedUiDocument>,
}

impl UiDocumentValidationResult {
    pub(crate) fn new(report: UiValidationReport, validated: Option<ValidatedUiDocument>) -> Self {
        Self { report, validated }
    }

    pub fn validated(&self) -> Option<&ValidatedUiDocument> {
        self.validated.as_ref()
    }

    pub fn into_validated(self) -> Option<ValidatedUiDocument> {
        self.validated
    }
}

pub(crate) struct DuplicateCheckedJson {
    pub value: Value,
    pub duplicate_paths: Vec<String>,
    pub duplicates_truncated: bool,
}

pub(crate) fn parse_duplicate_checked_json(
    source: &str,
) -> Result<DuplicateCheckedJson, serde_json::Error> {
    let duplicate_paths = RefCell::new(Vec::new());
    let duplicates_truncated = RefCell::new(false);
    let mut deserializer = serde_json::Deserializer::from_str(source);
    let value = JsonValueSeed {
        path: "$".to_owned(),
        duplicate_paths: &duplicate_paths,
        duplicates_truncated: &duplicates_truncated,
    }
    .deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(DuplicateCheckedJson {
        value,
        duplicate_paths: duplicate_paths.into_inner(),
        duplicates_truncated: duplicates_truncated.into_inner(),
    })
}

struct JsonValueSeed<'a> {
    path: String,
    duplicate_paths: &'a RefCell<Vec<String>>,
    duplicates_truncated: &'a RefCell<bool>,
}

impl<'de> DeserializeSeed<'de> for JsonValueSeed<'_> {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(JsonValueVisitor {
            path: self.path,
            duplicate_paths: self.duplicate_paths,
            duplicates_truncated: self.duplicates_truncated,
        })
    }
}

struct JsonValueVisitor<'a> {
    path: String,
    duplicate_paths: &'a RefCell<Vec<String>>,
    duplicates_truncated: &'a RefCell<bool>,
}

impl<'de> Visitor<'de> for JsonValueVisitor<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("JSON numbers must be finite"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(Value::String(value))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(1024));
        let mut index = 0usize;
        while let Some(value) = sequence.next_element_seed(JsonValueSeed {
            path: format!("{}[{index}]", self.path),
            duplicate_paths: self.duplicate_paths,
            duplicates_truncated: self.duplicates_truncated,
        })? {
            values.push(value);
            index += 1;
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut object = Map::new();
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            let field_path = format!("{}.{key}", self.path);
            let value = map.next_value_seed(JsonValueSeed {
                path: field_path.clone(),
                duplicate_paths: self.duplicate_paths,
                duplicates_truncated: self.duplicates_truncated,
            })?;
            if !keys.insert(key.clone()) {
                let mut duplicates = self.duplicate_paths.borrow_mut();
                if duplicates.len() < UI_DOCUMENT_MAX_DIAGNOSTICS + 1 {
                    duplicates.push(field_path);
                } else {
                    *self.duplicates_truncated.borrow_mut() = true;
                }
            }
            object.insert(key, value);
        }
        Ok(Value::Object(object))
    }
}

fn diagnostic_copy(code: &str) -> (&'static str, &'static str) {
    if code.contains("BUDGET") || code == "UI_BUDGET_PROFILE_UNKNOWN" {
        (
            "The document exceeds a frozen validation budget.",
            "Reduce the referenced collection or select a registered budget profile.",
        )
    } else if code.contains("CYCLE") {
        (
            "A reference cycle prevents deterministic resolution.",
            "Remove one reference edge so the dependency graph is acyclic.",
        )
    } else if code.contains("UNKNOWN") || code.contains("NOT_FOUND") {
        (
            "A reference does not resolve inside the declared document boundary.",
            "Declare the referenced ID or replace it with an existing allowed ID.",
        )
    } else if code.contains("FORBIDDEN") || code.contains("NOT_ALLOWED") {
        (
            "The field requests a capability that the document protocol does not allow.",
            "Use a closed protocol value and register privileged behavior in the host.",
        )
    } else if code.starts_with("UI_LAYOUT") {
        (
            "A layout field violates the deterministic layout contract.",
            "Adjust the reported field to a finite, bounded and internally consistent value.",
        )
    } else if code.starts_with("UI_ACTION") || code.starts_with("UI_BINDING") {
        (
            "A binding or action declaration is invalid.",
            "Use a declared typed binding and a closed action invocation.",
        )
    } else if code.starts_with("UI_STYLE") || code.starts_with("UI_ASSET") {
        (
            "A style or asset declaration is invalid.",
            "Correct the reported style or asset field and keep references inside the asset table.",
        )
    } else if code.starts_with("UI_TEXT") {
        (
            "A text declaration is invalid.",
            "Use one closed text source and keep its value within the documented limits.",
        )
    } else {
        (
            "The UI document does not satisfy the declared protocol.",
            "Correct the reported field using the v1 JSON Schema and protocol documentation.",
        )
    }
}
