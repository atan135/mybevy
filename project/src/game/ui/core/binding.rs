#![allow(dead_code)]

use bevy::prelude::*;
use std::collections::HashMap;

use crate::game::ui::widgets::DisabledButton;

pub(in crate::game) struct UiBindingPlugin;

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
    bools: HashMap<UiBindingPath, bool>,
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

    pub(in crate::game) fn set_bool(&mut self, path: impl AsRef<str>, value: bool) -> bool {
        let Some(path) = UiBindingPath::new(path) else {
            return false;
        };

        self.set_bool_path(path, value)
    }

    pub(in crate::game) fn set_bool_path(&mut self, path: UiBindingPath, value: bool) -> bool {
        if self.bools.get(&path) == Some(&value) {
            return false;
        }

        self.bools.insert(path, value);
        true
    }

    pub(in crate::game) fn bool(&self, path: impl AsRef<str>) -> Option<bool> {
        let path = UiBindingPath::new(path)?;
        self.bool_path(&path)
    }

    pub(in crate::game) fn bool_path(&self, path: &UiBindingPath) -> Option<bool> {
        self.bools.get(path).copied()
    }

    #[allow(dead_code)]
    pub(in crate::game) fn remove_text(&mut self, path: impl AsRef<str>) -> bool {
        let Some(path) = UiBindingPath::new(path) else {
            return false;
        };

        self.texts.remove(&path).is_some()
    }

    #[allow(dead_code)]
    pub(in crate::game) fn remove_bool(&mut self, path: impl AsRef<str>) -> bool {
        let Some(path) = UiBindingPath::new(path) else {
            return false;
        };

        self.bools.remove(&path).is_some()
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

fn apply_bound_visibility(
    values: Res<UiBindingValues>,
    mut nodes: Query<(Ref<UiBoundVisibility>, &mut Visibility)>,
) {
    let values_changed = values.is_changed();

    for (bound_visibility, mut visibility) in &mut nodes {
        if !values_changed && !bound_visibility.is_changed() {
            continue;
        }

        let value = values.bool_path(&bound_visibility.path).unwrap_or(false);
        let next_visibility = visibility_from_bound_bool(value, bound_visibility.mode);
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
}
