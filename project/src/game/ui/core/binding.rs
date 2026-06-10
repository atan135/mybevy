#![allow(dead_code)]

use bevy::prelude::*;
use std::collections::HashMap;

pub(in crate::game) struct UiBindingPlugin;

impl Plugin for UiBindingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiBindingValues>()
            .configure_sets(Update, UiBindingSystems::Apply)
            .add_systems(Update, apply_bound_texts.in_set(UiBindingSystems::Apply));
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(in crate::game) enum UiBindingSystems {
    Apply,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(in crate::game) struct UiBindingPath(String);

impl UiBindingPath {
    pub(in crate::game) fn new(path: impl AsRef<str>) -> Option<Self> {
        normalize_binding_path(path.as_ref()).map(Self)
    }

    pub(in crate::game) fn as_str(&self) -> &str {
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
pub(in crate::game) struct UiBindingValues {
    texts: HashMap<UiBindingPath, String>,
}

impl UiBindingValues {
    pub(in crate::game) fn set_text(
        &mut self,
        path: impl AsRef<str>,
        value: impl Into<String>,
    ) -> bool {
        let Some(path) = UiBindingPath::new(path) else {
            return false;
        };

        self.set_text_path(path, value)
    }

    pub(in crate::game) fn set_text_path(
        &mut self,
        path: UiBindingPath,
        value: impl Into<String>,
    ) -> bool {
        let value = value.into();
        if self.texts.get(&path) == Some(&value) {
            return false;
        }

        self.texts.insert(path, value);
        true
    }

    pub(in crate::game) fn text(&self, path: impl AsRef<str>) -> Option<&str> {
        let path = UiBindingPath::new(path)?;
        self.text_path(&path)
    }

    pub(in crate::game) fn text_path(&self, path: &UiBindingPath) -> Option<&str> {
        self.texts.get(path).map(String::as_str)
    }

    #[allow(dead_code)]
    pub(in crate::game) fn remove_text(&mut self, path: impl AsRef<str>) -> bool {
        let Some(path) = UiBindingPath::new(path) else {
            return false;
        };

        self.texts.remove(&path).is_some()
    }
}

#[derive(Clone, Debug, Component, Eq, PartialEq)]
pub(in crate::game) struct UiBoundText {
    pub path: UiBindingPath,
    pub fallback: String,
}

impl UiBoundText {
    pub(in crate::game) fn new(path: impl AsRef<str>) -> Option<Self> {
        Self::with_fallback(path, "")
    }

    pub(in crate::game) fn with_fallback(
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
pub(in crate::game) enum UiVisibilityBindingMode {
    #[default]
    VisibleWhenTrue,
    HiddenWhenTrue,
}

#[derive(Clone, Debug, Component, Eq, PartialEq)]
pub(in crate::game) struct UiBoundVisibility {
    pub path: UiBindingPath,
    pub mode: UiVisibilityBindingMode,
}

impl UiBoundVisibility {
    pub(in crate::game) fn new(path: impl AsRef<str>) -> Option<Self> {
        Self::with_mode(path, UiVisibilityBindingMode::VisibleWhenTrue)
    }

    pub(in crate::game) fn with_mode(
        path: impl AsRef<str>,
        mode: UiVisibilityBindingMode,
    ) -> Option<Self> {
        Some(Self {
            path: UiBindingPath::new(path)?,
            mode,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub(in crate::game) enum UiDisabledBindingMode {
    #[default]
    DisabledWhenTrue,
    EnabledWhenTrue,
}

#[derive(Clone, Debug, Component, Eq, PartialEq)]
pub(in crate::game) struct UiBoundDisabled {
    pub path: UiBindingPath,
    pub mode: UiDisabledBindingMode,
}

impl UiBoundDisabled {
    pub(in crate::game) fn new(path: impl AsRef<str>) -> Option<Self> {
        Self::with_mode(path, UiDisabledBindingMode::DisabledWhenTrue)
    }

    pub(in crate::game) fn with_mode(
        path: impl AsRef<str>,
        mode: UiDisabledBindingMode,
    ) -> Option<Self> {
        Some(Self {
            path: UiBindingPath::new(path)?,
            mode,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::game) enum UiDisabledMarkerIntent {
    Insert,
    Remove,
}

pub(in crate::game) fn visibility_from_bool(is_visible: bool) -> Visibility {
    if is_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    }
}

pub(in crate::game) fn visibility_from_bound_bool(
    value: bool,
    mode: UiVisibilityBindingMode,
) -> Visibility {
    match mode {
        UiVisibilityBindingMode::VisibleWhenTrue => visibility_from_bool(value),
        UiVisibilityBindingMode::HiddenWhenTrue => visibility_from_bool(!value),
    }
}

pub(in crate::game) fn is_disabled_from_bound_bool(
    value: bool,
    mode: UiDisabledBindingMode,
) -> bool {
    match mode {
        UiDisabledBindingMode::DisabledWhenTrue => value,
        UiDisabledBindingMode::EnabledWhenTrue => !value,
    }
}

pub(in crate::game) fn disabled_marker_intent(is_disabled: bool) -> UiDisabledMarkerIntent {
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
    fn bound_visibility_and_disabled_reject_invalid_paths() {
        assert!(UiBoundVisibility::new("menu.visible").is_some());
        assert!(UiBoundDisabled::new("menu.disabled").is_some());
        assert!(UiBoundVisibility::new("menu..visible").is_none());
        assert!(UiBoundDisabled::new(" ").is_none());
    }
}
