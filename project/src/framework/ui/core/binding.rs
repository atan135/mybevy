#![allow(dead_code)]

use bevy::prelude::*;
use std::collections::HashMap;

use crate::framework::ui::document::{
    UiBindingDeclaration, UiBindingMissingBehavior, UiBindingScope as UiDocumentBindingScope,
    UiBindingValue, UiBindingVisibility, binding_value_matches,
};
use crate::framework::ui::widgets::DisabledButton;

pub(crate) struct UiBindingPlugin;

impl Plugin for UiBindingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiBindingValues>()
            .configure_sets(Update, UiBindingSystems::Apply)
            .add_systems(
                Update,
                (
                    apply_bound_texts,
                    apply_bound_visibility,
                    apply_bound_button_disabled,
                )
                    .in_set(UiBindingSystems::Apply),
            );
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(crate) enum UiBindingSystems {
    Apply,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct UiBindingPath(String);

impl UiBindingPath {
    pub(crate) fn new(path: impl AsRef<str>) -> Option<Self> {
        normalize_binding_path(path.as_ref()).map(Self)
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for UiBindingPath {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl AsRef<str> for UiBindingPath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Debug, Default, Resource)]
pub(crate) struct UiBindingValues {
    texts: HashMap<UiBindingPath, String>,
    bools: HashMap<UiBindingPath, bool>,
    typed_values: HashMap<UiBindingPath, UiBindingValue>,
    scoped_values: HashMap<UiScopedBindingKey, UiBindingValue>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct UiScopedBindingKey {
    document_id: String,
    owner: String,
    scope: UiDocumentBindingScope,
    path: UiBindingPath,
}

impl UiBindingValues {
    pub(crate) fn set_text(&mut self, path: impl AsRef<str>, value: impl Into<String>) -> bool {
        let Some(path) = UiBindingPath::new(path) else {
            return false;
        };

        self.set_text_path(path, value)
    }

    pub(crate) fn set_text_path(&mut self, path: UiBindingPath, value: impl Into<String>) -> bool {
        let value = value.into();
        if self.texts.get(&path) == Some(&value) {
            return false;
        }

        self.texts.insert(path, value);
        true
    }

    pub(crate) fn text(&self, path: impl AsRef<str>) -> Option<&str> {
        let path = UiBindingPath::new(path)?;
        self.text_path(&path)
    }

    pub(crate) fn text_path(&self, path: &UiBindingPath) -> Option<&str> {
        self.texts.get(path).map(String::as_str)
    }

    pub(crate) fn set_bool(&mut self, path: impl AsRef<str>, value: bool) -> bool {
        let Some(path) = UiBindingPath::new(path) else {
            return false;
        };

        self.set_bool_path(path, value)
    }

    pub(crate) fn set_bool_path(&mut self, path: UiBindingPath, value: bool) -> bool {
        if self.bools.get(&path) == Some(&value) {
            return false;
        }

        self.bools.insert(path, value);
        true
    }

    pub(crate) fn bool(&self, path: impl AsRef<str>) -> Option<bool> {
        let path = UiBindingPath::new(path)?;
        self.bool_path(&path)
    }

    pub(crate) fn bool_path(&self, path: &UiBindingPath) -> Option<bool> {
        self.bools.get(path).copied()
    }

    pub(crate) fn set_number(&mut self, path: impl AsRef<str>, value: f64) -> bool {
        let Some(path) = UiBindingPath::new(path) else {
            return false;
        };
        if !value.is_finite() {
            return false;
        }
        self.typed_values
            .insert(path, UiBindingValue::Number(value))
            != Some(UiBindingValue::Number(value))
    }

    pub(crate) fn number(&self, path: impl AsRef<str>) -> Option<f64> {
        let path = UiBindingPath::new(path)?;
        match self.typed_values.get(&path) {
            Some(UiBindingValue::Number(value)) => Some(*value),
            _ => None,
        }
    }

    pub(crate) fn set_visibility(
        &mut self,
        path: impl AsRef<str>,
        value: UiBindingVisibility,
    ) -> bool {
        let Some(path) = UiBindingPath::new(path) else {
            return false;
        };
        self.typed_values
            .insert(path, UiBindingValue::Visibility(value))
            != Some(UiBindingValue::Visibility(value))
    }

    pub(crate) fn visibility_path(&self, path: &UiBindingPath) -> Option<UiBindingVisibility> {
        match self.typed_values.get(path) {
            Some(UiBindingValue::Visibility(value)) => Some(*value),
            _ => None,
        }
    }

    pub(crate) fn set_enum(&mut self, path: impl AsRef<str>, value: impl Into<String>) -> bool {
        let Some(path) = UiBindingPath::new(path) else {
            return false;
        };
        let value = UiBindingValue::Enum(value.into());
        if self.typed_values.get(&path) == Some(&value) {
            return false;
        }
        self.typed_values.insert(path, value);
        true
    }

    pub(crate) fn enum_value(&self, path: impl AsRef<str>) -> Option<&str> {
        let path = UiBindingPath::new(path)?;
        match self.typed_values.get(&path) {
            Some(UiBindingValue::Enum(value)) => Some(value),
            _ => None,
        }
    }

    pub(crate) fn set_scoped(
        &mut self,
        document_id: &str,
        owner: &str,
        path: &crate::framework::ui::document::UiBindingPath,
        declaration: &UiBindingDeclaration,
        value: UiBindingValue,
    ) -> bool {
        if !binding_value_matches(&declaration.value_type, &value) {
            return false;
        }
        let Some(path) = UiBindingPath::new(path.as_str()) else {
            return false;
        };
        let key = UiScopedBindingKey {
            document_id: document_id.to_owned(),
            owner: if declaration.scope == UiDocumentBindingScope::Document {
                String::new()
            } else {
                owner.to_owned()
            },
            scope: declaration.scope,
            path,
        };
        if self.scoped_values.get(&key) == Some(&value) {
            return false;
        }
        self.scoped_values.insert(key, value);
        true
    }

    pub(crate) fn scoped_value(
        &self,
        document_id: &str,
        owner: &str,
        path: &crate::framework::ui::document::UiBindingPath,
        declaration: &UiBindingDeclaration,
    ) -> Option<UiBindingValue> {
        let path = UiBindingPath::new(path.as_str())?;
        let key = UiScopedBindingKey {
            document_id: document_id.to_owned(),
            owner: if declaration.scope == UiDocumentBindingScope::Document {
                String::new()
            } else {
                owner.to_owned()
            },
            scope: declaration.scope,
            path,
        };
        self.scoped_values.get(&key).cloned().or_else(|| {
            (declaration.missing == UiBindingMissingBehavior::UseDefault)
                .then_some(declaration.default.clone())
                .flatten()
        })
    }

    pub(crate) fn clear_owner(&mut self, owner: &str) -> usize {
        let before = self.scoped_values.len();
        self.scoped_values
            .retain(|key, _| key.scope == UiDocumentBindingScope::Document || key.owner != owner);
        before - self.scoped_values.len()
    }

    pub(crate) fn clear_instance(&mut self, document_id: &str, owner: &str) -> usize {
        let before = self.scoped_values.len();
        self.scoped_values.retain(|key, _| {
            key.scope == UiDocumentBindingScope::Document
                || key.document_id != document_id
                || key.owner != owner
        });
        before - self.scoped_values.len()
    }

    pub(crate) fn clear_document(&mut self, document_id: &str) -> usize {
        let before = self.scoped_values.len();
        self.scoped_values
            .retain(|key, _| key.document_id != document_id);
        before - self.scoped_values.len()
    }

    #[allow(dead_code)]
    pub(crate) fn remove_text(&mut self, path: impl AsRef<str>) -> bool {
        let Some(path) = UiBindingPath::new(path) else {
            return false;
        };

        self.texts.remove(&path).is_some()
    }

    #[allow(dead_code)]
    pub(crate) fn remove_bool(&mut self, path: impl AsRef<str>) -> bool {
        let Some(path) = UiBindingPath::new(path) else {
            return false;
        };

        self.bools.remove(&path).is_some()
    }
}

#[derive(Clone, Debug, Component, Eq, PartialEq)]
pub(crate) struct UiBoundText {
    pub path: UiBindingPath,
    pub fallback: String,
}

impl UiBoundText {
    pub(crate) fn new(path: impl AsRef<str>) -> Option<Self> {
        Self::with_fallback(path, "")
    }

    pub(crate) fn with_fallback(
        path: impl AsRef<str>,
        fallback: impl Into<String>,
    ) -> Option<Self> {
        Some(Self {
            path: UiBindingPath::new(path)?,
            fallback: fallback.into(),
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub(crate) enum UiVisibilityBindingMode {
    #[default]
    VisibleWhenTrue,
    HiddenWhenTrue,
}

#[derive(Clone, Debug, Component, Eq, PartialEq)]
pub(crate) struct UiBoundVisibility {
    pub path: UiBindingPath,
    pub mode: UiVisibilityBindingMode,
}

impl UiBoundVisibility {
    pub(crate) fn new(path: impl AsRef<str>) -> Option<Self> {
        Self::with_mode(path, UiVisibilityBindingMode::VisibleWhenTrue)
    }

    pub(crate) fn with_mode(path: impl AsRef<str>, mode: UiVisibilityBindingMode) -> Option<Self> {
        Some(Self {
            path: UiBindingPath::new(path)?,
            mode,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub(crate) enum UiDisabledBindingMode {
    #[default]
    DisabledWhenTrue,
    EnabledWhenTrue,
}

#[derive(Clone, Debug, Component, Eq, PartialEq)]
pub(crate) struct UiBoundDisabled {
    pub path: UiBindingPath,
    pub mode: UiDisabledBindingMode,
}

impl UiBoundDisabled {
    pub(crate) fn new(path: impl AsRef<str>) -> Option<Self> {
        Self::with_mode(path, UiDisabledBindingMode::DisabledWhenTrue)
    }

    pub(crate) fn with_mode(path: impl AsRef<str>, mode: UiDisabledBindingMode) -> Option<Self> {
        Some(Self {
            path: UiBindingPath::new(path)?,
            mode,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiDisabledMarkerIntent {
    Insert,
    Remove,
}

pub(crate) fn visibility_from_bool(is_visible: bool) -> Visibility {
    if is_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    }
}

pub(crate) fn visibility_from_bound_bool(value: bool, mode: UiVisibilityBindingMode) -> Visibility {
    match mode {
        UiVisibilityBindingMode::VisibleWhenTrue => visibility_from_bool(value),
        UiVisibilityBindingMode::HiddenWhenTrue => visibility_from_bool(!value),
    }
}

pub(crate) fn is_disabled_from_bound_bool(value: bool, mode: UiDisabledBindingMode) -> bool {
    match mode {
        UiDisabledBindingMode::DisabledWhenTrue => value,
        UiDisabledBindingMode::EnabledWhenTrue => !value,
    }
}

pub(crate) fn disabled_marker_intent(is_disabled: bool) -> UiDisabledMarkerIntent {
    if is_disabled {
        UiDisabledMarkerIntent::Insert
    } else {
        UiDisabledMarkerIntent::Remove
    }
}

fn apply_bound_texts(
    values: Res<UiBindingValues>,
    mut texts: Query<(Ref<UiBoundText>, &mut Text)>,
) {
    let values_changed = values.is_changed();

    for (bound_text, mut text) in &mut texts {
        if !values_changed && !bound_text.is_changed() {
            continue;
        }

        let next_text = values
            .text_path(&bound_text.path)
            .unwrap_or(&bound_text.fallback);
        if text.0 != next_text {
            text.0 = next_text.to_string();
        }
    }
}

fn apply_bound_visibility(
    values: Res<UiBindingValues>,
    mut nodes: Query<(Ref<UiBoundVisibility>, &mut Visibility)>,
) {
    let values_changed = values.is_changed();

    for (bound_visibility, mut visibility) in &mut nodes {
        if !values_changed && !bound_visibility.is_changed() {
            continue;
        }

        let next_visibility = if let Some(value) = values.visibility_path(&bound_visibility.path) {
            match value {
                UiBindingVisibility::Inherited => Visibility::Inherited,
                UiBindingVisibility::Visible => Visibility::Visible,
                UiBindingVisibility::Hidden => Visibility::Hidden,
            }
        } else {
            let value = values.bool_path(&bound_visibility.path).unwrap_or(false);
            visibility_from_bound_bool(value, bound_visibility.mode)
        };
        if *visibility != next_visibility {
            *visibility = next_visibility;
        }
    }
}

fn apply_bound_button_disabled(
    mut commands: Commands,
    values: Res<UiBindingValues>,
    buttons: Query<(Entity, Ref<UiBoundDisabled>, Has<DisabledButton>), With<Button>>,
) {
    let values_changed = values.is_changed();

    for (entity, bound_disabled, is_disabled) in &buttons {
        if !values_changed && !bound_disabled.is_changed() {
            continue;
        }

        let value = values.bool_path(&bound_disabled.path).unwrap_or(false);
        let next_disabled = is_disabled_from_bound_bool(value, bound_disabled.mode);
        match disabled_marker_intent(next_disabled) {
            UiDisabledMarkerIntent::Insert if !is_disabled => {
                commands.entity(entity).insert(DisabledButton);
            }
            UiDisabledMarkerIntent::Remove if is_disabled => {
                commands.entity(entity).remove::<DisabledButton>();
            }
            _ => {}
        }
    }
}

fn normalize_binding_path(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut segments = Vec::new();
    for segment in trimmed.split('.') {
        let segment = segment.trim();
        if segment.is_empty() {
            return None;
        }
        segments.push(segment);
    }

    Some(segments.join("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_path_normalizes_outer_and_segment_whitespace() {
        let path = UiBindingPath::new("  login . submit . enabled  ").unwrap();

        assert_eq!(path.as_str(), "login.submit.enabled");
        assert_eq!(path.to_string(), "login.submit.enabled");
    }

    #[test]
    fn binding_path_rejects_empty_or_ambiguous_segments() {
        assert!(UiBindingPath::new("").is_none());
        assert!(UiBindingPath::new("   ").is_none());
        assert!(UiBindingPath::new(".login").is_none());
        assert!(UiBindingPath::new("login..enabled").is_none());
        assert!(UiBindingPath::new("login.").is_none());
    }

    #[test]
    fn bound_text_constructor_keeps_fallback_and_normalized_path() {
        let bound = UiBoundText::with_fallback(" status . title ", "Loading").unwrap();

        assert_eq!(bound.path.as_str(), "status.title");
        assert_eq!(bound.fallback, "Loading");
    }

    #[test]
    fn binding_values_set_get_and_reject_invalid_paths() {
        let mut values = UiBindingValues::default();

        assert!(values.set_text(" gallery . binding . status ", "Ready"));
        assert_eq!(values.text("gallery.binding.status"), Some("Ready"));
        assert!(!values.set_text("gallery.binding.status", "Ready"));
        assert!(values.set_text("gallery.binding.status", "Updated"));
        assert_eq!(values.text("gallery.binding.status"), Some("Updated"));
        assert!(!values.set_text("gallery..binding", "Invalid"));
        assert_eq!(values.text("gallery..binding"), None);

        assert!(values.set_bool(" gallery . binding . visible ", true));
        assert_eq!(values.bool("gallery.binding.visible"), Some(true));
        assert!(!values.set_bool("gallery.binding.visible", true));
        assert!(values.set_bool("gallery.binding.visible", false));
        assert_eq!(values.bool("gallery.binding.visible"), Some(false));
        assert!(!values.set_bool("gallery..binding", true));
        assert_eq!(values.bool("gallery..binding"), None);
    }

    #[test]
    fn binding_values_remove_text_and_bool_values() {
        let mut values = UiBindingValues::default();

        values.set_text("gallery.binding.status", "Ready");
        values.set_bool("gallery.binding.visible", true);

        assert!(values.remove_text(" gallery . binding . status "));
        assert_eq!(values.text("gallery.binding.status"), None);
        assert!(!values.remove_text("gallery.binding.status"));
        assert!(!values.remove_text("gallery..binding"));

        assert!(values.remove_bool(" gallery . binding . visible "));
        assert_eq!(values.bool("gallery.binding.visible"), None);
        assert!(!values.remove_bool("gallery.binding.visible"));
        assert!(!values.remove_bool("gallery..binding"));
    }

    #[test]
    fn binding_values_keep_legacy_text_and_bool_channels_independent() {
        let mut values = UiBindingValues::default();

        assert!(values.set_text("gallery.shared", "Ready"));
        assert!(values.set_bool("gallery.shared", true));
        assert_eq!(values.text("gallery.shared"), Some("Ready"));
        assert_eq!(values.bool("gallery.shared"), Some(true));

        assert!(values.remove_text("gallery.shared"));
        assert_eq!(values.text("gallery.shared"), None);
        assert_eq!(values.bool("gallery.shared"), Some(true));
    }

    #[test]
    fn apply_bound_texts_uses_value_and_fallback() {
        let mut app = App::new();
        app.add_plugins(UiBindingPlugin);

        let value_entity = app
            .world_mut()
            .spawn((
                Text::new(""),
                UiBoundText::with_fallback("gallery.binding.status", "Fallback").unwrap(),
            ))
            .id();
        let fallback_entity = app
            .world_mut()
            .spawn((
                Text::new(""),
                UiBoundText::with_fallback("gallery.binding.missing", "Fallback").unwrap(),
            ))
            .id();

        app.world_mut()
            .resource_mut::<UiBindingValues>()
            .set_text("gallery.binding.status", "Bound value");
        app.update();

        assert_eq!(
            app.world().get::<Text>(value_entity).unwrap().0,
            "Bound value"
        );
        assert_eq!(
            app.world().get::<Text>(fallback_entity).unwrap().0,
            "Fallback"
        );
    }

    #[test]
    fn visibility_helpers_map_bool_to_bevy_visibility() {
        assert_eq!(visibility_from_bool(true), Visibility::Visible);
        assert_eq!(visibility_from_bool(false), Visibility::Hidden);
        assert_eq!(
            visibility_from_bound_bool(true, UiVisibilityBindingMode::VisibleWhenTrue),
            Visibility::Visible
        );
        assert_eq!(
            visibility_from_bound_bool(true, UiVisibilityBindingMode::HiddenWhenTrue),
            Visibility::Hidden
        );
    }

    #[test]
    fn apply_bound_visibility_uses_bool_values_and_false_fallback() {
        let mut app = App::new();
        app.add_plugins(UiBindingPlugin);

        let visible_entity = app
            .world_mut()
            .spawn((
                Visibility::Hidden,
                UiBoundVisibility::new("gallery.binding.visible").unwrap(),
            ))
            .id();
        let hidden_entity = app
            .world_mut()
            .spawn((
                Visibility::Visible,
                UiBoundVisibility::with_mode(
                    "gallery.binding.hidden",
                    UiVisibilityBindingMode::HiddenWhenTrue,
                )
                .unwrap(),
            ))
            .id();
        let fallback_entity = app
            .world_mut()
            .spawn((
                Visibility::Visible,
                UiBoundVisibility::new("gallery.binding.missing").unwrap(),
            ))
            .id();

        {
            let mut values = app.world_mut().resource_mut::<UiBindingValues>();
            values.set_bool("gallery.binding.visible", true);
            values.set_bool("gallery.binding.hidden", true);
        }
        app.update();

        assert_eq!(
            *app.world().get::<Visibility>(visible_entity).unwrap(),
            Visibility::Visible
        );
        assert_eq!(
            *app.world().get::<Visibility>(hidden_entity).unwrap(),
            Visibility::Hidden
        );
        assert_eq!(
            *app.world().get::<Visibility>(fallback_entity).unwrap(),
            Visibility::Hidden
        );
    }

    #[test]
    fn disabled_helpers_map_bool_to_marker_intent() {
        assert!(is_disabled_from_bound_bool(
            true,
            UiDisabledBindingMode::DisabledWhenTrue
        ));
        assert!(is_disabled_from_bound_bool(
            false,
            UiDisabledBindingMode::EnabledWhenTrue
        ));
        assert_eq!(disabled_marker_intent(true), UiDisabledMarkerIntent::Insert);
        assert_eq!(
            disabled_marker_intent(false),
            UiDisabledMarkerIntent::Remove
        );
    }

    #[test]
    fn apply_bound_button_disabled_inserts_and_removes_disabled_button() {
        let mut app = App::new();
        app.add_plugins(UiBindingPlugin);

        let button = app
            .world_mut()
            .spawn((
                Button,
                UiBoundDisabled::new("gallery.binding.disabled").unwrap(),
            ))
            .id();
        app.update();

        assert!(!app.world().entity(button).contains::<DisabledButton>());

        app.world_mut()
            .resource_mut::<UiBindingValues>()
            .set_bool("gallery.binding.disabled", true);
        app.update();

        assert!(app.world().entity(button).contains::<DisabledButton>());

        app.world_mut()
            .resource_mut::<UiBindingValues>()
            .set_bool("gallery.binding.disabled", false);
        app.update();

        assert!(!app.world().entity(button).contains::<DisabledButton>());
    }

    #[test]
    fn bound_visibility_and_disabled_reject_invalid_paths() {
        assert!(UiBoundVisibility::new("menu.visible").is_some());
        assert!(UiBoundDisabled::new("menu.disabled").is_some());
        assert!(UiBoundVisibility::new("menu..visible").is_none());
        assert!(UiBoundDisabled::new(" ").is_none());
    }

    #[test]
    fn binding_values_support_number_visibility_and_restricted_enum_types() {
        let mut values = UiBindingValues::default();

        assert!(values.set_number("gallery.progress", 0.75));
        assert_eq!(values.number("gallery.progress"), Some(0.75));
        assert!(!values.set_number("gallery.progress", f64::NAN));

        assert!(values.set_visibility("gallery.visibility", UiBindingVisibility::Hidden));
        let path = UiBindingPath::new("gallery.visibility").unwrap();
        assert_eq!(
            values.visibility_path(&path),
            Some(UiBindingVisibility::Hidden)
        );

        assert!(values.set_enum("gallery.mode", "advanced"));
        assert_eq!(values.enum_value("gallery.mode"), Some("advanced"));
    }

    #[test]
    fn scoped_binding_values_apply_defaults_and_cleanup_owner_and_document_lifetimes() {
        use crate::framework::ui::document::{
            UiBindingPath as UiDocumentBindingPath, UiBindingType,
        };
        use std::str::FromStr;

        let mut values = UiBindingValues::default();
        let local_path = UiDocumentBindingPath::from_str("state.local").unwrap();
        let owner_path = UiDocumentBindingPath::from_str("state.owner").unwrap();
        let document_path = UiDocumentBindingPath::from_str("state.document").unwrap();
        let local = UiBindingDeclaration {
            scope: UiDocumentBindingScope::Local,
            value_type: UiBindingType::String,
            default: Some(UiBindingValue::String("fallback".to_owned())),
            missing: UiBindingMissingBehavior::UseDefault,
        };
        let owner = UiBindingDeclaration {
            scope: UiDocumentBindingScope::Owner,
            value_type: UiBindingType::Bool,
            default: None,
            missing: UiBindingMissingBehavior::UseConsumerFallback,
        };
        let document = UiBindingDeclaration {
            scope: UiDocumentBindingScope::Document,
            value_type: UiBindingType::Number,
            default: None,
            missing: UiBindingMissingBehavior::UseConsumerFallback,
        };

        assert_eq!(
            values.scoped_value("binding.actions", "owner_a", &local_path, &local),
            Some(UiBindingValue::String("fallback".to_owned()))
        );
        assert!(values.set_scoped(
            "binding.actions",
            "owner_a",
            &local_path,
            &local,
            UiBindingValue::String("local".to_owned())
        ));
        assert!(values.set_scoped(
            "binding.actions",
            "owner_a",
            &owner_path,
            &owner,
            UiBindingValue::Bool(true)
        ));
        assert!(values.set_scoped(
            "binding.actions",
            "owner_a",
            &document_path,
            &document,
            UiBindingValue::Number(1.0)
        ));
        assert!(!values.set_scoped(
            "binding.actions",
            "owner_a",
            &owner_path,
            &owner,
            UiBindingValue::String("wrong".to_owned())
        ));

        assert_eq!(values.clear_owner("owner_a"), 2);
        assert_eq!(
            values.scoped_value("binding.actions", "owner_b", &document_path, &document),
            Some(UiBindingValue::Number(1.0))
        );
        assert_eq!(values.clear_document("binding.actions"), 1);
        assert!(
            values
                .scoped_value("binding.actions", "owner_b", &document_path, &document)
                .is_none()
        );
    }

    #[test]
    fn scoped_binding_instance_cleanup_preserves_other_owners_and_document_scope() {
        use crate::framework::ui::document::{
            UiBindingPath as UiDocumentBindingPath, UiBindingType,
        };
        use std::str::FromStr;

        let path = UiDocumentBindingPath::from_str("state.value").unwrap();
        let local = UiBindingDeclaration {
            scope: UiDocumentBindingScope::Local,
            value_type: UiBindingType::String,
            default: None,
            missing: UiBindingMissingBehavior::UseConsumerFallback,
        };
        let document = UiBindingDeclaration {
            scope: UiDocumentBindingScope::Document,
            value_type: UiBindingType::String,
            default: None,
            missing: UiBindingMissingBehavior::UseConsumerFallback,
        };
        let mut values = UiBindingValues::default();
        assert!(values.set_scoped(
            "example.document",
            "owner_a",
            &path,
            &local,
            UiBindingValue::String("a".to_owned()),
        ));
        assert!(values.set_scoped(
            "example.document",
            "owner_b",
            &path,
            &local,
            UiBindingValue::String("b".to_owned()),
        ));
        assert!(values.set_scoped(
            "example.document",
            "owner_a",
            &path,
            &document,
            UiBindingValue::String("shared".to_owned()),
        ));

        assert_eq!(values.clear_instance("example.document", "owner_a"), 1);
        assert_eq!(
            values.scoped_value("example.document", "owner_b", &path, &local),
            Some(UiBindingValue::String("b".to_owned()))
        );
        assert_eq!(
            values.scoped_value("example.document", "owner_b", &path, &document),
            Some(UiBindingValue::String("shared".to_owned()))
        );
    }
}
