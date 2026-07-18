use std::collections::{HashMap, HashSet};

use bevy::{
    ecs::system::SystemParam, picking::Pickable, prelude::*, text::TextLayoutInfo, ui::OverflowAxis,
};
use serde::Serialize;

use crate::framework::ui::{
    core::{UiInputState, UiPanelKind, UiPanelRoot, UiViewport, focus::UiFocusState},
    document::{UiDocumentNodeAuditMarker, UiDocumentPanel, UiDocumentRuntimeRoot},
    overlays::toast::UiToastRoot,
    widgets::controls::{UiTextInputCaret, UiTextInputCaretMeasure, UiTextInputPlaceholder},
    widgets::{
        DisabledButton, DisabledTextInput, FocusableButton, LoadingButton, UiControlKind,
        UiControlMeta, UiIconButton, UiScrollView, UiTextInput, UiTextInputValue,
    },
};

pub(crate) const UI_SEMANTIC_TREE_SCHEMA_VERSION: u32 = 3;
const ROUNDING_SCALE: f32 = 64.0;

#[derive(SystemParam)]
#[allow(clippy::type_complexity)]
pub(crate) struct UiAuditSemanticWorld<'w, 's> {
    nodes: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static Name>,
            &'static ComputedNode,
            &'static UiGlobalTransform,
            Option<&'static Node>,
            Option<&'static InheritedVisibility>,
            Option<&'static Visibility>,
            Option<&'static ZIndex>,
            Option<&'static Pickable>,
        ),
    >,
    parents: Query<'w, 's, &'static ChildOf>,
    children: Query<'w, 's, &'static Children>,
    texts: Query<'w, 's, (&'static Text, Option<&'static TextLayoutInfo>)>,
    text_input_placeholders: Query<'w, 's, &'static UiTextInputPlaceholder>,
    text_input_values: Query<'w, 's, &'static UiTextInputValue>,
    nonsemantic_measurements:
        Query<'w, 's, (), Or<(With<UiTextInputCaret>, With<UiTextInputCaretMeasure>)>>,
    document_nodes: Query<'w, 's, &'static UiDocumentNodeAuditMarker>,
    document_roots: Query<'w, 's, &'static UiDocumentRuntimeRoot>,
    controls: Query<
        'w,
        's,
        (
            Option<&'static UiControlMeta>,
            Has<Button>,
            Has<UiIconButton>,
            Option<&'static UiIconButton>,
            Has<UiTextInput>,
            Has<DisabledButton>,
            Has<DisabledTextInput>,
            Has<LoadingButton>,
            Option<&'static Interaction>,
        ),
    >,
    scroll_views: Query<'w, 's, (), With<UiScrollView>>,
    images: Query<'w, 's, (), With<ImageNode>>,
    panels: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static Name>,
            &'static UiPanelRoot,
            Option<&'static InheritedVisibility>,
            Option<&'static Visibility>,
            Option<&'static ZIndex>,
            Option<&'static Pickable>,
        ),
    >,
    toast_roots: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static Name>,
            Option<&'static InheritedVisibility>,
            Option<&'static Visibility>,
            Option<&'static ZIndex>,
            Option<&'static Pickable>,
        ),
        With<UiToastRoot>,
    >,
    focusable: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static InheritedVisibility>,
            Has<DisabledButton>,
            Has<DisabledTextInput>,
            Has<LoadingButton>,
        ),
        (With<Button>, With<FocusableButton>),
    >,
    focus_state: Res<'w, UiFocusState>,
    input_state: Res<'w, UiInputState>,
    current_owner: Res<'w, crate::framework::ui::core::UiCurrentOwner>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct UiAuditSemanticTree {
    schema_version: u32,
    coordinate_space: &'static str,
    rect_convention: &'static str,
    rounding: &'static str,
    target_root_id: String,
    viewport: UiAuditSemanticRect,
    safe_area: UiAuditSemanticRect,
    nodes: Vec<UiAuditSemanticNode>,
    panels: Vec<UiAuditSemanticPanel>,
}

impl UiAuditSemanticTree {
    pub(crate) fn missing(viewport: &UiViewport) -> Self {
        let viewport_rect = viewport_rect(viewport);
        Self {
            schema_version: UI_SEMANTIC_TREE_SCHEMA_VERSION,
            coordinate_space: "logical_pixels",
            rect_convention: "half_open",
            rounding: "nearest_1_64_half_away_from_zero",
            target_root_id: "audit-root/missing".to_owned(),
            viewport: viewport_rect,
            safe_area: safe_area_rect(viewport),
            nodes: Vec::new(),
            panels: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
struct UiAuditSemanticRect {
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
}

impl UiAuditSemanticRect {
    fn from_extents(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Self {
        Self {
            min_x: rounded(min_x),
            min_y: rounded(min_y),
            max_x: rounded(max_x),
            max_y: rounded(max_y),
        }
    }

    fn width(self) -> f32 {
        self.max_x - self.min_x
    }

    fn height(self) -> f32 {
        self.max_y - self.min_y
    }

    fn intersect(self, other: Self) -> Self {
        let min_x = self.min_x.max(other.min_x);
        let min_y = self.min_y.max(other.min_y);
        let max_x = self.max_x.min(other.max_x).max(min_x);
        let max_y = self.max_y.min(other.max_y).max(min_y);
        Self::from_extents(min_x, min_y, max_x, max_y)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct UiAuditSemanticNode {
    stable_id: String,
    identity_source: &'static str,
    capture_entity: String,
    entity_name: Option<String>,
    stack_index: u32,
    parent_id: Option<String>,
    depth: u32,
    role: &'static str,
    visible: bool,
    fully_clipped: bool,
    bounds: UiAuditSemanticRect,
    clip_bounds: UiAuditSemanticRect,
    measured_text_bounds: Option<UiAuditSemanticRect>,
    text_nonempty: bool,
    has_visible_label: bool,
    interaction: &'static str,
    disabled: bool,
    loading: bool,
    focused: bool,
    scroll: Option<UiAuditSemanticScroll>,
    document_id: Option<String>,
    node_id: Option<String>,
    source_path: Option<String>,
    panel_id: Option<String>,
    likely_files: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
struct UiAuditSemanticScroll {
    viewport_height: f32,
    content_height: f32,
    max_offset: f32,
    current_offset: f32,
    content_reachable: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct UiAuditSemanticPanel {
    stable_id: String,
    capture_entity: String,
    entity_name: Option<String>,
    likely_files: Vec<String>,
    kind: &'static str,
    layer_policy: &'static str,
    visible: bool,
    z_index: i32,
    has_focusable_descendants: bool,
    focused_descendant: bool,
    focused_stable_id: Option<String>,
    active_focus_scope: bool,
    focus_scope_enforced: bool,
    focus_suppressed: bool,
    blocks_lower_input: bool,
    pickable_blocks_lower: bool,
    input_block_reason: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum RuntimeSemanticRole {
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

impl RuntimeSemanticRole {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Layout => "layout",
            Self::Text => "text",
            Self::CriticalText => "critical_text",
            Self::Button => "button",
            Self::IconButton => "icon_button",
            Self::TextInput => "text_input",
            Self::Scroll => "scroll",
            Self::Image => "image",
            Self::Modal => "modal",
            Self::Loading => "loading",
            Self::Floating => "floating",
            Self::Toast => "toast",
        }
    }
}

pub(crate) fn collect_semantic_tree(
    world: &UiAuditSemanticWorld,
    target_root: Option<Entity>,
    viewport: &UiViewport,
) -> UiAuditSemanticTree {
    let Some(target_root) = target_root else {
        return UiAuditSemanticTree::missing(viewport);
    };
    if world.nodes.get(target_root).is_err() {
        return UiAuditSemanticTree::missing(viewport);
    }

    let audit_roots = audit_roots(target_root, world);
    let mut entities = world
        .nodes
        .iter()
        .filter_map(|(entity, _, computed, _, _, _, _, _, _)| {
            nearest_audit_root(entity, &audit_roots, &world.parents).map(|root| {
                (
                    entity,
                    root,
                    depth_from_root(entity, root, &world.parents),
                    computed.stack_index(),
                )
            })
        })
        .collect::<Vec<_>>();
    entities.sort_by_key(|(entity, root, depth, stack)| (*root, *depth, *stack, *entity));
    let entity_set = entities
        .iter()
        .map(|(entity, _, _, _)| *entity)
        .collect::<HashSet<_>>();
    let mut stable_ids = HashMap::new();
    let mut identity_sources = HashMap::new();
    for (entity, root, _, _) in &entities {
        let parent = nearest_semantic_parent(*entity, &entity_set, &world.parents);
        let parent_id = parent.and_then(|parent| stable_ids.get(&parent)).cloned();
        let (stable_id, identity_source) =
            semantic_identity(*entity, *root, parent, parent_id.as_deref(), world);
        stable_ids.insert(*entity, stable_id);
        identity_sources.insert(*entity, identity_source);
    }

    let mut nodes = entities
        .iter()
        .filter_map(|(entity, root, depth, _)| {
            collect_node(
                *entity,
                *depth,
                *root,
                viewport,
                &entity_set,
                &stable_ids,
                &identity_sources,
                world,
            )
        })
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| left.stable_id.cmp(&right.stable_id));
    let panels = collect_panels(world, &stable_ids, &audit_roots, target_root);
    let target_root_id = stable_ids
        .get(&target_root)
        .cloned()
        .unwrap_or_else(|| "audit-root/missing".to_owned());
    UiAuditSemanticTree {
        schema_version: UI_SEMANTIC_TREE_SCHEMA_VERSION,
        coordinate_space: "logical_pixels",
        rect_convention: "half_open",
        rounding: "nearest_1_64_half_away_from_zero",
        target_root_id,
        viewport: viewport_rect(viewport),
        safe_area: safe_area_rect(viewport),
        nodes,
        panels,
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_node(
    entity: Entity,
    depth: u32,
    audit_root: Entity,
    viewport: &UiViewport,
    entity_set: &HashSet<Entity>,
    stable_ids: &HashMap<Entity, String>,
    identity_sources: &HashMap<Entity, &'static str>,
    world: &UiAuditSemanticWorld,
) -> Option<UiAuditSemanticNode> {
    let (_, name, computed, transform, _, inherited_visibility, visibility, _, _) =
        world.nodes.get(entity).ok()?;
    let role = semantic_role(entity, name, world);
    let bounds = logical_rect(computed, transform);
    let clip_bounds = effective_clip(entity, audit_root, viewport, world);
    let visible = inherited_visibility.is_none_or(|visibility| visibility.get())
        && visibility.is_none_or(|visibility| *visibility != Visibility::Hidden);
    let fully_clipped = clip_bounds.width() <= 0.0 || clip_bounds.height() <= 0.0;
    let parent_id = nearest_semantic_parent(entity, entity_set, &world.parents)
        .and_then(|parent| stable_ids.get(&parent))
        .cloned();
    let document = nearest_document_marker(entity, world);
    let panel = nearest_panel(entity, world);
    let toast_panel_id = nearest_toast_root(entity, world)
        .map(|root| format!("toast:{}", toast_root_ordinal(root, world)));
    let text = world.texts.get(entity).ok();
    let measured_text_bounds = text.and_then(|(_, layout)| {
        layout.map(|layout| logical_measured_text_rect(computed, transform, layout))
    });
    let text_nonempty = text.is_some_and(|(text, _)| !text.0.trim().is_empty());
    let control = world.controls.get(entity).ok();
    let interaction = control
        .and_then(|(_, _, _, _, _, _, _, _, interaction)| interaction)
        .map_or("none", interaction_name);
    let disabled = control.is_some_and(|(_, _, _, _, _, disabled_button, disabled_input, _, _)| {
        disabled_button || disabled_input
    });
    let loading = control.is_some_and(|(_, _, _, _, _, _, _, loading, _)| loading);
    let has_visible_label =
        visible_label(entity, visible, text_nonempty, audit_root, viewport, world);
    let scroll = (role == RuntimeSemanticRole::Scroll).then(|| {
        let scale = computed.inverse_scale_factor();
        let raw_viewport_height = computed.size().y * scale;
        let raw_content_height = computed.content_size().y * scale;
        let raw_max_offset = raw_content_height - raw_viewport_height;
        let raw_current_offset = computed.scroll_position.y * scale;
        let metrics_valid = [
            raw_viewport_height,
            raw_content_height,
            raw_max_offset,
            raw_current_offset,
        ]
        .iter()
        .all(|value| value.is_finite() && *value >= 0.0);
        let viewport_height = finite_nonnegative(raw_viewport_height);
        let content_height = finite_nonnegative(raw_content_height);
        let max_offset = finite_nonnegative(raw_max_offset);
        let current_offset = finite_nonnegative(raw_current_offset);
        let scroll_y = world
            .nodes
            .get(entity)
            .ok()
            .and_then(|(_, _, _, _, node, _, _, _, _)| node)
            .is_some_and(|node| node.overflow.y == OverflowAxis::Scroll);
        UiAuditSemanticScroll {
            viewport_height: rounded(viewport_height),
            content_height: rounded(content_height),
            max_offset: rounded(max_offset),
            current_offset: rounded(current_offset),
            content_reachable: metrics_valid
                && scroll_y
                && current_offset <= max_offset + (1.0 / ROUNDING_SCALE),
        }
    });
    let likely_files = likely_files(
        document,
        panel.map(|(_, panel)| panel),
        toast_panel_id.is_some(),
    );
    Some(UiAuditSemanticNode {
        stable_id: stable_ids.get(&entity)?.clone(),
        identity_source: identity_sources
            .get(&entity)
            .copied()
            .unwrap_or("hierarchy_fallback"),
        capture_entity: format!("{entity:?}"),
        entity_name: name.map(|name| name.as_str().to_owned()),
        stack_index: computed.stack_index(),
        parent_id,
        depth,
        role: role.as_str(),
        visible,
        fully_clipped,
        bounds,
        clip_bounds,
        measured_text_bounds,
        text_nonempty,
        has_visible_label,
        interaction,
        disabled,
        loading,
        focused: world.focus_state.focused_entity == Some(entity),
        scroll,
        document_id: document.map(|marker| marker.document_id.as_str().to_owned()),
        node_id: document.map(|marker| marker.node_id.as_str().to_owned()),
        source_path: document.map(|marker| marker.source_path.clone()),
        panel_id: panel
            .map(|(_, panel)| panel.id.to_string())
            .or(toast_panel_id),
        likely_files,
    })
}

fn collect_panels(
    world: &UiAuditSemanticWorld,
    stable_ids: &HashMap<Entity, String>,
    audit_roots: &HashSet<Entity>,
    target_root: Entity,
) -> Vec<UiAuditSemanticPanel> {
    let focused = world.focus_state.focused_entity;
    let visible_panels = world
        .panels
        .iter()
        .filter(|(entity, _, _, _, _, _, _)| {
            audit_roots.contains(entity)
                || *entity == target_root
                || world
                    .parents
                    .iter_ancestors(target_root)
                    .any(|ancestor| ancestor == *entity)
        })
        .filter(|(_, _, _, inherited, visibility, _, _)| {
            inherited.is_none_or(|visibility| visibility.get())
                && visibility.is_none_or(|visibility| *visibility != Visibility::Hidden)
        })
        .collect::<Vec<_>>();
    let active_focus_panel = visible_panels
        .iter()
        .filter(|(_, _, panel, _, _, _, _)| panel.kind == UiPanelKind::BlockingOverlay)
        .max_by_key(|(entity, _, _, _, _, z, _)| (z.map_or(0, |z| z.0), *entity))
        .map(|(entity, _, _, _, _, _, _)| *entity);
    let active_focus_panel = active_focus_panel.or_else(|| {
        visible_panels
            .iter()
            .filter(|(entity, _, panel, _, _, _, _)| {
                panel.kind == UiPanelKind::Modal
                    || (panel.kind == UiPanelKind::Floating
                        && panel_has_focusable_descendants(*entity, world))
            })
            .max_by_key(|(entity, _, panel, _, _, z, _)| {
                ((z.map_or(0, |z| z.0), *entity), panel_rank(panel.kind))
            })
            .map(|(entity, _, _, _, _, _, _)| *entity)
    });
    let mut panels = visible_panels
        .into_iter()
        .map(|(entity, name, panel, inherited, visibility, z_index, _)| {
            let visible = inherited.is_none_or(|visibility| visibility.get())
                && visibility.is_none_or(|visibility| *visibility != Visibility::Hidden);
            let focused_descendant = focused.is_some_and(|focused| {
                focused == entity
                    || world
                        .parents
                        .iter_ancestors(focused)
                        .any(|item| item == entity)
            });
            let focusables = panel_has_focusable_descendants(entity, world);
            let blocks_lower_input = world.input_state.pointer_blocked
                && (world.input_state.top_blocking_panel == Some(panel.id)
                    || world.input_state.focused_panel == Some(panel.id));
            UiAuditSemanticPanel {
                stable_id: panel.id.to_string(),
                capture_entity: format!("{entity:?}"),
                entity_name: name.map(|name| name.as_str().to_owned()),
                likely_files: panel_likely_files(panel),
                kind: panel_kind(panel.kind),
                layer_policy: panel_layer_policy(panel),
                visible,
                z_index: z_index.map_or(0, |z| z.0),
                has_focusable_descendants: focusables,
                focused_descendant,
                focused_stable_id: focused.and_then(|entity| stable_ids.get(&entity)).cloned(),
                active_focus_scope: active_focus_panel == Some(entity),
                focus_scope_enforced: matches!(
                    panel.kind,
                    UiPanelKind::Modal | UiPanelKind::BlockingOverlay
                ) && active_focus_panel == Some(entity)
                    || panel.kind == UiPanelKind::Floating && focused_descendant,
                focus_suppressed: focused.is_none() || focused_descendant,
                blocks_lower_input,
                pickable_blocks_lower: subtree_blocks_lower(entity, world),
                input_block_reason: if blocks_lower_input {
                    world.input_state.pointer_block_reason.clone()
                } else {
                    "none".to_owned()
                },
            }
        })
        .collect::<Vec<_>>();

    for (ordinal, toast_entity) in visible_audit_toast_roots(world).into_iter().enumerate() {
        let Ok((_, name, _, _, z_index, _)) = world.toast_roots.get(toast_entity) else {
            continue;
        };
        panels.push(UiAuditSemanticPanel {
            stable_id: format!("toast:{ordinal}"),
            capture_entity: format!("{toast_entity:?}"),
            entity_name: name.map(|name| name.as_str().to_owned()),
            likely_files: vec!["project/src/framework/ui/overlays/toast.rs".to_owned()],
            kind: "toast",
            layer_policy: "toast",
            visible: true,
            z_index: z_index.map_or(0, |z| z.0),
            has_focusable_descendants: false,
            focused_descendant: false,
            focused_stable_id: None,
            active_focus_scope: false,
            focus_scope_enforced: false,
            focus_suppressed: focused.is_none(),
            blocks_lower_input: false,
            pickable_blocks_lower: subtree_blocks_lower(toast_entity, world),
            input_block_reason: "none".to_owned(),
        });
    }
    panels.sort_by(|left, right| left.stable_id.cmp(&right.stable_id));
    panels
}

fn semantic_identity(
    entity: Entity,
    audit_root: Entity,
    parent: Option<Entity>,
    parent_id: Option<&str>,
    world: &UiAuditSemanticWorld,
) -> (String, &'static str) {
    if entity == audit_root {
        if let Ok((_, _, panel, _, _, _, _)) = world.panels.get(entity) {
            return (format!("panel:{}/root", panel.id), "named_hierarchy");
        }
        if world.toast_roots.contains(entity) {
            return (
                format!("toast:{}/root", toast_root_ordinal(entity, world)),
                "hierarchy_fallback",
            );
        }
    }
    if let Ok(marker) = world.document_nodes.get(entity) {
        let stable_id = nearest_document_runtime_root(entity, world).map_or_else(
            || {
                stable_hierarchy_id(
                    parent_id.unwrap_or("audit-root/root"),
                    &format!("node:{}", sanitize_segment(marker.node_id.as_str())),
                )
            },
            |runtime| {
                declarative_stable_id(
                    &runtime.owner,
                    runtime.panel,
                    marker.document_id.as_str(),
                    marker.node_id.as_str(),
                )
            },
        );
        return (stable_id, "declarative_node");
    }
    if entity == audit_root {
        let root_name = world
            .nodes
            .get(entity)
            .ok()
            .and_then(|(_, name, _, _, _, _, _, _, _)| name)
            .map(|name| sanitize_segment(name.as_str()))
            .unwrap_or_else(|| "root".to_owned());
        return (format!("audit-root/{root_name}"), "named_hierarchy");
    }
    let role = world
        .nodes
        .get(entity)
        .ok()
        .map(|(_, name, _, _, _, _, _, _, _)| semantic_role(entity, name, world))
        .unwrap_or(RuntimeSemanticRole::Layout);
    let name = world
        .nodes
        .get(entity)
        .ok()
        .and_then(|(_, name, _, _, _, _, _, _, _)| name);
    let ordinal = sibling_ordinal(entity, parent, name, role, world);
    let segment = if let Some(name) = name {
        format!("name:{}[{ordinal}]", sanitize_segment(name.as_str()))
    } else {
        format!("{}[{ordinal}]", role.as_str())
    };
    (
        stable_hierarchy_id(parent_id.unwrap_or("audit-root/root"), &segment),
        if name.is_some() {
            "named_hierarchy"
        } else {
            "hierarchy_fallback"
        },
    )
}

fn sibling_ordinal(
    entity: Entity,
    parent: Option<Entity>,
    name: Option<&Name>,
    role: RuntimeSemanticRole,
    world: &UiAuditSemanticWorld,
) -> usize {
    let Some(parent) = parent else {
        return 0;
    };
    let Ok(children) = world.children.get(parent) else {
        return 0;
    };
    let mut ordinal = 0;
    for child in children.iter() {
        if child == entity {
            break;
        }
        let Ok((_, sibling_name, _, _, _, _, _, _, _)) = world.nodes.get(child) else {
            continue;
        };
        let matches = if let Some(name) = name {
            sibling_name.is_some_and(|sibling| sibling.as_str() == name.as_str())
        } else {
            sibling_name.is_none() && semantic_role(child, sibling_name, world) == role
        };
        if matches {
            ordinal += 1;
        }
    }
    ordinal
}

fn stable_hierarchy_id(parent_id: &str, segment: &str) -> String {
    format!("{parent_id}/{segment}")
}

fn declarative_stable_id(
    owner: &str,
    panel: UiDocumentPanel,
    document_id: &str,
    node_id: &str,
) -> String {
    format!(
        "document:{}:{}/{}/node:{}",
        sanitize_segment(owner),
        document_panel_name(panel),
        sanitize_segment(document_id),
        sanitize_segment(node_id),
    )
}

const fn document_panel_name(panel: UiDocumentPanel) -> &'static str {
    match panel {
        UiDocumentPanel::Page => "page",
        UiDocumentPanel::Hud => "hud",
        UiDocumentPanel::Floating => "floating",
        UiDocumentPanel::Modal => "modal",
        UiDocumentPanel::BlockingOverlay => "blocking_overlay",
    }
}

fn semantic_role(
    entity: Entity,
    name: Option<&Name>,
    world: &UiAuditSemanticWorld,
) -> RuntimeSemanticRole {
    if world.toast_roots.contains(entity) {
        return RuntimeSemanticRole::Toast;
    }
    if world.nonsemantic_measurements.contains(entity) {
        return RuntimeSemanticRole::Layout;
    }
    if let Ok((_, _, panel, _, _, _, _)) = world.panels.get(entity) {
        return match panel.kind {
            UiPanelKind::Floating => RuntimeSemanticRole::Floating,
            UiPanelKind::Modal => RuntimeSemanticRole::Modal,
            UiPanelKind::BlockingOverlay => RuntimeSemanticRole::Loading,
            UiPanelKind::Page | UiPanelKind::Hud => RuntimeSemanticRole::Layout,
        };
    }
    if world.scroll_views.contains(entity) {
        return RuntimeSemanticRole::Scroll;
    }
    if let Ok((meta, is_button, is_icon_button, _, is_text_input, _, _, _, _)) =
        world.controls.get(entity)
    {
        if is_icon_button || meta.is_some_and(|meta| meta.kind == UiControlKind::ImageButton) {
            return RuntimeSemanticRole::IconButton;
        }
        if is_text_input || meta.is_some_and(|meta| meta.kind == UiControlKind::TextInput) {
            return RuntimeSemanticRole::TextInput;
        }
        if is_button
            || meta
                .is_some_and(|meta| matches!(meta.kind, UiControlKind::Button | UiControlKind::Tab))
        {
            return RuntimeSemanticRole::Button;
        }
    }
    if world.texts.contains(entity) {
        let critical_name = name.is_some_and(|name| {
            let name = name.as_str().to_ascii_lowercase();
            name.contains("title") || name.contains("label")
        });
        let under_control = world.parents.iter_ancestors(entity).any(|ancestor| {
            world
                .controls
                .get(ancestor)
                .is_ok_and(|(meta, button, icon, _, input, _, _, _, _)| {
                    meta.is_some() || button || icon || input
                })
        });
        return if critical_name || under_control {
            RuntimeSemanticRole::CriticalText
        } else {
            RuntimeSemanticRole::Text
        };
    }
    if world.images.contains(entity) {
        return RuntimeSemanticRole::Image;
    }
    RuntimeSemanticRole::Layout
}

fn visible_label(
    entity: Entity,
    entity_visible: bool,
    own_text_nonempty: bool,
    audit_root: Entity,
    viewport: &UiViewport,
    world: &UiAuditSemanticWorld,
) -> bool {
    if !entity_visible {
        return false;
    }
    if world
        .controls
        .get(entity)
        .ok()
        .and_then(|(_, _, _, icon, _, _, _, _, _)| icon)
        .is_some_and(|icon| !icon.accessible_label.trim().is_empty())
    {
        return true;
    }
    let is_text_input = world
        .controls
        .get(entity)
        .is_ok_and(|(_, _, _, _, is_text_input, _, _, _, _)| is_text_input);
    if is_text_input {
        let value = world.text_input_values.get(entity).ok();
        let placeholder = world.text_input_placeholders.get(entity).ok();
        return value.zip(placeholder).is_some_and(|(value, placeholder)| {
            text_input_placeholder_is_visible_label(&value.0, &placeholder.0)
                && world.children.iter_descendants(entity).any(|child| {
                    text_entity_has_visible_area(child, false, audit_root, viewport, world)
                })
        });
    }
    own_text_nonempty && text_entity_has_visible_area(entity, true, audit_root, viewport, world)
        || world
            .children
            .iter_descendants(entity)
            .any(|child| text_entity_has_visible_area(child, true, audit_root, viewport, world))
}

fn text_entity_has_visible_area(
    entity: Entity,
    require_nonempty_text: bool,
    audit_root: Entity,
    viewport: &UiViewport,
    world: &UiAuditSemanticWorld,
) -> bool {
    if world.nonsemantic_measurements.contains(entity) {
        return false;
    }
    let Ok((text, layout)) = world.texts.get(entity) else {
        return false;
    };
    if require_nonempty_text && text.0.trim().is_empty() {
        return false;
    }
    let visible =
        world
            .nodes
            .get(entity)
            .ok()
            .is_some_and(|(_, _, _, _, _, inherited, visibility, _, _)| {
                inherited.is_none_or(|visibility| visibility.get())
                    && visibility.is_none_or(|visibility| *visibility != Visibility::Hidden)
            });
    if !visible {
        return false;
    }
    let Some(layout) = layout else {
        return false;
    };
    let Ok((_, _, computed, transform, _, _, _, _, _)) = world.nodes.get(entity) else {
        return false;
    };
    let visible_bounds = logical_measured_text_rect(computed, transform, layout)
        .intersect(effective_clip(entity, audit_root, viewport, world));
    visible_bounds.width() > 0.0 && visible_bounds.height() > 0.0
}

fn text_input_placeholder_is_visible_label(value: &str, placeholder: &str) -> bool {
    value.is_empty() && !placeholder.trim().is_empty()
}

fn effective_clip(
    entity: Entity,
    target_root: Entity,
    viewport: &UiViewport,
    world: &UiAuditSemanticWorld,
) -> UiAuditSemanticRect {
    let mut hierarchy = vec![entity];
    for ancestor in world.parents.iter_ancestors(entity) {
        hierarchy.push(ancestor);
        if ancestor == target_root {
            break;
        }
    }
    hierarchy.reverse();
    let mut clip = viewport_rect(viewport);
    for ancestor in hierarchy {
        let Ok((_, _, computed, transform, node, _, _, _, _)) = world.nodes.get(ancestor) else {
            continue;
        };
        let Some(node) = node else {
            continue;
        };
        let bounds = logical_rect(computed, transform);
        if node.overflow.x != OverflowAxis::Visible {
            clip.min_x = clip.min_x.max(bounds.min_x);
            clip.max_x = clip.max_x.min(bounds.max_x).max(clip.min_x);
        }
        if node.overflow.y != OverflowAxis::Visible {
            clip.min_y = clip.min_y.max(bounds.min_y);
            clip.max_y = clip.max_y.min(bounds.max_y).max(clip.min_y);
        }
    }
    UiAuditSemanticRect::from_extents(clip.min_x, clip.min_y, clip.max_x, clip.max_y).intersect(
        world
            .nodes
            .get(entity)
            .ok()
            .map(|(_, _, computed, transform, _, _, _, _, _)| logical_rect(computed, transform))
            .unwrap_or_default(),
    )
}

fn logical_rect(computed: &ComputedNode, transform: &UiGlobalTransform) -> UiAuditSemanticRect {
    let half = computed.size() * 0.5;
    let affine = transform.affine();
    let scale = computed.inverse_scale_factor();
    let points = [
        affine.transform_point2(Vec2::new(-half.x, -half.y)),
        affine.transform_point2(Vec2::new(half.x, -half.y)),
        affine.transform_point2(Vec2::new(-half.x, half.y)),
        affine.transform_point2(Vec2::new(half.x, half.y)),
    ];
    let min = points
        .iter()
        .copied()
        .reduce(Vec2::min)
        .unwrap_or(Vec2::ZERO)
        * scale;
    let max = points
        .iter()
        .copied()
        .reduce(Vec2::max)
        .unwrap_or(Vec2::ZERO)
        * scale;
    UiAuditSemanticRect::from_extents(min.x, min.y, max.x, max.y)
}

fn logical_measured_text_rect(
    computed: &ComputedNode,
    transform: &UiGlobalTransform,
    layout: &TextLayoutInfo,
) -> UiAuditSemanticRect {
    let scale = computed.inverse_scale_factor();
    let center = transform.affine().translation * scale;
    let size = layout.size * scale;
    UiAuditSemanticRect::from_extents(
        center.x - size.x * 0.5,
        center.y - size.y * 0.5,
        center.x + size.x * 0.5,
        center.y + size.y * 0.5,
    )
}

fn nearest_document_marker<'a>(
    entity: Entity,
    world: &'a UiAuditSemanticWorld,
) -> Option<&'a UiDocumentNodeAuditMarker> {
    world.document_nodes.get(entity).ok().or_else(|| {
        world
            .parents
            .iter_ancestors(entity)
            .find_map(|ancestor| world.document_nodes.get(ancestor).ok())
    })
}

fn nearest_document_runtime_root<'a>(
    entity: Entity,
    world: &'a UiAuditSemanticWorld,
) -> Option<&'a UiDocumentRuntimeRoot> {
    world.document_roots.get(entity).ok().or_else(|| {
        world
            .parents
            .iter_ancestors(entity)
            .find_map(|ancestor| world.document_roots.get(ancestor).ok())
    })
}

fn nearest_toast_root(entity: Entity, world: &UiAuditSemanticWorld) -> Option<Entity> {
    world
        .toast_roots
        .contains(entity)
        .then_some(entity)
        .or_else(|| {
            world
                .parents
                .iter_ancestors(entity)
                .find(|ancestor| world.toast_roots.contains(*ancestor))
        })
}

fn nearest_panel<'a>(
    entity: Entity,
    world: &'a UiAuditSemanticWorld,
) -> Option<(Entity, &'a UiPanelRoot)> {
    world
        .panels
        .get(entity)
        .ok()
        .map(|(entity, _, panel, _, _, _, _)| (entity, panel))
        .or_else(|| {
            world.parents.iter_ancestors(entity).find_map(|ancestor| {
                world
                    .panels
                    .get(ancestor)
                    .ok()
                    .map(|(entity, _, panel, _, _, _, _)| (entity, panel))
            })
        })
}

fn nearest_semantic_parent(
    entity: Entity,
    entities: &HashSet<Entity>,
    parents: &Query<&ChildOf>,
) -> Option<Entity> {
    parents
        .iter_ancestors(entity)
        .find(|ancestor| entities.contains(ancestor))
}

fn audit_roots(target_root: Entity, world: &UiAuditSemanticWorld) -> HashSet<Entity> {
    let mut roots = HashSet::from([target_root]);
    let target_owner = nearest_panel(target_root, world)
        .and_then(|(_, panel)| panel.owner)
        .or(world.current_owner.owner);
    for (entity, _, panel, inherited, visibility, _, _) in &world.panels {
        let is_overlay = matches!(
            panel.kind,
            UiPanelKind::Floating | UiPanelKind::Modal | UiPanelKind::BlockingOverlay
        );
        let visible = inherited.is_none_or(|visibility| visibility.get())
            && visibility.is_none_or(|visibility| *visibility != Visibility::Hidden);
        if is_overlay && visible && panel.owner == target_owner {
            roots.insert(entity);
        }
    }
    for entity in visible_audit_toast_roots(world) {
        roots.insert(entity);
    }
    roots
}

fn nearest_audit_root(
    entity: Entity,
    roots: &HashSet<Entity>,
    parents: &Query<&ChildOf>,
) -> Option<Entity> {
    roots.contains(&entity).then_some(entity).or_else(|| {
        parents
            .iter_ancestors(entity)
            .find(|ancestor| roots.contains(ancestor))
    })
}

fn toast_root_ordinal(entity: Entity, world: &UiAuditSemanticWorld) -> usize {
    visible_audit_toast_roots(world)
        .iter()
        .position(|candidate| *candidate == entity)
        .unwrap_or(0)
}

fn visible_audit_toast_roots(world: &UiAuditSemanticWorld) -> Vec<Entity> {
    let mut roots = world
        .toast_roots
        .iter()
        .filter(|(_, _, inherited, visibility, _, _)| {
            inherited.is_none_or(|visibility| visibility.get())
                && visibility.is_none_or(|visibility| *visibility != Visibility::Hidden)
        })
        .map(|(entity, _, _, _, z, _)| {
            let stack = world
                .nodes
                .get(entity)
                .ok()
                .map_or(0, |(_, _, computed, _, _, _, _, _, _)| {
                    computed.stack_index()
                });
            (entity, z.map_or(0, |z| z.0), stack)
        })
        .collect::<Vec<_>>();
    roots.sort_by_key(|(entity, z, stack)| (*z, *stack, *entity));
    roots.into_iter().map(|(entity, _, _)| entity).collect()
}

fn depth_from_root(entity: Entity, root: Entity, parents: &Query<&ChildOf>) -> u32 {
    if entity == root {
        return 0;
    }
    parents
        .iter_ancestors(entity)
        .take_while(|ancestor| *ancestor != root)
        .count()
        .saturating_add(1)
        .try_into()
        .unwrap_or(u32::MAX)
}

fn likely_files(
    document: Option<&UiDocumentNodeAuditMarker>,
    panel: Option<&UiPanelRoot>,
    is_toast: bool,
) -> Vec<String> {
    if let Some(document) = document {
        return vec![document.source_path.clone()];
    }
    if is_toast {
        return vec!["project/src/framework/ui/overlays/toast.rs".to_owned()];
    }
    match panel {
        Some(panel) => panel_likely_files(panel),
        None => vec!["project/src/framework/ui/".to_owned()],
    }
}

fn panel_likely_files(panel: &UiPanelRoot) -> Vec<String> {
    let path = match panel.id.as_str() {
        "login_page" | "character_select_page" => "project/src/game/screens/auth/login.rs",
        "game_list_page" => "project/src/game/screens/lobby/game_list.rs",
        "audio_settings_page" => "project/src/game/screens/settings/audio.rs",
        "audio_monitor_page" => "project/src/game/screens/dev/audio_monitor.rs",
        "audio_gallery_page" => "project/src/game/screens/dev/audio_gallery.rs",
        "ui_gallery_page" | "gallery_floating" => "project/src/game/screens/dev/ui_gallery.rs",
        "touch_ripple_hud" => "project/src/game/screens/gameplay/touch_ripple.rs",
        "sample_scene_hud" => "project/src/game/screens/gameplay/sample_scene.rs",
        "robot_sync_scene_hud" => "project/src/game/screens/gameplay/robot_sync_scene.rs",
        "fangyuan_home_hud" => "project/src/game/screens/gameplay/fangyuan_home.rs",
        "fangyuan_player_preview_hud" => {
            "project/src/game/screens/gameplay/fangyuan_player_preview.rs"
        }
        "confirm_modal" => "project/src/framework/ui/overlays/modal.rs",
        "global_loading" => "project/src/framework/ui/overlays/loading.rs",
        "tooltip" | "dropdown" => "project/src/framework/ui/overlays/popover.rs",
        "document_page"
        | "document_hud"
        | "document_floating"
        | "document_modal"
        | "document_blocking_overlay" => "project/src/framework/ui/document/runtime.rs",
        _ => match panel.kind {
            UiPanelKind::Page | UiPanelKind::Hud => "project/src/game/screens/mod.rs",
            UiPanelKind::Floating => "project/src/framework/ui/overlays/popover.rs",
            UiPanelKind::Modal => "project/src/framework/ui/overlays/modal.rs",
            UiPanelKind::BlockingOverlay => "project/src/framework/ui/overlays/loading.rs",
        },
    };
    vec![path.to_owned()]
}

fn viewport_rect(viewport: &UiViewport) -> UiAuditSemanticRect {
    UiAuditSemanticRect::from_extents(0.0, 0.0, viewport.logical_width, viewport.logical_height)
}

fn safe_area_rect(viewport: &UiViewport) -> UiAuditSemanticRect {
    UiAuditSemanticRect::from_extents(
        viewport.safe_area.left,
        viewport.safe_area.top,
        (viewport.logical_width - viewport.safe_area.right).max(viewport.safe_area.left),
        (viewport.logical_height - viewport.safe_area.bottom).max(viewport.safe_area.top),
    )
}

fn rounded(value: f32) -> f32 {
    if !value.is_finite() {
        return 0.0;
    }
    let rounded = (value * ROUNDING_SCALE).round() / ROUNDING_SCALE;
    if rounded == -0.0 { 0.0 } else { rounded }
}

fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn interaction_name(interaction: &Interaction) -> &'static str {
    match interaction {
        Interaction::None => "none",
        Interaction::Hovered => "hovered",
        Interaction::Pressed => "pressed",
    }
}

fn panel_has_focusable_descendants(entity: Entity, world: &UiAuditSemanticWorld) -> bool {
    world.focusable.iter().any(
        |(candidate, inherited, disabled_button, disabled_input, loading)| {
            !disabled_button
                && !disabled_input
                && !loading
                && inherited.is_none_or(|visibility| visibility.get())
                && (candidate == entity
                    || world
                        .parents
                        .iter_ancestors(candidate)
                        .any(|item| item == entity))
        },
    )
}

fn subtree_blocks_lower(entity: Entity, world: &UiAuditSemanticWorld) -> bool {
    world.nodes.iter().any(
        |(candidate, _, _, _, _, inherited, visibility, _, pickable)| {
            (candidate == entity
                || world
                    .parents
                    .iter_ancestors(candidate)
                    .any(|ancestor| ancestor == entity))
                && inherited.is_none_or(|visibility| visibility.get())
                && visibility.is_none_or(|visibility| *visibility != Visibility::Hidden)
                && pickable.is_none_or(|pickable| pickable.should_block_lower)
        },
    )
}

fn panel_layer_policy(panel: &UiPanelRoot) -> &'static str {
    match panel.kind {
        UiPanelKind::Page | UiPanelKind::Hud => "base",
        UiPanelKind::Floating if matches!(panel.id.as_str(), "dropdown" | "tooltip") => {
            "transient_above_modal"
        }
        UiPanelKind::Floating => "floating",
        UiPanelKind::Modal => "modal",
        UiPanelKind::BlockingOverlay => "blocking",
    }
}

const fn panel_kind(kind: UiPanelKind) -> &'static str {
    match kind {
        UiPanelKind::Page => "page",
        UiPanelKind::Hud => "hud",
        UiPanelKind::Floating => "floating",
        UiPanelKind::Modal => "modal",
        UiPanelKind::BlockingOverlay => "blocking_overlay",
    }
}

const fn panel_rank(kind: UiPanelKind) -> u8 {
    match kind {
        UiPanelKind::Page => 0,
        UiPanelKind::Hud => 1,
        UiPanelKind::Floating => 2,
        UiPanelKind::Modal => 3,
        UiPanelKind::BlockingOverlay => 4,
    }
}

fn sanitize_segment(value: &str) -> String {
    let mut output = String::new();
    let mut prior_separator = false;
    for character in value.chars().take(80) {
        let normalized = character.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() || matches!(normalized, '_' | '-' | '.') {
            output.push(normalized);
            prior_separator = false;
        } else if !prior_separator {
            output.push('_');
            prior_separator = true;
        }
    }
    let output = output.trim_matches('_');
    if output.is_empty() || matches!(output, "." | "..") {
        "unnamed".to_owned()
    } else {
        output.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::ui::core::{
        UiCurrentOwner, UiInputMode, UiOwnerId, UiPanelId, UiSafeArea,
    };
    use bevy::ecs::system::SystemState;
    use std::collections::BTreeSet;

    fn computed(width: f32, height: f32) -> ComputedNode {
        ComputedNode {
            size: Vec2::new(width, height),
            content_size: Vec2::new(width, height),
            inverse_scale_factor: 1.0,
            ..default()
        }
    }

    #[test]
    fn rects_are_half_open_rounded_and_intersect_deterministically() {
        let left = UiAuditSemanticRect::from_extents(0.0, 0.0, 10.007, 10.0);
        let right = UiAuditSemanticRect::from_extents(10.0, 0.0, 20.0, 10.0);
        assert_eq!(left.max_x, 10.0);
        assert_eq!(left.intersect(right).width(), 0.0);
        let viewport = UiAuditSemanticRect::from_extents(0.0, 0.0, 100.0, 100.0);
        let outside = UiAuditSemanticRect::from_extents(120.0, 10.0, 140.0, 30.0);
        assert_eq!(viewport.intersect(outside).width(), 0.0);
        let partial = UiAuditSemanticRect::from_extents(90.0, 10.0, 110.0, 30.0);
        assert_eq!(
            viewport.intersect(partial),
            UiAuditSemanticRect::from_extents(90.0, 10.0, 100.0, 30.0)
        );
    }

    #[test]
    fn stable_hierarchy_identity_never_depends_on_capture_entity() {
        let first_capture = Entity::from_raw_u32(1).unwrap();
        let second_capture = Entity::from_raw_u32(900).unwrap();
        assert_ne!(first_capture, second_capture);
        let first = stable_hierarchy_id("panel:login/root", "button[0]");
        let second = stable_hierarchy_id("panel:login/root", "button[0]");
        assert_eq!(first, second);
        assert!(!first.contains(&format!("{first_capture:?}")));
        assert!(!second.contains(&format!("{second_capture:?}")));
    }

    #[test]
    fn declarative_identity_disambiguates_instances_without_entity_or_instance_id() {
        let first_entity = Entity::from_raw_u32(4).unwrap();
        let reallocated_entity = Entity::from_raw_u32(404).unwrap();
        let first = declarative_stable_id(
            "owner_a",
            UiDocumentPanel::Page,
            "example.shared",
            "page.title",
        );
        let same_business_instance = declarative_stable_id(
            "owner_a",
            UiDocumentPanel::Page,
            "example.shared",
            "page.title",
        );
        let second_instance = declarative_stable_id(
            "owner_b",
            UiDocumentPanel::Modal,
            "example.shared",
            "page.title",
        );
        assert_eq!(first, same_business_instance);
        assert_ne!(first, second_instance);
        assert!(!first.contains(&format!("{first_entity:?}")));
        assert!(!same_business_instance.contains(&format!("{reallocated_entity:?}")));
    }

    #[test]
    fn stable_segments_normalize_names_without_path_escapes() {
        assert_eq!(sanitize_segment("Primary / Action"), "primary_action");
        assert_eq!(sanitize_segment("../"), "unnamed");
    }

    #[test]
    fn text_input_value_never_substitutes_for_an_independent_visible_label() {
        assert!(text_input_placeholder_is_visible_label("", "Account"));
        assert!(!text_input_placeholder_is_visible_label(
            "typed value",
            "Account"
        ));
        assert!(!text_input_placeholder_is_visible_label("typed value", ""));
        assert!(!text_input_placeholder_is_visible_label("", "  "));
    }

    #[test]
    fn collector_includes_same_owner_overlay_subtree_and_excludes_other_owner() {
        let mut world = World::new();
        let owner = UiOwnerId::new("owner_a");
        world.insert_resource(UiFocusState::default());
        world.insert_resource(UiInputState::default());
        world.insert_resource(UiCurrentOwner { owner: Some(owner) });

        let page = world
            .spawn((
                Name::new("PageA"),
                UiPanelRoot {
                    id: UiPanelId::new("page_a"),
                    kind: UiPanelKind::Page,
                    owner: Some(owner),
                },
                Node {
                    overflow: Overflow::clip(),
                    ..default()
                },
                InheritedVisibility::VISIBLE,
                computed(100.0, 100.0),
                UiGlobalTransform::from_xy(50.0, 50.0),
            ))
            .id();
        let partial = world
            .spawn((
                Name::new("partial"),
                Node::default(),
                InheritedVisibility::VISIBLE,
                computed(20.0, 20.0),
                UiGlobalTransform::from_xy(95.0, 50.0),
            ))
            .id();
        let outside = world
            .spawn((
                Name::new("outside"),
                Node::default(),
                InheritedVisibility::VISIBLE,
                computed(20.0, 20.0),
                UiGlobalTransform::from_xy(130.0, 50.0),
            ))
            .id();
        world.entity_mut(page).add_children(&[partial, outside]);

        let modal = world
            .spawn((
                Name::new("ModalSameOwner"),
                UiPanelRoot {
                    id: UiPanelId::new("modal_same_owner"),
                    kind: UiPanelKind::Modal,
                    owner: Some(owner),
                },
                Node::default(),
                InheritedVisibility::VISIBLE,
                computed(80.0, 80.0),
                UiGlobalTransform::from_xy(50.0, 50.0),
                ZIndex(100),
            ))
            .id();
        let modal_button = world
            .spawn((
                Button,
                FocusableButton,
                InheritedVisibility::VISIBLE,
                computed(48.0, 48.0),
                UiGlobalTransform::from_xy(50.0, 50.0),
            ))
            .id();
        world.entity_mut(modal).add_child(modal_button);

        let other_owner = UiOwnerId::new("owner_b");
        let other_modal = world
            .spawn((
                Name::new("ModalOtherOwner"),
                UiPanelRoot {
                    id: UiPanelId::new("modal_other_owner"),
                    kind: UiPanelKind::Modal,
                    owner: Some(other_owner),
                },
                Node::default(),
                InheritedVisibility::VISIBLE,
                computed(80.0, 80.0),
                UiGlobalTransform::from_xy(50.0, 50.0),
                ZIndex(100),
            ))
            .id();
        let other_button = world
            .spawn((
                Button,
                FocusableButton,
                InheritedVisibility::VISIBLE,
                computed(48.0, 48.0),
                UiGlobalTransform::from_xy(50.0, 50.0),
            ))
            .id();
        world.entity_mut(other_modal).add_child(other_button);

        let viewport = UiViewport::from_device_logical_size(
            100.0,
            100.0,
            UiInputMode::Touch,
            UiSafeArea::default(),
        );
        let mut state = SystemState::<UiAuditSemanticWorld>::new(&mut world);
        let semantic_world = state.get(&world);
        let tree = collect_semantic_tree(&semantic_world, Some(page), &viewport);

        let ids = tree
            .nodes
            .iter()
            .map(|node| node.stable_id.as_str())
            .collect::<BTreeSet<_>>();
        assert!(ids.contains("panel:modal_same_owner/root"), "ids={ids:?}");
        let modal_panel = tree
            .panels
            .iter()
            .find(|panel| panel.stable_id == "modal_same_owner")
            .unwrap();
        assert_eq!(modal_panel.capture_entity, format!("{modal:?}"));
        assert_ne!(modal_panel.capture_entity, "panel");
        assert_eq!(modal_panel.entity_name.as_deref(), Some("ModalSameOwner"));
        assert_eq!(
            modal_panel.likely_files,
            ["project/src/framework/ui/overlays/modal.rs"]
        );
        assert!(tree.nodes.iter().any(|node| {
            node.capture_entity == format!("{modal_button:?}")
                && node.parent_id.as_deref() == Some("panel:modal_same_owner/root")
                && node.stack_index == 0
        }));
        assert!(
            !tree
                .nodes
                .iter()
                .any(|node| node.capture_entity == format!("{other_modal:?}")
                    || node.capture_entity == format!("{other_button:?}"))
        );

        let partial = tree
            .nodes
            .iter()
            .find(|node| node.capture_entity == format!("{partial:?}"))
            .unwrap();
        assert_eq!(partial.entity_name.as_deref(), Some("partial"));
        assert_eq!(
            partial.clip_bounds,
            UiAuditSemanticRect::from_extents(85.0, 40.0, 100.0, 60.0)
        );
        assert!(!partial.fully_clipped);
        let outside = tree
            .nodes
            .iter()
            .find(|node| node.capture_entity == format!("{outside:?}"))
            .unwrap();
        assert!(outside.fully_clipped);
    }

    #[test]
    fn invisible_lower_toast_does_not_shift_visible_toast_identity_or_panel() {
        let mut world = World::new();
        let owner = UiOwnerId::new("owner_a");
        world.insert_resource(UiFocusState::default());
        world.insert_resource(UiInputState::default());
        world.insert_resource(UiCurrentOwner { owner: Some(owner) });

        let page = world
            .spawn((
                Name::new("PageA"),
                UiPanelRoot {
                    id: UiPanelId::new("page_a"),
                    kind: UiPanelKind::Page,
                    owner: Some(owner),
                },
                Node::default(),
                InheritedVisibility::VISIBLE,
                computed(100.0, 100.0),
                UiGlobalTransform::from_xy(50.0, 50.0),
            ))
            .id();
        let invisible_toast = world
            .spawn((
                Name::new("InvisibleToast"),
                UiToastRoot::for_audit_test(),
                Node::default(),
                InheritedVisibility::HIDDEN,
                computed(80.0, 20.0),
                UiGlobalTransform::from_xy(50.0, 20.0),
                ZIndex(100),
            ))
            .id();
        let visible_toast = world
            .spawn((
                Name::new("VisibleToast"),
                UiToastRoot::for_audit_test(),
                Node::default(),
                InheritedVisibility::VISIBLE,
                computed(80.0, 20.0),
                UiGlobalTransform::from_xy(50.0, 40.0),
                ZIndex(200),
            ))
            .id();
        let visible_child = world
            .spawn((
                Name::new("VisibleToastChild"),
                Node::default(),
                InheritedVisibility::VISIBLE,
                computed(40.0, 10.0),
                UiGlobalTransform::from_xy(50.0, 40.0),
            ))
            .id();
        world.entity_mut(visible_toast).add_child(visible_child);

        let viewport = UiViewport::from_device_logical_size(
            100.0,
            100.0,
            UiInputMode::Touch,
            UiSafeArea::default(),
        );
        let mut state = SystemState::<UiAuditSemanticWorld>::new(&mut world);
        let semantic_world = state.get(&world);
        let tree = collect_semantic_tree(&semantic_world, Some(page), &viewport);

        let toast_panels = tree
            .panels
            .iter()
            .filter(|panel| panel.kind == "toast")
            .collect::<Vec<_>>();
        assert_eq!(toast_panels.len(), 1);
        let panel = toast_panels[0];
        assert_eq!(panel.stable_id, "toast:0");
        assert_eq!(panel.capture_entity, format!("{visible_toast:?}"));
        assert_eq!(panel.entity_name.as_deref(), Some("VisibleToast"));
        assert_eq!(
            panel.likely_files,
            ["project/src/framework/ui/overlays/toast.rs"]
        );

        let root = tree
            .nodes
            .iter()
            .find(|node| node.capture_entity == format!("{visible_toast:?}"))
            .unwrap();
        assert_eq!(root.stable_id, "toast:0/root");
        assert_eq!(root.panel_id.as_deref(), Some("toast:0"));
        let child = tree
            .nodes
            .iter()
            .find(|node| node.capture_entity == format!("{visible_child:?}"))
            .unwrap();
        assert!(child.stable_id.starts_with("toast:0/root/"));
        assert_eq!(child.panel_id.as_deref(), Some("toast:0"));
        assert!(
            tree.nodes
                .iter()
                .all(|node| node.capture_entity != format!("{invisible_toast:?}"))
        );
        assert!(tree.panels.iter().all(|panel| panel.stable_id != "toast:1"));
    }

    #[test]
    fn visible_label_requires_positive_text_area_after_effective_clipping() {
        let mut world = World::new();
        let owner = UiOwnerId::new("owner_a");
        world.insert_resource(UiFocusState::default());
        world.insert_resource(UiInputState::default());
        world.insert_resource(UiCurrentOwner { owner: Some(owner) });

        let page = world
            .spawn((
                UiPanelRoot {
                    id: UiPanelId::new("page_a"),
                    kind: UiPanelKind::Page,
                    owner: Some(owner),
                },
                Node {
                    overflow: Overflow::clip(),
                    ..default()
                },
                InheritedVisibility::VISIBLE,
                computed(100.0, 100.0),
                UiGlobalTransform::from_xy(50.0, 50.0),
            ))
            .id();
        let clipped_button = world
            .spawn((
                Button,
                Node::default(),
                InheritedVisibility::VISIBLE,
                computed(40.0, 30.0),
                UiGlobalTransform::from_xy(30.0, 30.0),
            ))
            .id();
        let partial_button = world
            .spawn((
                Button,
                Node::default(),
                InheritedVisibility::VISIBLE,
                computed(40.0, 30.0),
                UiGlobalTransform::from_xy(70.0, 70.0),
            ))
            .id();
        let clipped_input = world
            .spawn((
                Button,
                UiTextInput,
                UiTextInputValue(String::new()),
                UiTextInputPlaceholder("Clipped placeholder".to_owned()),
                Node::default(),
                InheritedVisibility::VISIBLE,
                computed(40.0, 30.0),
                UiGlobalTransform::from_xy(30.0, 80.0),
            ))
            .id();
        let partial_input = world
            .spawn((
                Button,
                UiTextInput,
                UiTextInputValue(String::new()),
                UiTextInputPlaceholder("Partial placeholder".to_owned()),
                Node::default(),
                InheritedVisibility::VISIBLE,
                computed(40.0, 30.0),
                UiGlobalTransform::from_xy(70.0, 80.0),
            ))
            .id();
        let text_layout = || TextLayoutInfo {
            size: Vec2::new(20.0, 10.0),
            ..default()
        };
        let clipped_text = world
            .spawn((
                Text::new("Clipped label"),
                text_layout(),
                Node::default(),
                InheritedVisibility::VISIBLE,
                computed(20.0, 10.0),
                UiGlobalTransform::from_xy(150.0, 30.0),
            ))
            .id();
        let partial_text = world
            .spawn((
                Text::new("Partial label"),
                text_layout(),
                Node::default(),
                InheritedVisibility::VISIBLE,
                computed(20.0, 10.0),
                UiGlobalTransform::from_xy(95.0, 70.0),
            ))
            .id();
        let clipped_placeholder_text = world
            .spawn((
                Text::new(""),
                text_layout(),
                Node::default(),
                InheritedVisibility::VISIBLE,
                computed(20.0, 10.0),
                UiGlobalTransform::from_xy(150.0, 80.0),
            ))
            .id();
        let partial_placeholder_text = world
            .spawn((
                Text::new(""),
                text_layout(),
                Node::default(),
                InheritedVisibility::VISIBLE,
                computed(20.0, 10.0),
                UiGlobalTransform::from_xy(95.0, 80.0),
            ))
            .id();
        world.entity_mut(clipped_button).add_child(clipped_text);
        world.entity_mut(partial_button).add_child(partial_text);
        world
            .entity_mut(clipped_input)
            .add_child(clipped_placeholder_text);
        world
            .entity_mut(partial_input)
            .add_child(partial_placeholder_text);
        world.entity_mut(page).add_children(&[
            clipped_button,
            partial_button,
            clipped_input,
            partial_input,
        ]);

        let viewport = UiViewport::from_device_logical_size(
            100.0,
            100.0,
            UiInputMode::Touch,
            UiSafeArea::default(),
        );
        let mut state = SystemState::<UiAuditSemanticWorld>::new(&mut world);
        let semantic_world = state.get(&world);
        let tree = collect_semantic_tree(&semantic_world, Some(page), &viewport);

        let clipped = tree
            .nodes
            .iter()
            .find(|node| node.capture_entity == format!("{clipped_button:?}"))
            .unwrap();
        let partial = tree
            .nodes
            .iter()
            .find(|node| node.capture_entity == format!("{partial_button:?}"))
            .unwrap();
        let clipped_placeholder = tree
            .nodes
            .iter()
            .find(|node| node.capture_entity == format!("{clipped_input:?}"))
            .unwrap();
        let partial_placeholder = tree
            .nodes
            .iter()
            .find(|node| node.capture_entity == format!("{partial_input:?}"))
            .unwrap();
        assert!(!clipped.has_visible_label);
        assert!(partial.has_visible_label);
        assert!(!clipped_placeholder.has_visible_label);
        assert!(partial_placeholder.has_visible_label);
    }
}
