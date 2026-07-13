use super::{
    UiActionId, UiActionInvocation, UiBindingPath, UiDocument, UiDocumentId, UiNode, UiNodeId,
    UiTextContent, UiTextFormat, ValidatedUiDocument,
};
use bevy::prelude::{Message, Resource};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const UI_ACTION_STRING_MAX_BYTES: usize = 4 * 1024;
pub const UI_BINDING_ENUM_MAX_VALUES: usize = 64;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiBindingScope {
    Document,
    Owner,
    Local,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiBindingVisibility {
    Inherited,
    Visible,
    Hidden,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiBindingType {
    String,
    Bool,
    Number,
    Visibility,
    Enum { values: Vec<String> },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum UiBindingValue {
    String(String),
    Bool(bool),
    Number(f64),
    Visibility(UiBindingVisibility),
    Enum(String),
}

impl UiBindingValue {
    pub fn value_type_name(&self) -> &'static str {
        match self {
            Self::String(_) => "string",
            Self::Bool(_) => "bool",
            Self::Number(_) => "number",
            Self::Visibility(_) => "visibility",
            Self::Enum(_) => "enum",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiBindingMissingBehavior {
    #[default]
    UseConsumerFallback,
    UseDefault,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiBindingDeclaration {
    pub scope: UiBindingScope,
    pub value_type: UiBindingType,
    #[serde(default)]
    pub default: Option<UiBindingValue>,
    #[serde(default)]
    pub missing: UiBindingMissingBehavior,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum UiActionValue {
    String(String),
    Bool(bool),
    Number(f64),
    Enum(String),
    Node(UiNodeId),
    Binding(UiBindingValue),
}

#[derive(Clone, Debug, PartialEq)]
pub enum UiActionParamType {
    String { max_bytes: usize },
    Bool,
    Number { min: Option<f64>, max: Option<f64> },
    Enum { values: BTreeSet<String> },
    Node { allowed: BTreeSet<UiNodeId> },
    Binding(UiBindingType),
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiActionParamSchema {
    pub value_type: UiActionParamType,
    pub required: bool,
    pub default: Option<UiActionValue>,
}

impl UiActionParamSchema {
    pub fn required(value_type: UiActionParamType) -> Self {
        Self {
            value_type,
            required: true,
            default: None,
        }
    }

    pub fn optional(value_type: UiActionParamType, default: Option<UiActionValue>) -> Self {
        Self {
            value_type,
            required: false,
            default,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiRegisteredActionKind {
    Route {
        target: String,
    },
    ClosePanel {
        target: String,
    },
    BusinessCommand {
        target: String,
    },
    UpdateLocalState {
        binding: UiBindingPath,
        value_param: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiActionDescriptor {
    pub id: UiActionId,
    pub document_id: UiDocumentId,
    pub owner: String,
    pub kind: UiRegisteredActionKind,
    pub params: BTreeMap<String, UiActionParamSchema>,
}

impl UiActionDescriptor {
    pub fn new(
        id: UiActionId,
        document_id: UiDocumentId,
        owner: impl Into<String>,
        kind: UiRegisteredActionKind,
    ) -> Self {
        Self {
            id,
            document_id,
            owner: owner.into(),
            kind,
            params: BTreeMap::new(),
        }
    }

    pub fn with_param(mut self, name: impl Into<String>, schema: UiActionParamSchema) -> Self {
        self.params.insert(name.into(), schema);
        self
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct UiHostBindingKey {
    pub scope: UiBindingScope,
    pub path: UiBindingPath,
}

impl UiHostBindingKey {
    pub fn new(scope: UiBindingScope, path: UiBindingPath) -> Self {
        Self { scope, path }
    }
}

#[derive(Clone, Debug, Default, Resource)]
pub struct UiActionRegistry {
    descriptors: BTreeMap<UiActionId, UiActionDescriptor>,
}

impl UiActionRegistry {
    pub fn register(
        &mut self,
        descriptor: UiActionDescriptor,
    ) -> Result<(), UiActionRegistryError> {
        validate_descriptor(&descriptor)?;
        if self.descriptors.contains_key(&descriptor.id) {
            return Err(UiActionRegistryError::DuplicateAction(descriptor.id));
        }
        self.descriptors.insert(descriptor.id.clone(), descriptor);
        Ok(())
    }

    pub fn descriptor(&self, id: &UiActionId) -> Option<&UiActionDescriptor> {
        self.descriptors.get(id)
    }

    pub fn dispatch(
        &self,
        document: &ValidatedUiDocument,
        context: &UiActionDispatchContext,
    ) -> Result<UiActionDispatch, UiBindingActionError> {
        if !context.owner_alive {
            return Err(UiBindingActionError::new(
                "UI_ACTION_OWNER_DESTROYED",
                "$.runtime.owner",
                Some(context.source_node.clone()),
            ));
        }
        let invocation = find_node_action(&document.document().root, &context.source_node)
            .ok_or_else(|| {
                let code = if document.node_path(&context.source_node).is_some() {
                    "UI_ACTION_SOURCE_HAS_NO_ACTION"
                } else {
                    "UI_ACTION_SOURCE_NODE_UNKNOWN"
                };
                UiBindingActionError::new(
                    code,
                    "$.runtime.source_node",
                    Some(context.source_node.clone()),
                )
            })?;
        validate_invocation(
            document,
            invocation,
            context.source_node.clone(),
            &context.owner,
            self,
            "$.runtime.action",
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiActionRegistryError {
    DuplicateAction(UiActionId),
    InvalidOwner,
    InvalidTarget,
    InvalidParameterName,
    InvalidParameterSchema,
    InvalidParameterDefault,
    InvalidLocalStateSchema,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiActionDispatchContext {
    pub owner: String,
    pub owner_alive: bool,
    pub source_node: UiNodeId,
}

#[derive(Clone, Debug, Message, PartialEq)]
pub struct UiActionDispatch {
    pub action: UiActionId,
    pub document_id: UiDocumentId,
    pub owner: String,
    pub source_node: UiNodeId,
    pub kind: UiRegisteredActionKind,
    pub params: BTreeMap<String, UiActionValue>,
}

#[derive(Clone, Debug, Message, PartialEq)]
pub struct UiActionRejected {
    pub action: UiActionId,
    pub error: UiBindingActionError,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiBindingActionError {
    pub code: &'static str,
    pub path: String,
    pub node_id: Option<UiNodeId>,
}

impl UiBindingActionError {
    fn new(code: &'static str, path: impl Into<String>, node_id: Option<UiNodeId>) -> Self {
        Self {
            code,
            path: path.into(),
            node_id,
        }
    }
}

pub struct UiDocumentHostValidationContext<'a> {
    pub owner: &'a str,
    pub owner_alive: bool,
    pub action_registry: &'a UiActionRegistry,
    pub bindings: &'a BTreeMap<UiHostBindingKey, UiBindingType>,
}

impl ValidatedUiDocument {
    pub fn validate_with_host(
        &self,
        context: &UiDocumentHostValidationContext<'_>,
    ) -> Vec<UiBindingActionError> {
        let mut errors = Vec::new();
        if !context.owner_alive {
            errors.push(UiBindingActionError::new(
                "UI_ACTION_OWNER_DESTROYED",
                "$.host.owner",
                None,
            ));
            return errors;
        }
        for (path, declaration) in &self.document().bindings {
            if declaration.scope == UiBindingScope::Local {
                continue;
            }
            let key = UiHostBindingKey::new(declaration.scope, path.clone());
            match context.bindings.get(&key) {
                None => errors.push(UiBindingActionError::new(
                    "UI_BINDING_HOST_PATH_UNKNOWN",
                    format!("$.bindings.{path}"),
                    None,
                )),
                Some(value_type) if value_type != &declaration.value_type => {
                    errors.push(UiBindingActionError::new(
                        "UI_BINDING_HOST_TYPE_MISMATCH",
                        format!("$.bindings.{path}.value_type"),
                        None,
                    ))
                }
                Some(_) => {}
            }
        }
        validate_host_node(self, &self.document().root, "$.root", context, &mut errors);
        errors
    }
}

pub(crate) fn validate_binding_action_document(document: &UiDocument) -> Vec<UiBindingActionError> {
    let mut errors = Vec::new();
    for (path, declaration) in &document.bindings {
        let field_path = format!("$.bindings.{path}");
        validate_binding_declaration(declaration, &field_path, &mut errors);
    }
    validate_static_node(document, &document.root, "$.root", &mut errors);
    errors
}

fn validate_binding_declaration(
    declaration: &UiBindingDeclaration,
    path: &str,
    errors: &mut Vec<UiBindingActionError>,
) {
    if let UiBindingType::Enum { values } = &declaration.value_type {
        let unique = values.iter().collect::<BTreeSet<_>>();
        if values.is_empty()
            || values.len() > UI_BINDING_ENUM_MAX_VALUES
            || unique.len() != values.len()
            || values.iter().any(|value| !is_safe_identifier(value))
        {
            errors.push(UiBindingActionError::new(
                "UI_BINDING_ENUM_INVALID",
                format!("{path}.value_type.values"),
                None,
            ));
        }
    }
    if declaration.missing == UiBindingMissingBehavior::UseDefault && declaration.default.is_none()
    {
        errors.push(UiBindingActionError::new(
            "UI_BINDING_DEFAULT_REQUIRED",
            format!("{path}.default"),
            None,
        ));
    }
    if let Some(default) = &declaration.default
        && !binding_value_matches(&declaration.value_type, default)
    {
        errors.push(UiBindingActionError::new(
            "UI_BINDING_DEFAULT_TYPE_MISMATCH",
            format!("{path}.default"),
            None,
        ));
    }
}

fn validate_static_node(
    document: &UiDocument,
    node: &UiNode,
    path: &str,
    errors: &mut Vec<UiBindingActionError>,
) {
    visit_node_text_content(node, |content, content_path| {
        validate_bound_text(
            document,
            node.id(),
            content,
            &format!("{path}{content_path}"),
            errors,
        )
    });
    if let UiNode::Button { on_click, .. } = node {
        validate_action_values(on_click, node.id(), &format!("{path}.on_click"), errors);
    }
    for (index, child) in node.children().iter().enumerate() {
        validate_static_node(document, child, &node.child_path(path, index), errors);
    }
}

fn validate_bound_text(
    document: &UiDocument,
    node_id: &UiNodeId,
    content: &UiTextContent,
    path: &str,
    errors: &mut Vec<UiBindingActionError>,
) {
    let UiTextContent::Binding(source) = content else {
        return;
    };
    let Some(declaration) = document.bindings.get(&source.binding_path) else {
        errors.push(UiBindingActionError::new(
            "UI_BINDING_PATH_UNDECLARED",
            format!("{path}.binding_path"),
            Some(node_id.clone()),
        ));
        return;
    };
    let type_matches = match source.format {
        UiTextFormat::Plain => matches!(
            declaration.value_type,
            UiBindingType::String
                | UiBindingType::Bool
                | UiBindingType::Number
                | UiBindingType::Enum { .. }
        ),
        UiTextFormat::Number { .. } | UiTextFormat::Percent { .. } | UiTextFormat::Bytes { .. } => {
            declaration.value_type == UiBindingType::Number
        }
    };
    if !type_matches {
        errors.push(UiBindingActionError::new(
            "UI_BINDING_TYPE_MISMATCH",
            format!("{path}.binding_path"),
            Some(node_id.clone()),
        ));
    }
}

fn visit_node_text_content(node: &UiNode, mut visitor: impl FnMut(&UiTextContent, &str)) {
    match node {
        UiNode::Text { content, .. } => visitor(content, ".content"),
        UiNode::Button {
            label: Some(label), ..
        } => visitor(label, ".label"),
        _ => {}
    }
    if let Some(component) = node.component() {
        for (slot, content) in &component.slots {
            if let super::UiControlSlotContent::Text { content } = content {
                visitor(
                    content,
                    &format!(".component.slots.{slot:?}.content").to_lowercase(),
                );
            }
        }
    }
}

fn validate_action_values(
    invocation: &UiActionInvocation,
    node_id: &UiNodeId,
    path: &str,
    errors: &mut Vec<UiBindingActionError>,
) {
    for (name, value) in &invocation.params {
        if !is_safe_identifier(name) {
            errors.push(UiBindingActionError::new(
                "UI_ACTION_PARAM_NAME_INVALID",
                format!("{path}.params.{name}"),
                Some(node_id.clone()),
            ));
        }
        if !action_value_is_intrinsically_safe(value) {
            errors.push(UiBindingActionError::new(
                "UI_ACTION_PARAM_FORBIDDEN",
                format!("{path}.params.{name}"),
                Some(node_id.clone()),
            ));
        }
    }
}

fn validate_host_node(
    document: &ValidatedUiDocument,
    node: &UiNode,
    path: &str,
    context: &UiDocumentHostValidationContext<'_>,
    errors: &mut Vec<UiBindingActionError>,
) {
    if let UiNode::Button { on_click, .. } = node
        && let Err(error) = validate_invocation(
            document,
            on_click,
            node.id().clone(),
            context.owner,
            context.action_registry,
            &format!("{path}.on_click"),
        )
    {
        errors.push(error);
    }
    for (index, child) in node.children().iter().enumerate() {
        validate_host_node(
            document,
            child,
            &node.child_path(path, index),
            context,
            errors,
        );
    }
}

fn find_node_action<'a>(node: &'a UiNode, id: &UiNodeId) -> Option<&'a UiActionInvocation> {
    if node.id() == id {
        return match node {
            UiNode::Button { on_click, .. } => Some(on_click),
            _ => None,
        };
    }
    node.children()
        .iter()
        .find_map(|child| find_node_action(child, id))
}

fn validate_invocation(
    document: &ValidatedUiDocument,
    invocation: &UiActionInvocation,
    source_node: UiNodeId,
    owner: &str,
    registry: &UiActionRegistry,
    path: &str,
) -> Result<UiActionDispatch, UiBindingActionError> {
    if document.node_path(&source_node).is_none() {
        return Err(UiBindingActionError::new(
            "UI_ACTION_SOURCE_NODE_UNKNOWN",
            format!("{path}.source_node"),
            Some(source_node),
        ));
    }
    let descriptor = registry.descriptor(&invocation.action).ok_or_else(|| {
        UiBindingActionError::new(
            "UI_ACTION_UNKNOWN",
            format!("{path}.action"),
            Some(source_node.clone()),
        )
    })?;
    if descriptor.document_id != document.document().document_id {
        return Err(UiBindingActionError::new(
            "UI_ACTION_DOCUMENT_FORBIDDEN",
            format!("{path}.action"),
            Some(source_node),
        ));
    }
    if descriptor.owner != owner {
        return Err(UiBindingActionError::new(
            "UI_ACTION_OWNER_FORBIDDEN",
            format!("{path}.action"),
            Some(source_node),
        ));
    }
    let mut params = BTreeMap::new();
    for name in invocation.params.keys() {
        if !descriptor.params.contains_key(name) {
            return Err(UiBindingActionError::new(
                "UI_ACTION_PARAM_UNKNOWN",
                format!("{path}.params.{name}"),
                Some(source_node),
            ));
        }
    }
    for (name, schema) in &descriptor.params {
        let value = invocation.params.get(name).or(schema.default.as_ref());
        let Some(value) = value else {
            if schema.required {
                return Err(UiBindingActionError::new(
                    "UI_ACTION_PARAM_REQUIRED",
                    format!("{path}.params.{name}"),
                    Some(source_node),
                ));
            }
            continue;
        };
        if !action_value_matches(document, &schema.value_type, value) {
            return Err(UiBindingActionError::new(
                "UI_ACTION_PARAM_TYPE_MISMATCH",
                format!("{path}.params.{name}"),
                Some(source_node),
            ));
        }
        params.insert(name.clone(), value.clone());
    }
    if let UiRegisteredActionKind::UpdateLocalState {
        binding,
        value_param,
    } = &descriptor.kind
    {
        let Some(declaration) = document.document().bindings.get(binding) else {
            return Err(UiBindingActionError::new(
                "UI_ACTION_LOCAL_BINDING_UNKNOWN",
                format!("{path}.action"),
                Some(source_node),
            ));
        };
        if declaration.scope != UiBindingScope::Local {
            return Err(UiBindingActionError::new(
                "UI_ACTION_LOCAL_BINDING_FORBIDDEN",
                format!("{path}.action"),
                Some(source_node),
            ));
        }
        let matches = params.get(value_param).is_some_and(|value| {
            matches!(value, UiActionValue::Binding(binding_value) if binding_value_matches(&declaration.value_type, binding_value))
        });
        if !matches {
            return Err(UiBindingActionError::new(
                "UI_ACTION_LOCAL_VALUE_MISMATCH",
                format!("{path}.params.{value_param}"),
                Some(source_node),
            ));
        }
    }
    Ok(UiActionDispatch {
        action: invocation.action.clone(),
        document_id: document.document().document_id.clone(),
        owner: owner.to_owned(),
        source_node,
        kind: descriptor.kind.clone(),
        params,
    })
}

fn action_value_matches(
    document: &ValidatedUiDocument,
    schema: &UiActionParamType,
    value: &UiActionValue,
) -> bool {
    match (schema, value) {
        (UiActionParamType::String { max_bytes }, UiActionValue::String(value)) => {
            value.len() <= *max_bytes && safe_action_string(value)
        }
        (UiActionParamType::Bool, UiActionValue::Bool(_)) => true,
        (UiActionParamType::Number { min, max }, UiActionValue::Number(value)) => {
            value.is_finite()
                && min.is_none_or(|min| *value >= min)
                && max.is_none_or(|max| *value <= max)
        }
        (UiActionParamType::Enum { values }, UiActionValue::Enum(value)) => values.contains(value),
        (UiActionParamType::Node { allowed }, UiActionValue::Node(node)) => {
            allowed.contains(node) && document.node_path(node).is_some()
        }
        (UiActionParamType::Binding(value_type), UiActionValue::Binding(value)) => {
            binding_value_matches(value_type, value)
        }
        _ => false,
    }
}

fn action_value_is_intrinsically_safe(value: &UiActionValue) -> bool {
    match value {
        UiActionValue::String(value) => {
            value.len() <= UI_ACTION_STRING_MAX_BYTES && safe_action_string(value)
        }
        UiActionValue::Number(value) => value.is_finite(),
        UiActionValue::Enum(value) => is_safe_identifier(value),
        UiActionValue::Binding(value) => binding_value_is_safe(value),
        UiActionValue::Bool(_) | UiActionValue::Node(_) => true,
    }
}

pub fn binding_value_matches(value_type: &UiBindingType, value: &UiBindingValue) -> bool {
    match (value_type, value) {
        (UiBindingType::String, UiBindingValue::String(value)) => {
            value.len() <= UI_ACTION_STRING_MAX_BYTES
        }
        (UiBindingType::Bool, UiBindingValue::Bool(_)) => true,
        (UiBindingType::Number, UiBindingValue::Number(value)) => value.is_finite(),
        (UiBindingType::Visibility, UiBindingValue::Visibility(_)) => true,
        (UiBindingType::Enum { values }, UiBindingValue::Enum(value)) => values.contains(value),
        _ => false,
    }
}

fn binding_value_is_safe(value: &UiBindingValue) -> bool {
    match value {
        UiBindingValue::String(value) => {
            value.len() <= UI_ACTION_STRING_MAX_BYTES && safe_action_string(value)
        }
        UiBindingValue::Number(value) => value.is_finite(),
        UiBindingValue::Enum(value) => is_safe_identifier(value),
        UiBindingValue::Bool(_) | UiBindingValue::Visibility(_) => true,
    }
}

fn validate_descriptor(descriptor: &UiActionDescriptor) -> Result<(), UiActionRegistryError> {
    if !is_safe_identifier(&descriptor.owner) {
        return Err(UiActionRegistryError::InvalidOwner);
    }
    let target_is_safe = match &descriptor.kind {
        UiRegisteredActionKind::Route { target }
        | UiRegisteredActionKind::ClosePanel { target }
        | UiRegisteredActionKind::BusinessCommand { target } => {
            is_safe_namespaced_identifier(target)
        }
        UiRegisteredActionKind::UpdateLocalState { value_param, .. } => {
            is_safe_identifier(value_param)
        }
    };
    if !target_is_safe {
        return Err(UiActionRegistryError::InvalidTarget);
    }
    for (name, schema) in &descriptor.params {
        if !is_safe_identifier(name) {
            return Err(UiActionRegistryError::InvalidParameterName);
        }
        if !action_param_schema_is_valid(&schema.value_type) {
            return Err(UiActionRegistryError::InvalidParameterSchema);
        }
        if let Some(default) = &schema.default {
            if !action_default_matches(&schema.value_type, default) {
                return Err(UiActionRegistryError::InvalidParameterDefault);
            }
        }
    }
    if let UiRegisteredActionKind::UpdateLocalState { value_param, .. } = &descriptor.kind {
        let Some(schema) = descriptor.params.get(value_param) else {
            return Err(UiActionRegistryError::InvalidLocalStateSchema);
        };
        if !schema.required || !matches!(schema.value_type, UiActionParamType::Binding(_)) {
            return Err(UiActionRegistryError::InvalidLocalStateSchema);
        }
    }
    Ok(())
}

fn action_param_schema_is_valid(schema: &UiActionParamType) -> bool {
    match schema {
        UiActionParamType::String { max_bytes } => {
            (1..=UI_ACTION_STRING_MAX_BYTES).contains(max_bytes)
        }
        UiActionParamType::Bool => true,
        UiActionParamType::Number { min, max } => {
            min.is_none_or(f64::is_finite)
                && max.is_none_or(f64::is_finite)
                && min.zip(*max).is_none_or(|(min, max)| min <= max)
        }
        UiActionParamType::Enum { values } => {
            !values.is_empty()
                && values.len() <= UI_BINDING_ENUM_MAX_VALUES
                && values.iter().all(|value| is_safe_identifier(value))
        }
        UiActionParamType::Node { allowed } => {
            !allowed.is_empty() && allowed.len() <= UI_BINDING_ENUM_MAX_VALUES
        }
        UiActionParamType::Binding(value_type) => binding_type_schema_is_valid(value_type),
    }
}

fn binding_type_schema_is_valid(value_type: &UiBindingType) -> bool {
    match value_type {
        UiBindingType::Enum { values } => {
            let unique = values.iter().collect::<BTreeSet<_>>();
            !values.is_empty()
                && values.len() <= UI_BINDING_ENUM_MAX_VALUES
                && unique.len() == values.len()
                && values.iter().all(|value| is_safe_identifier(value))
        }
        _ => true,
    }
}

fn action_default_matches(schema: &UiActionParamType, value: &UiActionValue) -> bool {
    match (schema, value) {
        (UiActionParamType::String { max_bytes }, UiActionValue::String(value)) => {
            value.len() <= *max_bytes && safe_action_string(value)
        }
        (UiActionParamType::Bool, UiActionValue::Bool(_)) => true,
        (UiActionParamType::Number { min, max }, UiActionValue::Number(value)) => {
            value.is_finite()
                && min.is_none_or(|min| *value >= min)
                && max.is_none_or(|max| *value <= max)
        }
        (UiActionParamType::Enum { values }, UiActionValue::Enum(value)) => values.contains(value),
        (UiActionParamType::Node { allowed }, UiActionValue::Node(value)) => {
            allowed.contains(value)
        }
        (UiActionParamType::Binding(value_type), UiActionValue::Binding(value)) => {
            binding_value_matches(value_type, value)
        }
        _ => false,
    }
}

fn is_safe_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
        && value.len() <= 128
}

fn is_safe_namespaced_identifier(value: &str) -> bool {
    value.len() <= 128 && value.split('.').count() >= 2 && value.split('.').all(is_safe_identifier)
}

fn safe_action_string(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    let looks_like_command_line = ["cargo ", "powershell ", "cmd ", "bash ", "sh "]
        .iter()
        .any(|prefix| normalized.starts_with(prefix));
    let looks_like_network_address = value.parse::<std::net::IpAddr>().is_ok()
        || value.parse::<std::net::SocketAddr>().is_ok()
        || value.rsplit_once(':').is_some_and(|(host, port)| {
            port.parse::<u16>().is_ok()
                && (host.eq_ignore_ascii_case("localhost") || host.contains('.'))
        });
    !looks_like_command_line
        && !looks_like_network_address
        && !value.contains("://")
        && !value.contains('\\')
        && !value.contains('/')
        && !value.starts_with('/')
        && !value
            .chars()
            .any(|character| matches!(character, '\0' | '\r' | '\n' | ';' | '|' | '&' | '`'))
        && !value.contains("--")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::str::FromStr;

    const BINDING_ACTION_DOCUMENT: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/ui/documents/fixtures/binding_actions.v1.json"
    ));

    #[test]
    fn ui_document_binding_protocol_covers_typed_values_scopes_defaults_and_missing_behavior() {
        let validated = UiDocument::parse_and_validate_json(BINDING_ACTION_DOCUMENT).unwrap();
        let bindings = &validated.document().bindings;

        assert_eq!(bindings.len(), 5);
        assert_eq!(
            binding(bindings, "state.title").scope,
            UiBindingScope::Local
        );
        assert_eq!(
            binding(bindings, "state.ready").scope,
            UiBindingScope::Owner
        );
        assert_eq!(
            binding(bindings, "state.progress").scope,
            UiBindingScope::Document
        );
        assert_eq!(
            binding(bindings, "state.visibility").default,
            Some(UiBindingValue::Visibility(UiBindingVisibility::Visible))
        );
        assert_eq!(
            binding(bindings, "state.mode").value_type,
            UiBindingType::Enum {
                values: vec!["basic".to_owned(), "advanced".to_owned()]
            }
        );
        assert_eq!(
            binding(bindings, "state.title").missing,
            UiBindingMissingBehavior::UseDefault
        );
    }

    #[test]
    fn ui_document_binding_validation_rejects_type_mismatch_bad_default_and_enum() {
        let mut value: Value = serde_json::from_str(BINDING_ACTION_DOCUMENT).unwrap();
        value["bindings"]["state.title"]["default"] = json!({ "kind": "bool", "value": true });
        assert_binding_error(
            UiDocument::parse_and_validate_json(&value.to_string()).unwrap_err(),
            "UI_BINDING_DEFAULT_TYPE_MISMATCH",
        );

        let mut value: Value = serde_json::from_str(BINDING_ACTION_DOCUMENT).unwrap();
        value["bindings"]["state.title"]["missing"] = json!("use_default");
        value["bindings"]["state.title"]["default"] = Value::Null;
        assert_binding_error(
            UiDocument::parse_and_validate_json(&value.to_string()).unwrap_err(),
            "UI_BINDING_DEFAULT_REQUIRED",
        );

        let mut value: Value = serde_json::from_str(BINDING_ACTION_DOCUMENT).unwrap();
        value["bindings"]["state.mode"]["value_type"]["values"] = json!(["basic", "basic"]);
        assert_binding_error(
            UiDocument::parse_and_validate_json(&value.to_string()).unwrap_err(),
            "UI_BINDING_ENUM_INVALID",
        );

        let mut value: Value = serde_json::from_str(BINDING_ACTION_DOCUMENT).unwrap();
        value["bindings"]["state.title"]["value_type"] = json!({ "kind": "visibility" });
        value["bindings"]["state.title"]["default"] =
            json!({ "kind": "visibility", "value": "visible" });
        assert_binding_error(
            UiDocument::parse_and_validate_json(&value.to_string()).unwrap_err(),
            "UI_BINDING_TYPE_MISMATCH",
        );
    }

    #[test]
    fn ui_document_action_registry_validates_four_closed_kinds_and_host_bindings() {
        let validated = UiDocument::parse_and_validate_json(BINDING_ACTION_DOCUMENT).unwrap();
        let registry = complete_registry();
        let bindings = host_bindings();
        let context = UiDocumentHostValidationContext {
            owner: "binding_owner",
            owner_alive: true,
            action_registry: &registry,
            bindings: &bindings,
        };

        assert!(validated.validate_with_host(&context).is_empty());

        let route = dispatch(
            &validated,
            &registry,
            "binding.route",
            "binding_owner",
            true,
        )
        .unwrap();
        assert!(matches!(route.kind, UiRegisteredActionKind::Route { .. }));
        let close = dispatch(
            &validated,
            &registry,
            "binding.close",
            "binding_owner",
            true,
        )
        .unwrap();
        assert!(matches!(
            close.kind,
            UiRegisteredActionKind::ClosePanel { .. }
        ));
        let business = dispatch(
            &validated,
            &registry,
            "binding.business",
            "binding_owner",
            true,
        )
        .unwrap();
        assert!(matches!(
            business.kind,
            UiRegisteredActionKind::BusinessCommand { .. }
        ));
        assert_eq!(
            business.params["target"],
            UiActionValue::Node(UiNodeId::from_str("binding.title").unwrap())
        );
        let update = dispatch(
            &validated,
            &registry,
            "binding.update",
            "binding_owner",
            true,
        )
        .unwrap();
        assert!(matches!(
            update.kind,
            UiRegisteredActionKind::UpdateLocalState { .. }
        ));
    }

    #[test]
    fn ui_document_action_validation_rejects_unknown_params_types_targets_and_permissions() {
        let validated = UiDocument::parse_and_validate_json(BINDING_ACTION_DOCUMENT).unwrap();
        let bindings = host_bindings();
        let empty_registry = UiActionRegistry::default();
        let errors = validated.validate_with_host(&UiDocumentHostValidationContext {
            owner: "binding_owner",
            owner_alive: true,
            action_registry: &empty_registry,
            bindings: &bindings,
        });
        assert!(errors.iter().any(|error| error.code == "UI_ACTION_UNKNOWN"));

        let registry = complete_registry();
        let mut value: Value = serde_json::from_str(BINDING_ACTION_DOCUMENT).unwrap();
        value["root"]["children"][3]["on_click"]["params"]["mode"] =
            json!({ "kind": "bool", "value": true });
        let wrong_type = UiDocument::parse_and_validate_json(&value.to_string()).unwrap();
        assert_dispatch_error(
            dispatch(
                &wrong_type,
                &registry,
                "binding.business",
                "binding_owner",
                true,
            ),
            "UI_ACTION_PARAM_TYPE_MISMATCH",
        );

        let mut value: Value = serde_json::from_str(BINDING_ACTION_DOCUMENT).unwrap();
        value["root"]["children"][3]["on_click"]["params"]["target"] =
            json!({ "kind": "node", "value": "other.page_node" });
        let cross_page = UiDocument::parse_and_validate_json(&value.to_string()).unwrap();
        assert_dispatch_error(
            dispatch(
                &cross_page,
                &registry,
                "binding.business",
                "binding_owner",
                true,
            ),
            "UI_ACTION_PARAM_TYPE_MISMATCH",
        );

        let mut value: Value = serde_json::from_str(BINDING_ACTION_DOCUMENT).unwrap();
        value["root"]["children"][3]["on_click"]["params"]["target"] =
            json!({ "kind": "node", "value": "binding.route" });
        let same_page_not_allowed =
            UiDocument::parse_and_validate_json(&value.to_string()).unwrap();
        assert_dispatch_error(
            dispatch(
                &same_page_not_allowed,
                &registry,
                "binding.business",
                "binding_owner",
                true,
            ),
            "UI_ACTION_PARAM_TYPE_MISMATCH",
        );

        assert_dispatch_error(
            dispatch(&validated, &registry, "binding.route", "other_owner", true),
            "UI_ACTION_OWNER_FORBIDDEN",
        );

        assert_dispatch_error(
            registry.dispatch(
                &validated,
                &UiActionDispatchContext {
                    owner: "binding_owner".to_owned(),
                    owner_alive: true,
                    source_node: UiNodeId::from_str("binding.title").unwrap(),
                },
            ),
            "UI_ACTION_SOURCE_HAS_NO_ACTION",
        );

        let mut cross_document = UiActionRegistry::default();
        cross_document
            .register(UiActionDescriptor::new(
                UiActionId::from_str("binding.route").unwrap(),
                UiDocumentId::from_str("other.document").unwrap(),
                "binding_owner",
                UiRegisteredActionKind::Route {
                    target: "game.route_lobby".to_owned(),
                },
            ))
            .unwrap();
        assert_dispatch_error(
            dispatch(
                &validated,
                &cross_document,
                "binding.route",
                "binding_owner",
                true,
            ),
            "UI_ACTION_DOCUMENT_FORBIDDEN",
        );
    }

    #[test]
    fn ui_document_action_dispatch_rejects_destroyed_owner_and_non_local_update() {
        let validated = UiDocument::parse_and_validate_json(BINDING_ACTION_DOCUMENT).unwrap();
        let registry = complete_registry();
        assert_dispatch_error(
            dispatch(
                &validated,
                &registry,
                "binding.route",
                "binding_owner",
                false,
            ),
            "UI_ACTION_OWNER_DESTROYED",
        );

        let mut registry = UiActionRegistry::default();
        registry
            .register(
                UiActionDescriptor::new(
                    UiActionId::from_str("binding.update").unwrap(),
                    UiDocumentId::from_str("binding.actions").unwrap(),
                    "binding_owner",
                    UiRegisteredActionKind::UpdateLocalState {
                        binding: UiBindingPath::from_str("state.ready").unwrap(),
                        value_param: "value".to_owned(),
                    },
                )
                .with_param(
                    "value",
                    UiActionParamSchema::required(UiActionParamType::Binding(UiBindingType::Bool)),
                ),
            )
            .unwrap();
        assert_dispatch_error(
            dispatch(
                &validated,
                &registry,
                "binding.update",
                "binding_owner",
                true,
            ),
            "UI_ACTION_PARAM_TYPE_MISMATCH",
        );
    }

    #[test]
    fn ui_document_action_input_is_closed_and_rejects_path_url_message_and_shell_strings() {
        let unknown_field = BINDING_ACTION_DOCUMENT.replacen(
            "\"action\": \"binding.route\"",
            "\"action\": \"binding.route\", \"system\": \"GameRouteCommand\"",
            1,
        );
        let error = UiDocument::parse_and_validate_json(&unknown_field).unwrap_err();
        assert_eq!(error.code(), "UI_DOCUMENT_PARSE_FAILED");
        assert!(error.to_string().contains("unknown field `system`"));

        let unknown_message = BINDING_ACTION_DOCUMENT.replacen(
            "\"action\": \"binding.route\"",
            "\"action\": \"binding.route\", \"message\": \"GameRouteCommand\"",
            1,
        );
        let error = UiDocument::parse_and_validate_json(&unknown_message).unwrap_err();
        assert_eq!(error.code(), "UI_DOCUMENT_PARSE_FAILED");
        assert!(error.to_string().contains("unknown field `message`"));

        for forbidden in [
            "https://example.invalid",
            "c:\\\\secret.txt",
            "../secret.txt",
            "ui/private/secret.txt",
            "127.0.0.1:8080",
            "server.example:443",
            "cargo run",
            "cargo run --release",
            "ok; shutdown",
        ] {
            let mut value: Value = serde_json::from_str(BINDING_ACTION_DOCUMENT).unwrap();
            value["root"]["children"][1]["on_click"]["params"] = json!({
                "payload": { "kind": "string", "value": forbidden }
            });
            assert_binding_error(
                UiDocument::parse_and_validate_json(&value.to_string()).unwrap_err(),
                "UI_ACTION_PARAM_FORBIDDEN",
            );
        }

        for opaque_payload in ["Alice", "SaveCommand", "handle_game_route_commands"] {
            let mut value: Value = serde_json::from_str(BINDING_ACTION_DOCUMENT).unwrap();
            value["root"]["children"][1]["on_click"]["params"] = json!({
                "payload": { "kind": "string", "value": opaque_payload }
            });
            UiDocument::parse_and_validate_json(&value.to_string()).unwrap();
        }

        let mut value: Value = serde_json::from_str(BINDING_ACTION_DOCUMENT).unwrap();
        value["root"]["children"][1]["on_click"]["params"] = json!({
            "payload": {
                "kind": "binding",
                "value": { "kind": "string", "value": "https://example.invalid" }
            }
        });
        assert_binding_error(
            UiDocument::parse_and_validate_json(&value.to_string()).unwrap_err(),
            "UI_ACTION_PARAM_FORBIDDEN",
        );
    }

    #[test]
    fn ui_document_action_registry_rejects_invalid_descriptor_schemas() {
        let update_without_value = descriptor_for_registry_test(
            "registry.update_missing",
            UiRegisteredActionKind::UpdateLocalState {
                binding: UiBindingPath::from_str("state.mode").unwrap(),
                value_param: "value".to_owned(),
            },
        );
        assert_registration_error(
            update_without_value,
            UiActionRegistryError::InvalidLocalStateSchema,
        );

        let update_with_string = descriptor_for_registry_test(
            "registry.update_string",
            UiRegisteredActionKind::UpdateLocalState {
                binding: UiBindingPath::from_str("state.mode").unwrap(),
                value_param: "value".to_owned(),
            },
        )
        .with_param(
            "value",
            UiActionParamSchema::required(UiActionParamType::String { max_bytes: 32 }),
        );
        assert_registration_error(
            update_with_string,
            UiActionRegistryError::InvalidLocalStateSchema,
        );

        for (id, value_type) in [
            (
                "registry.string_zero",
                UiActionParamType::String { max_bytes: 0 },
            ),
            (
                "registry.string_large",
                UiActionParamType::String {
                    max_bytes: UI_ACTION_STRING_MAX_BYTES + 1,
                },
            ),
            (
                "registry.enum_empty",
                UiActionParamType::Enum {
                    values: BTreeSet::new(),
                },
            ),
            (
                "registry.enum_unsafe",
                UiActionParamType::Enum {
                    values: ["BadValue".to_owned()].into_iter().collect(),
                },
            ),
            (
                "registry.node_empty",
                UiActionParamType::Node {
                    allowed: BTreeSet::new(),
                },
            ),
            (
                "registry.number_range",
                UiActionParamType::Number {
                    min: Some(2.0),
                    max: Some(1.0),
                },
            ),
            (
                "registry.binding_enum_empty",
                UiActionParamType::Binding(UiBindingType::Enum { values: Vec::new() }),
            ),
            (
                "registry.binding_enum_duplicate",
                UiActionParamType::Binding(UiBindingType::Enum {
                    values: vec!["basic".to_owned(), "basic".to_owned()],
                }),
            ),
            (
                "registry.binding_enum_unsafe",
                UiActionParamType::Binding(UiBindingType::Enum {
                    values: vec!["BadValue".to_owned()],
                }),
            ),
        ] {
            let descriptor = descriptor_for_registry_test(
                id,
                UiRegisteredActionKind::BusinessCommand {
                    target: "game.registry_test".to_owned(),
                },
            )
            .with_param("value", UiActionParamSchema::required(value_type));
            assert_registration_error(descriptor, UiActionRegistryError::InvalidParameterSchema);
        }

        let opaque_pascal_case = descriptor_for_registry_test(
            "registry.opaque_string",
            UiRegisteredActionKind::BusinessCommand {
                target: "game.registry_test".to_owned(),
            },
        )
        .with_param(
            "value",
            UiActionParamSchema::optional(
                UiActionParamType::String { max_bytes: 32 },
                Some(UiActionValue::String("SaveCommand".to_owned())),
            ),
        );
        UiActionRegistry::default()
            .register(opaque_pascal_case)
            .unwrap();
    }

    #[test]
    fn ui_document_host_validation_reports_external_binding_type_mismatch() {
        let validated = UiDocument::parse_and_validate_json(BINDING_ACTION_DOCUMENT).unwrap();
        let registry = complete_registry();
        let mut bindings = host_bindings();
        bindings.insert(
            UiHostBindingKey::new(
                UiBindingScope::Owner,
                UiBindingPath::from_str("state.ready").unwrap(),
            ),
            UiBindingType::String,
        );
        let errors = validated.validate_with_host(&UiDocumentHostValidationContext {
            owner: "binding_owner",
            owner_alive: true,
            action_registry: &registry,
            bindings: &bindings,
        });
        assert!(
            errors
                .iter()
                .any(|error| error.code == "UI_BINDING_HOST_TYPE_MISMATCH")
        );
    }

    fn complete_registry() -> UiActionRegistry {
        let mut registry = UiActionRegistry::default();
        let document_id = UiDocumentId::from_str("binding.actions").unwrap();
        for descriptor in [
            UiActionDescriptor::new(
                UiActionId::from_str("binding.route").unwrap(),
                document_id.clone(),
                "binding_owner",
                UiRegisteredActionKind::Route {
                    target: "game.route_lobby".to_owned(),
                },
            ),
            UiActionDescriptor::new(
                UiActionId::from_str("binding.close").unwrap(),
                document_id.clone(),
                "binding_owner",
                UiRegisteredActionKind::ClosePanel {
                    target: "ui.panel_dialog".to_owned(),
                },
            ),
            UiActionDescriptor::new(
                UiActionId::from_str("binding.business").unwrap(),
                document_id.clone(),
                "binding_owner",
                UiRegisteredActionKind::BusinessCommand {
                    target: "game.submit_profile".to_owned(),
                },
            )
            .with_param(
                "mode",
                UiActionParamSchema::required(UiActionParamType::Enum {
                    values: ["basic".to_owned(), "advanced".to_owned()]
                        .into_iter()
                        .collect(),
                }),
            )
            .with_param(
                "target",
                UiActionParamSchema::required(UiActionParamType::Node {
                    allowed: [UiNodeId::from_str("binding.title").unwrap()]
                        .into_iter()
                        .collect(),
                }),
            ),
            UiActionDescriptor::new(
                UiActionId::from_str("binding.update").unwrap(),
                document_id,
                "binding_owner",
                UiRegisteredActionKind::UpdateLocalState {
                    binding: UiBindingPath::from_str("state.mode").unwrap(),
                    value_param: "value".to_owned(),
                },
            )
            .with_param(
                "value",
                UiActionParamSchema::required(UiActionParamType::Binding(UiBindingType::Enum {
                    values: vec!["basic".to_owned(), "advanced".to_owned()],
                })),
            ),
        ] {
            registry.register(descriptor).unwrap();
        }
        registry
    }

    fn host_bindings() -> BTreeMap<UiHostBindingKey, UiBindingType> {
        BTreeMap::from([
            (
                UiHostBindingKey::new(
                    UiBindingScope::Owner,
                    UiBindingPath::from_str("state.ready").unwrap(),
                ),
                UiBindingType::Bool,
            ),
            (
                UiHostBindingKey::new(
                    UiBindingScope::Document,
                    UiBindingPath::from_str("state.progress").unwrap(),
                ),
                UiBindingType::Number,
            ),
        ])
    }

    fn descriptor_for_registry_test(id: &str, kind: UiRegisteredActionKind) -> UiActionDescriptor {
        UiActionDescriptor::new(
            UiActionId::from_str(id).unwrap(),
            UiDocumentId::from_str("binding.actions").unwrap(),
            "binding_owner",
            kind,
        )
    }

    fn assert_registration_error(descriptor: UiActionDescriptor, expected: UiActionRegistryError) {
        assert_eq!(
            UiActionRegistry::default()
                .register(descriptor)
                .unwrap_err(),
            expected
        );
    }

    fn dispatch(
        document: &ValidatedUiDocument,
        registry: &UiActionRegistry,
        node_id: &str,
        owner: &str,
        owner_alive: bool,
    ) -> Result<UiActionDispatch, UiBindingActionError> {
        let node_id = UiNodeId::from_str(node_id).unwrap();
        registry.dispatch(
            document,
            &UiActionDispatchContext {
                owner: owner.to_owned(),
                owner_alive,
                source_node: node_id,
            },
        )
    }

    fn binding<'a>(
        bindings: &'a BTreeMap<UiBindingPath, UiBindingDeclaration>,
        path: &str,
    ) -> &'a UiBindingDeclaration {
        bindings
            .get(&UiBindingPath::from_str(path).unwrap())
            .unwrap()
    }

    fn assert_binding_error(error: super::super::UiDocumentError, code: &str) {
        let super::super::UiDocumentError::InvalidBindingAction { errors } = error else {
            panic!("expected binding/action error {code}, got {error:?}");
        };
        assert!(
            errors.iter().any(|error| error.code == code),
            "missing {code}: {errors:?}"
        );
    }

    fn assert_dispatch_error(result: Result<UiActionDispatch, UiBindingActionError>, code: &str) {
        assert_eq!(result.unwrap_err().code, code);
    }
}
