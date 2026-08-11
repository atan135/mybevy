use std::str::FromStr;

use bevy::prelude::*;

use crate::{
    framework::ui::{
        audit::{UiAuditCaptureState, UiAuditCaptureStateApplied},
        core::{
            UI_PANEL_DROPDOWN, UI_PANEL_TOOLTIP, UiAnimationCommand, UiAnimationDirection,
            UiAnimationEasing, UiAnimationId, UiAnimationRepeat, UiAnimationSpec, UiAnimations,
            UiPanelCommand, UiPanelRequest, UiPanelRoot, binding::UiBindingValues,
        },
        document::{
            UiActionDispatch, UiBindingDeclaration, UiBindingMissingBehavior, UiBindingPath,
            UiBindingScope, UiBindingType, UiBindingValue, UiDocumentNodeMarker,
        },
        i18n::UiI18n,
        overlays::UiDropdownPanel,
        style::{
            UI_EFFECT_PRESET_GALLERY_COMPOSITE, UI_EFFECT_PRESET_GALLERY_GRADIENT,
            UI_EFFECT_PRESET_GALLERY_MATERIAL_FALLBACK, UI_EFFECT_PRESET_GALLERY_SHADOW,
            UI_EFFECT_PRESET_GALLERY_TEXT_SHADOW, UiEffectBinding,
        },
        widgets::{
            UiControlMeta, UiControlOwner, UiDropdown, UiScrollAuditAnchorId, UiTooltipPinned,
        },
    },
    game::{
        navigation::UI_DOCUMENT_GALLERY_DOCUMENT,
        ui_ids::{
            ANCHOR_UI_GALLERY_ANIMATIONS, ANCHOR_UI_GALLERY_COMPONENT_CHECKBOXES,
            ANCHOR_UI_GALLERY_COMPONENT_DROPDOWN, ANCHOR_UI_GALLERY_COMPONENT_SEGMENTED,
            ANCHOR_UI_GALLERY_COMPONENT_TOGGLES, ANCHOR_UI_GALLERY_COMPONENT_TOOLTIP,
            ANCHOR_UI_GALLERY_COMPONENTS, ANCHOR_UI_GALLERY_EFFECTS, ANCHOR_UI_GALLERY_ICON_STATES,
            ANCHOR_UI_GALLERY_ICONS, ANCHOR_UI_GALLERY_IMAGE_ATLAS, ANCHOR_UI_GALLERY_IMAGE_MODES,
            ANCHOR_UI_GALLERY_IMAGE_TILING, ANCHOR_UI_GALLERY_INPUTS,
            ANCHOR_UI_GALLERY_STYLE_SCOPES, ANCHOR_UI_GALLERY_TYPOGRAPHY,
            ANCHOR_UI_GALLERY_TYPOGRAPHY_OVERFLOW, ANCHOR_UI_GALLERY_VISUAL_ACCEPTANCE,
            OWNER_UI_DOCUMENT_GALLERY, SCROLL_UI_GALLERY_MAIN,
        },
    },
};

#[cfg(test)]
const GALLERY_SOURCE: &str =
    include_str!("../../../../assets/ui/documents/approved/gallery/declarative_gallery.v1.json");

#[derive(Component)]
pub(super) struct DocumentGalleryAuditDropdown;

#[derive(Component)]
pub(super) struct DocumentGalleryAuditTooltip;

#[derive(Component)]
pub(super) struct DocumentGalleryAnimationSample;

const DOCUMENT_GALLERY_ANIMATION_AUDIT_PROGRESS: f32 = 0.625;
const DOCUMENT_GALLERY_STATUS_BINDING: &str = "gallery.status";
const DOCUMENT_GALLERY_STATUS_ACTION: &str = "gallery.set_status";

pub(super) fn tag_ui_document_gallery_audit_nodes(
    mut commands: Commands,
    nodes: Query<(Entity, &UiDocumentNodeMarker), Added<UiDocumentNodeMarker>>,
    i18n: Res<UiI18n>,
    mut binding_values: ResMut<UiBindingValues>,
) {
    for (entity, marker) in &nodes {
        match marker.node_id.as_str() {
            "gallery.root" => {
                commands.entity(entity).insert(SCROLL_UI_GALLERY_MAIN);
            }
            "gallery.pair.binding.status" => {
                set_document_gallery_status(
                    &mut binding_values,
                    i18n.tr(
                        "ui_gallery.binding.status.initial",
                        "Waiting for binding update.",
                    ),
                );
            }
            "gallery.section.visual_acceptance" => {
                insert_anchor(&mut commands, entity, ANCHOR_UI_GALLERY_VISUAL_ACCEPTANCE);
            }
            "gallery.section.image_modes" => {
                insert_anchor(&mut commands, entity, ANCHOR_UI_GALLERY_IMAGE_MODES);
            }
            "gallery.image_modes.tiling" => {
                insert_anchor(&mut commands, entity, ANCHOR_UI_GALLERY_IMAGE_TILING);
            }
            "gallery.image_modes.atlas" => {
                insert_anchor(&mut commands, entity, ANCHOR_UI_GALLERY_IMAGE_ATLAS);
            }
            "gallery.section.typography" => {
                insert_anchor(&mut commands, entity, ANCHOR_UI_GALLERY_TYPOGRAPHY);
            }
            "gallery.section.typography_overflow" => {
                insert_anchor(&mut commands, entity, ANCHOR_UI_GALLERY_TYPOGRAPHY_OVERFLOW);
            }
            "gallery.section.icons" => {
                insert_anchor(&mut commands, entity, ANCHOR_UI_GALLERY_ICONS);
            }
            "gallery.section.icon_states" => {
                insert_anchor(&mut commands, entity, ANCHOR_UI_GALLERY_ICON_STATES);
            }
            "gallery.section.style_scopes" => {
                insert_anchor(&mut commands, entity, ANCHOR_UI_GALLERY_STYLE_SCOPES);
            }
            "gallery.section.effects" => {
                insert_anchor(&mut commands, entity, ANCHOR_UI_GALLERY_EFFECTS);
            }
            "gallery.section.animations" => {
                insert_anchor(&mut commands, entity, ANCHOR_UI_GALLERY_ANIMATIONS);
            }
            "gallery.section.components" => {
                insert_anchor(&mut commands, entity, ANCHOR_UI_GALLERY_COMPONENTS);
            }
            "gallery.section.component_checkboxes" => {
                insert_anchor(
                    &mut commands,
                    entity,
                    ANCHOR_UI_GALLERY_COMPONENT_CHECKBOXES,
                );
            }
            "gallery.section.component_toggles" => {
                insert_anchor(&mut commands, entity, ANCHOR_UI_GALLERY_COMPONENT_TOGGLES);
            }
            "gallery.section.component_segmented" => {
                insert_anchor(&mut commands, entity, ANCHOR_UI_GALLERY_COMPONENT_SEGMENTED);
            }
            "gallery.section.component_dropdown" => {
                insert_anchor(&mut commands, entity, ANCHOR_UI_GALLERY_COMPONENT_DROPDOWN);
            }
            "gallery.section.component_tooltip" => {
                insert_anchor(&mut commands, entity, ANCHOR_UI_GALLERY_COMPONENT_TOOLTIP);
            }
            "gallery.section.inputs" => {
                insert_anchor(&mut commands, entity, ANCHOR_UI_GALLERY_INPUTS);
            }
            "gallery.select" => {
                commands.entity(entity).insert(DocumentGalleryAuditDropdown);
            }
            "gallery.tooltip" => {
                commands.entity(entity).insert(DocumentGalleryAuditTooltip);
            }
            "gallery.pair.effect.box_shadow" => {
                commands
                    .entity(entity)
                    .insert(UiEffectBinding::new(UI_EFFECT_PRESET_GALLERY_SHADOW));
            }
            "gallery.pair.effect.text_shadow" => {
                commands
                    .entity(entity)
                    .insert(UiEffectBinding::new(UI_EFFECT_PRESET_GALLERY_TEXT_SHADOW));
            }
            "gallery.pair.effect.gradient" => {
                commands
                    .entity(entity)
                    .insert(UiEffectBinding::new(UI_EFFECT_PRESET_GALLERY_GRADIENT));
            }
            "gallery.pair.effect.composite" => {
                commands
                    .entity(entity)
                    .insert(UiEffectBinding::new(UI_EFFECT_PRESET_GALLERY_COMPOSITE));
            }
            "gallery.pair.effect.material" => {
                commands.entity(entity).insert(UiEffectBinding::new(
                    UI_EFFECT_PRESET_GALLERY_MATERIAL_FALLBACK,
                ));
            }
            _ => {}
        }
        if let Some(animation) = document_gallery_animation(marker.node_id.as_str()) {
            commands.entity(entity).insert((
                UiTransform::default(),
                UiAnimations::try_from_spec(animation)
                    .expect("approved document Gallery animation must be valid"),
                DocumentGalleryAnimationSample,
            ));
        }
    }
}

pub(super) fn localize_ui_document_gallery_status_actions(
    mut actions: MessageReader<UiActionDispatch>,
    i18n: Res<UiI18n>,
    mut binding_values: ResMut<UiBindingValues>,
) {
    for action in actions.read() {
        if action.action.as_str() == DOCUMENT_GALLERY_STATUS_ACTION
            && action.document_id.as_str() == UI_DOCUMENT_GALLERY_DOCUMENT
            && action.owner == OWNER_UI_DOCUMENT_GALLERY.as_str()
        {
            set_document_gallery_status(
                &mut binding_values,
                i18n.tr("ui_gallery.binding.status.updated", "Bound text updated"),
            );
        }
    }
}

fn set_document_gallery_status(binding_values: &mut UiBindingValues, value: String) {
    let path = UiBindingPath::from_str(DOCUMENT_GALLERY_STATUS_BINDING)
        .expect("Gallery status binding path is static and valid");
    let declaration = UiBindingDeclaration {
        scope: UiBindingScope::Local,
        value_type: UiBindingType::String,
        default: Some(UiBindingValue::String(value.clone())),
        missing: UiBindingMissingBehavior::UseDefault,
    };
    binding_values.set_scoped(
        UI_DOCUMENT_GALLERY_DOCUMENT,
        OWNER_UI_DOCUMENT_GALLERY.as_str(),
        &path,
        &declaration,
        UiBindingValue::String(value),
    );
}

fn document_gallery_animation(node_id: &str) -> Option<UiAnimationSpec> {
    let animation = match node_id {
        "gallery.pair.animation.control" => UiAnimationSpec::transform_scale(
            UiAnimationId::new("gallery.control.press"),
            Vec2::splat(0.97),
            Vec2::ONE,
            0.52,
        )
        .with_easing(UiAnimationEasing::EaseInOutCubic),
        "gallery.pair.animation.page_entry" => UiAnimationSpec::transform_translation(
            UiAnimationId::new("gallery.page.entry_sample"),
            Vec2::new(0.0, 18.0),
            Vec2::ZERO,
            0.8,
        )
        .with_easing(UiAnimationEasing::EaseOutCubic),
        "gallery.pair.animation.dialog_entry" => UiAnimationSpec::transform_scale(
            UiAnimationId::new("gallery.modal.entry"),
            Vec2::splat(0.9),
            Vec2::ONE,
            0.72,
        )
        .with_easing(UiAnimationEasing::EaseOutCubic),
        "gallery.pair.animation.dialog_exit" => UiAnimationSpec::transform_scale(
            UiAnimationId::new("gallery.modal.exit"),
            Vec2::ONE,
            Vec2::splat(0.88),
            0.72,
        )
        .with_easing(UiAnimationEasing::EaseInCubic),
        "gallery.pair.animation.loading_loop" => UiAnimationSpec::transform_scale(
            UiAnimationId::new("gallery.loading.loop"),
            Vec2::splat(0.94),
            Vec2::ONE,
            0.64,
        )
        .with_easing(UiAnimationEasing::EaseInOutCubic),
        "gallery.pair.animation.layout_size" => UiAnimationSpec::layout_size(
            UiAnimationId::new("gallery.layout.size"),
            Vec2::new(96.0, 72.0),
            Vec2::new(120.0, 84.0),
            0.9,
        )
        .with_easing(UiAnimationEasing::EaseInOutCubic),
        "gallery.pair.animation.color_transition" => UiAnimationSpec::background_color(
            UiAnimationId::new("gallery.color.transition"),
            Color::srgb(0.08, 0.48, 0.43),
            Color::srgb(0.58, 0.23, 0.29),
            0.9,
        )
        .with_easing(UiAnimationEasing::EaseInOutCubic),
        "gallery.pair.animation.alpha_transition" => UiAnimationSpec::alpha(
            UiAnimationId::new("gallery.alpha.transition"),
            0.35,
            0.9,
            0.84,
        )
        .with_easing(UiAnimationEasing::EaseInOutCubic),
        _ => return None,
    };
    Some(
        animation
            .with_direction(UiAnimationDirection::Alternate)
            .with_repeat(UiAnimationRepeat::Infinite),
    )
}

fn insert_anchor(commands: &mut Commands, entity: Entity, anchor: UiScrollAuditAnchorId) {
    commands.entity(entity).insert(anchor);
}

pub(super) fn apply_ui_document_gallery_component_audit_state(
    mut commands: Commands,
    mut state_events: MessageReader<UiAuditCaptureStateApplied>,
    dropdowns: Query<
        (Entity, &UiDropdown, &UiControlMeta, Option<&UiControlOwner>),
        With<DocumentGalleryAuditDropdown>,
    >,
    tooltips: Query<(Entity, Has<UiTooltipPinned>), With<DocumentGalleryAuditTooltip>>,
    panel_roots: Query<&UiPanelRoot>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
) {
    for event in state_events.read() {
        panel_commands.write(UiPanelCommand::Close(UI_PANEL_TOOLTIP));

        if let Ok((entity, pinned)) = tooltips.single() {
            let should_pin = event.state == UiAuditCaptureState::ComponentTooltip;
            if should_pin != pinned {
                if should_pin {
                    commands.entity(entity).insert(UiTooltipPinned);
                } else {
                    commands.entity(entity).remove::<UiTooltipPinned>();
                }
            }
        }

        if event.state != UiAuditCaptureState::ComponentOverlays {
            panel_commands.write(UiPanelCommand::Close(UI_PANEL_DROPDOWN));
            continue;
        }
        let dropdown_is_open = panel_roots.iter().any(|panel| {
            panel.id == UI_PANEL_DROPDOWN && panel.owner == Some(OWNER_UI_DOCUMENT_GALLERY)
        });
        if dropdown_is_open {
            continue;
        }
        let Ok((entity, dropdown, meta, owner)) = dropdowns.single() else {
            continue;
        };
        panel_commands.write(UiPanelCommand::Open(UiPanelRequest::Dropdown(
            UiDropdownPanel {
                anchor: entity,
                meta: *meta,
                owner: owner
                    .map(|owner| owner.0)
                    .or(Some(OWNER_UI_DOCUMENT_GALLERY)),
                dropdown: dropdown.clone(),
            },
        )));
    }
}

pub(super) fn freeze_ui_document_gallery_animation_audit_state(
    mut state_events: MessageReader<UiAuditCaptureStateApplied>,
    samples: Query<Entity, With<DocumentGalleryAnimationSample>>,
    mut animation_commands: MessageWriter<UiAnimationCommand>,
) {
    if state_events.read().count() == 0 {
        return;
    }
    let mut entities = samples.iter().collect::<Vec<_>>();
    entities.sort_by_key(|entity| entity.to_bits());
    for entity in entities {
        animation_commands.write(UiAnimationCommand::Seek {
            entity,
            target: None,
            progress: DOCUMENT_GALLERY_ANIMATION_AUDIT_PROGRESS,
            pause: true,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use crate::framework::ui::{
        document::{UiComponentState, UiDocument, UiNode},
        widgets::{UiControlId, UiControlKind, UiDropdownOption, UiTooltip, UiTooltipTone},
    };

    const FIRST_BATCH_PAIR_IDS: [&str; 12] = [
        "gallery.pair.image_fit.contain",
        "gallery.pair.image_fit.stretch",
        "gallery.pair.image_fit.cover",
        "gallery.pair.image_mode.nine_slice.panel",
        "gallery.pair.image_mode.tile_both",
        "gallery.pair.image_mode.atlas_red",
        "gallery.pair.typography.regular",
        "gallery.pair.typography.long_word",
        "gallery.pair.icon.help",
        "gallery.pair.style.nested",
        "gallery.pair.effect.material",
        "gallery.pair.button.disabled",
    ];

    const AUDIT_SECTION_IDS: [&str; 18] = [
        "gallery.root",
        "gallery.section.visual_acceptance",
        "gallery.section.image_modes",
        "gallery.image_modes.tiling",
        "gallery.image_modes.atlas",
        "gallery.section.typography",
        "gallery.section.typography_overflow",
        "gallery.section.icons",
        "gallery.section.icon_states",
        "gallery.section.style_scopes",
        "gallery.section.effects",
        "gallery.section.animations",
        "gallery.section.components",
        "gallery.section.component_checkboxes",
        "gallery.section.component_toggles",
        "gallery.section.component_segmented",
        "gallery.section.component_dropdown",
        "gallery.section.inputs",
    ];

    const TOP_LEVEL_SECTION_IDS: [&str; 20] = [
        "gallery.section.visual_foundation",
        "gallery.section.visual_acceptance",
        "gallery.section.image_modes",
        "gallery.section.typography",
        "gallery.section.typography_overflow",
        "gallery.section.typography_boundary",
        "gallery.section.buttons",
        "gallery.section.icons",
        "gallery.section.icon_states",
        "gallery.section.style_scopes",
        "gallery.section.effects",
        "gallery.section.animations",
        "gallery.section.components",
        "gallery.section.selection",
        "gallery.section.numeric",
        "gallery.section.inputs",
        "gallery.section.binding",
        "gallery.section.overlays",
        "gallery.section.images",
        "gallery.section.stress",
    ];

    const SECTION_TITLES: [(&str, &str); 23] = [
        (
            "gallery.section.visual_foundation.title",
            "Visual Foundation",
        ),
        ("gallery.section.image_fit.title", "Image Fit"),
        (
            "gallery.section.visual_acceptance.title",
            "High Fidelity Acceptance",
        ),
        (
            "gallery.section.image_modes.title",
            "Nine-slice, Tiling, and Atlas Frames",
        ),
        ("gallery.section.typography.title", "Typography"),
        (
            "gallery.section.typography_overflow.title",
            "Mixed text and overflow states",
        ),
        (
            "gallery.section.typography_boundary.title",
            "Alignment and missing glyph",
        ),
        ("gallery.section.buttons.title", "Buttons"),
        ("gallery.section.icons.title", "Icon and Image Buttons"),
        ("gallery.section.icon_states.title", "Icon Button States"),
        ("gallery.section.style_scopes.title", "Scoped Styles"),
        ("gallery.section.effects.title", "Effects"),
        ("gallery.section.animations.title", "Animation and Motion"),
        ("gallery.section.components.title", "Component States"),
        (
            "gallery.components.selection_states",
            "Checkbox, Toggle, and Segmented States",
        ),
        ("gallery.section.selection.title", "Selection Controls"),
        ("gallery.section.numeric.title", "Numeric Controls"),
        ("gallery.section.inputs.title", "Inputs"),
        ("gallery.section.binding.title", "Binding Sample"),
        ("gallery.section.overlays.title", "Overlays"),
        ("gallery.section.images.title", "Images"),
        ("gallery.section.atlas_sources.title", "Atlas Source Images"),
        ("gallery.section.stress.title", "Stress Sample"),
    ];

    const SECTION_I18N_KEYS: [(&str, &str); 23] = [
        (
            "gallery.section.visual_foundation.title",
            "ui_gallery.visual_foundation.section",
        ),
        (
            "gallery.section.image_fit.title",
            "ui_gallery.image_fit.section",
        ),
        (
            "gallery.section.visual_acceptance.title",
            "ui_gallery.visual_acceptance.section",
        ),
        (
            "gallery.section.image_modes.title",
            "ui_gallery.image_modes.section",
        ),
        (
            "gallery.section.typography.title",
            "ui_gallery.typography.section",
        ),
        (
            "gallery.section.typography_overflow.title",
            "ui_gallery.typography.overflow",
        ),
        (
            "gallery.section.typography_boundary.title",
            "ui_gallery.typography.boundary",
        ),
        (
            "gallery.section.buttons.title",
            "ui_gallery.buttons.section",
        ),
        (
            "gallery.section.icons.title",
            "ui_gallery.icon_buttons.section",
        ),
        (
            "gallery.section.icon_states.title",
            "ui_gallery.icon_states.section",
        ),
        (
            "gallery.section.style_scopes.title",
            "ui_gallery.style_scopes.section",
        ),
        (
            "gallery.section.effects.title",
            "ui_gallery.effects.section",
        ),
        (
            "gallery.section.animations.title",
            "ui_gallery.animations.section",
        ),
        (
            "gallery.section.components.title",
            "ui_gallery.components.section",
        ),
        (
            "gallery.components.selection_states",
            "ui_gallery.components.selection_states",
        ),
        (
            "gallery.section.selection.title",
            "ui_gallery.selection.section",
        ),
        (
            "gallery.section.numeric.title",
            "ui_gallery.numeric.section",
        ),
        ("gallery.section.inputs.title", "ui_gallery.inputs.section"),
        (
            "gallery.section.binding.title",
            "ui_gallery.binding.section",
        ),
        (
            "gallery.section.overlays.title",
            "ui_gallery.overlays.section",
        ),
        ("gallery.section.images.title", "ui_gallery.images.section"),
        (
            "gallery.section.atlas_sources.title",
            "ui_gallery.images.atlas_sources",
        ),
        ("gallery.section.stress.title", "ui_gallery.stress.section"),
    ];

    #[test]
    fn declarative_gallery_first_batch_has_stable_pair_and_audit_ids() {
        serde_json::from_str::<UiDocument>(GALLERY_SOURCE)
            .expect("declarative Gallery must match the closed UiDocument structure");
        let validation = UiDocument::validate_json(GALLERY_SOURCE);
        assert!(
            validation.report.valid,
            "{:?}",
            validation.report.diagnostics
        );
        let document = validation.validated().unwrap().document();
        let mut ids = BTreeSet::new();
        collect_ids(&document.root, &mut ids);

        for expected in FIRST_BATCH_PAIR_IDS
            .into_iter()
            .chain(AUDIT_SECTION_IDS)
            .chain(["gallery.section.component_tooltip"])
        {
            assert!(
                ids.contains(expected),
                "missing Gallery parity node {expected}"
            );
        }
    }

    #[test]
    fn declarative_gallery_keeps_all_runtime_control_kinds() {
        serde_json::from_str::<UiDocument>(GALLERY_SOURCE)
            .expect("declarative Gallery must match the closed UiDocument structure");
        let validation = UiDocument::validate_json(GALLERY_SOURCE);
        assert!(
            validation.report.valid,
            "{:?}",
            validation.report.diagnostics
        );
        let document = validation.validated().unwrap().document();
        let mut kinds = BTreeSet::new();
        collect_kinds(&document.root, &mut kinds);

        for expected in [
            "button",
            "text_input",
            "checkbox",
            "toggle",
            "segmented",
            "slider",
            "stepper",
            "progress",
            "tab",
            "select",
            "badge",
            "tooltip",
            "image_button",
        ] {
            assert!(kinds.contains(expected), "missing control kind {expected}");
        }
    }

    #[test]
    fn declarative_gallery_matches_complete_content_contract() {
        let validation = UiDocument::validate_json(GALLERY_SOURCE);
        assert!(
            validation.report.valid,
            "{:?}",
            validation.report.diagnostics
        );
        assert!(validation.report.budget_usage.nodes > 400);
        assert!(
            validation.report.budget_usage.nodes
                <= crate::framework::ui::document::UI_DOCUMENT_MAX_NODES
        );

        let raw: serde_json::Value = serde_json::from_str(GALLERY_SOURCE).unwrap();
        let root_children = raw["root"]["children"][1]["component"]["children"]
            .as_array()
            .expect("Gallery scroll children must be declared");
        let top_level_ids = root_children
            .iter()
            .map(|node| node["id"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(top_level_ids, TOP_LEVEL_SECTION_IDS);

        let mut raw_nodes = BTreeMap::new();
        collect_raw_nodes(&raw["root"], &mut raw_nodes);
        for (id, columns, children) in [
            ("gallery.visual_foundation.fixtures", 4, 4),
            ("gallery.image_modes.nine_slice_grid", 3, 3),
            ("gallery.section.image_tiling", 3, 3),
            ("gallery.section.image_atlas", 4, 4),
            ("gallery.styles.grid", 4, 6),
            ("gallery.effects.grid", 3, 5),
            ("gallery.selection.grid", 3, 7),
            ("gallery.overlays.grid", 5, 7),
            ("gallery.images.grid", 2, 2),
            ("gallery.section.atlas_sources", 6, 7),
            ("gallery.stress.grid", 3, 24),
        ] {
            assert_eq!(raw_nodes[id]["layout"]["display"], "grid", "grid {id}");
            assert_eq!(
                raw_nodes[id]["layout"]["grid_columns"][0]["repeat"], columns,
                "columns {id}"
            );
            assert_eq!(
                raw_nodes[id]["children"].as_array().unwrap().len(),
                children,
                "children {id}"
            );
        }
        for suffix in [
            "transparent_edge",
            "non_square_2x1",
            "nine_slice_12px",
            "atlas_four_frames",
        ] {
            let card_id = format!("gallery.pair.visual_fixture.{suffix}.card");
            let frame_id = format!("gallery.pair.visual_fixture.{suffix}.frame");
            assert_eq!(
                raw_nodes[card_id.as_str()]["style"]["component"],
                "gallery_image_card"
            );
            assert_eq!(raw_nodes[card_id.as_str()]["layout"]["gap"]["px"], 3);
            assert_eq!(
                raw_nodes[card_id.as_str()]["layout"]["padding"]["all"]["px"],
                6
            );
            assert_eq!(raw_nodes[frame_id.as_str()]["layout"]["aspect_ratio"], 1.0);
        }
        assert_eq!(
            raw_nodes["gallery.pair.image_mode.tile_x.card"]["layout"]["gap"]["px"],
            3
        );
        assert_eq!(
            raw_nodes["gallery.pair.image_mode.tile_x.card"]["layout"]["padding"]["all"]["px"],
            6
        );
        assert_eq!(
            raw_nodes["gallery.pair.image_mode.tile_x.label"]["style"]["role"],
            "muted"
        );
        for section_id in TOP_LEVEL_SECTION_IDS
            .into_iter()
            .chain(["gallery.section.image_fit", "gallery.section.atlas_sources"])
        {
            let title_id = format!("{section_id}.title");
            assert_eq!(
                raw_nodes[title_id.as_str()]["style"]["text_role"],
                "section_label",
                "text role {title_id}"
            );
            assert_eq!(
                raw_nodes[title_id.as_str()]["style"]["role"],
                "muted",
                "color role {title_id}"
            );
        }
        for (id, expected) in SECTION_TITLES {
            assert_eq!(raw_literal(&raw_nodes, id), expected, "title {id}");
        }
        for (id, expected) in SECTION_I18N_KEYS {
            assert_eq!(
                raw_i18n_key(&raw_nodes, id, "content"),
                expected,
                "key {id}"
            );
        }

        let document = validation.validated().unwrap().document();
        let mut ids = BTreeSet::new();
        collect_ids(&document.root, &mut ids);
        for expected in complete_case_ids() {
            assert!(
                ids.contains(expected.as_str()),
                "missing Gallery case {expected}"
            );
        }

        let expected_paths = BTreeSet::from([
            "ui/fixtures/visual-foundation/transparent-edge.png",
            "ui/fixtures/visual-foundation/non-square-2x1.png",
            "ui/fixtures/visual-foundation/nine-slice-12px.png",
            "ui/fixtures/visual-foundation/atlas-four-frames.png",
            "ui/images/battlepass_bg_dragon01.png",
            "ui/images/battlepass_bg_dragon02.png",
            "ui/atlas/day_goal_tap.png",
            "ui/atlas/day_goal_tap2.png",
            "ui/atlas/puzzle_img1.png",
            "ui/atlas/puzzle_img_icon.png",
            "ui/atlas/puzzle_img_select.png",
            "ui/atlas/puzzle_img_select1.png",
            "ui/atlas/puzzle_img_time.png",
            "ui/icons/add.png",
            "ui/icons/remove.png",
            "ui/icons/help.png",
            "ui/icons/close.png",
            "ui/icons/loading.png",
            "ui/icons/arrow-left.png",
            "ui/icons/arrow-right.png",
            "ui/icons/full-color-badge.png",
            "ui/icons/missing.png",
        ]);
        let actual_paths = document
            .assets
            .values()
            .filter_map(|asset| match &asset.source {
                crate::framework::ui::document::UiAssetSource::Packaged { path } => {
                    Some(path.as_str())
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(actual_paths, expected_paths);

        let mut states = BTreeMap::new();
        collect_component_states(&document.root, &mut states);
        for family in ["checkboxes", "toggles", "segmented"] {
            for (suffix, state) in gallery_matrix_states() {
                let id = format!("gallery.pair.components.{family}.{suffix}");
                assert_eq!(states.get(id.as_str()), Some(&state), "state {id}");
            }
        }

        assert_eq!(super::super::ui_gallery::GALLERY_STRESS_ITEM_COUNT, 24);
        for index in 1..=super::super::ui_gallery::GALLERY_STRESS_ITEM_COUNT {
            let number = format!("{index:02}");
            let prefix = format!("gallery.pair.stress.item_{number}");
            assert_eq!(
                raw_literal(&raw_nodes, &format!("{prefix}.title")),
                format!("Item {number}")
            );
            assert_eq!(
                raw_i18n_key(&raw_nodes, &format!("{prefix}.title"), "content"),
                format!("ui_gallery.stress.item_{number}")
            );
            assert_eq!(
                raw_literal(&raw_nodes, &format!("{prefix}.state")),
                ["Ready", "Waiting", "Done"][(index - 1) % 3]
            );
            assert_eq!(
                raw_slot_literal(&raw_nodes, &format!("{prefix}.inspect"), "label"),
                "Inspect"
            );
        }
    }

    #[test]
    fn declarative_gallery_declares_all_effect_and_animation_adapters() {
        for id in [
            "gallery.pair.animation.control",
            "gallery.pair.animation.page_entry",
            "gallery.pair.animation.dialog_entry",
            "gallery.pair.animation.dialog_exit",
            "gallery.pair.animation.loading_loop",
            "gallery.pair.animation.layout_size",
            "gallery.pair.animation.color_transition",
            "gallery.pair.animation.alpha_transition",
        ] {
            assert!(document_gallery_animation(id).is_some(), "animation {id}");
        }
        assert!(document_gallery_animation("gallery.pair.animation.unknown").is_none());
    }

    #[test]
    fn component_overlay_audit_state_opens_document_dropdown() {
        let mut app = App::new();
        app.add_message::<UiAuditCaptureStateApplied>()
            .add_message::<UiPanelCommand>()
            .add_systems(Update, apply_ui_document_gallery_component_audit_state);
        let dropdown = app
            .world_mut()
            .spawn((
                UiDropdown::new("Choose", vec![UiDropdownOption::new("one", "One")], None),
                UiControlMeta::new(UiControlId::new("gallery.select"), UiControlKind::Dropdown),
                UiControlOwner(OWNER_UI_DOCUMENT_GALLERY),
                DocumentGalleryAuditDropdown,
            ))
            .id();
        app.world_mut().write_message(UiAuditCaptureStateApplied {
            state: UiAuditCaptureState::ComponentOverlays,
        });
        app.update();

        let messages = app.world().resource::<Messages<UiPanelCommand>>();
        let mut cursor = bevy::ecs::message::MessageCursor::default();
        let panel_commands = cursor.read(messages).collect::<Vec<_>>();
        assert!(panel_commands.iter().any(|command| matches!(
            command,
            UiPanelCommand::Open(UiPanelRequest::Dropdown(panel)) if panel.anchor == dropdown
        )));
    }

    #[test]
    fn component_overlay_audit_state_keeps_existing_document_dropdown_open() {
        let mut app = App::new();
        app.add_message::<UiAuditCaptureStateApplied>()
            .add_message::<UiPanelCommand>()
            .add_systems(Update, apply_ui_document_gallery_component_audit_state);
        app.world_mut().spawn((
            UiDropdown::new("Choose", vec![UiDropdownOption::new("one", "One")], None),
            UiControlMeta::new(UiControlId::new("gallery.select"), UiControlKind::Dropdown),
            UiControlOwner(OWNER_UI_DOCUMENT_GALLERY),
            DocumentGalleryAuditDropdown,
        ));
        app.world_mut().spawn(UiPanelRoot {
            id: UI_PANEL_DROPDOWN,
            kind: crate::framework::ui::core::UiPanelKind::Floating,
            owner: Some(OWNER_UI_DOCUMENT_GALLERY),
        });
        app.world_mut().write_message(UiAuditCaptureStateApplied {
            state: UiAuditCaptureState::ComponentOverlays,
        });
        app.update();

        let messages = app.world().resource::<Messages<UiPanelCommand>>();
        let mut cursor = bevy::ecs::message::MessageCursor::default();
        let panel_commands = cursor.read(messages).collect::<Vec<_>>();
        assert!(!panel_commands.iter().any(|command| matches!(
            command,
            UiPanelCommand::Open(UiPanelRequest::Dropdown(_))
                | UiPanelCommand::Close(UI_PANEL_DROPDOWN)
        )));
    }

    #[test]
    fn component_tooltip_audit_state_pins_document_tooltip() {
        let mut app = App::new();
        app.add_message::<UiAuditCaptureStateApplied>()
            .add_message::<UiPanelCommand>()
            .add_systems(Update, apply_ui_document_gallery_component_audit_state);
        let tooltip = app
            .world_mut()
            .spawn((
                UiTooltip {
                    text: "Stable tooltip".to_owned(),
                    tone: UiTooltipTone::Standard,
                },
                DocumentGalleryAuditTooltip,
            ))
            .id();
        app.world_mut().write_message(UiAuditCaptureStateApplied {
            state: UiAuditCaptureState::ComponentTooltip,
        });
        app.update();
        assert!(app.world().entity(tooltip).contains::<UiTooltipPinned>());

        app.world_mut().write_message(UiAuditCaptureStateApplied {
            state: UiAuditCaptureState::Middle,
        });
        app.update();
        assert!(!app.world().entity(tooltip).contains::<UiTooltipPinned>());
    }

    fn collect_ids<'a>(node: &'a UiNode, ids: &mut BTreeSet<&'a str>) {
        ids.insert(node.id().as_str());
        for child in node.children() {
            collect_ids(child, ids);
        }
    }

    fn collect_kinds(node: &UiNode, kinds: &mut BTreeSet<&'static str>) {
        kinds.insert(match node {
            UiNode::Container { .. } => "container",
            UiNode::Text { .. } => "text",
            UiNode::Image { .. } => "image",
            UiNode::Icon { .. } => "icon",
            UiNode::Spacer { .. } => "spacer",
            UiNode::Button { .. } => "button",
            UiNode::TextInput { .. } => "text_input",
            UiNode::Checkbox { .. } => "checkbox",
            UiNode::Toggle { .. } => "toggle",
            UiNode::Segmented { .. } => "segmented",
            UiNode::Slider { .. } => "slider",
            UiNode::Stepper { .. } => "stepper",
            UiNode::Scroll { .. } => "scroll",
            UiNode::Modal { .. } => "modal",
            UiNode::ImageButton { .. } => "image_button",
            UiNode::Badge { .. } => "badge",
            UiNode::Progress { .. } => "progress",
            UiNode::Tab { .. } => "tab",
            UiNode::Tooltip { .. } => "tooltip",
            UiNode::Select { .. } => "select",
        });
        for child in node.children() {
            collect_kinds(child, kinds);
        }
    }

    fn collect_raw_nodes<'a>(
        node: &'a serde_json::Value,
        nodes: &mut BTreeMap<&'a str, &'a serde_json::Value>,
    ) {
        if let Some(id) = node["id"].as_str() {
            nodes.insert(id, node);
        }
        for child in node["children"].as_array().into_iter().flatten().chain(
            node["component"]["children"]
                .as_array()
                .into_iter()
                .flatten(),
        ) {
            collect_raw_nodes(child, nodes);
        }
    }

    fn raw_literal(nodes: &BTreeMap<&str, &serde_json::Value>, id: &str) -> String {
        raw_content_text(&nodes[id]["content"])
            .unwrap_or_else(|| panic!("node {id} must have literal or fallback content"))
            .to_owned()
    }

    fn raw_slot_literal(
        nodes: &BTreeMap<&str, &serde_json::Value>,
        id: &str,
        slot: &str,
    ) -> String {
        raw_content_text(&nodes[id]["component"]["slots"][slot]["content"])
            .unwrap_or_else(|| panic!("node {id} must have literal or fallback {slot} content"))
            .to_owned()
    }

    fn raw_content_text(content: &serde_json::Value) -> Option<&str> {
        content["literal"]
            .as_str()
            .or_else(|| content["fallback"].as_str())
    }

    fn raw_i18n_key(nodes: &BTreeMap<&str, &serde_json::Value>, id: &str, field: &str) -> String {
        let content = if field == "content" {
            &nodes[id]["content"]
        } else {
            &nodes[id]["component"]["slots"][field]["content"]
        };
        content["i18n_key"]
            .as_str()
            .unwrap_or_else(|| panic!("node {id} must have i18n {field} content"))
            .to_owned()
    }

    fn collect_component_states<'a>(
        node: &'a UiNode,
        states: &mut BTreeMap<&'a str, UiComponentState>,
    ) {
        if let Some(component) = node.component() {
            states.insert(node.id().as_str(), component.states[0]);
        }
        for child in node.children() {
            collect_component_states(child, states);
        }
    }

    fn gallery_matrix_states() -> [(&'static str, UiComponentState); 8] {
        [
            ("normal", UiComponentState::Normal),
            ("hovered", UiComponentState::Hovered),
            ("pressed", UiComponentState::Pressed),
            ("focused", UiComponentState::Focused),
            ("selected", UiComponentState::Selected),
            ("disabled", UiComponentState::Disabled),
            ("loading", UiComponentState::Loading),
            ("error", UiComponentState::Error),
        ]
    }

    fn complete_case_ids() -> BTreeSet<String> {
        let mut ids = BTreeSet::new();
        for suffix in ["natural", "stretch", "contain", "cover"] {
            ids.insert(format!("gallery.pair.image_fit.{suffix}"));
        }
        for suffix in [
            "transparent_edge",
            "non_square_2x1",
            "nine_slice_12px",
            "atlas_four_frames",
        ] {
            ids.insert(format!("gallery.pair.visual_fixture.{suffix}"));
        }
        for suffix in [
            "background",
            "shadow_gradient",
            "nine_slice",
            "regular",
            "medium",
            "bold",
            "help",
            "selected",
            "loading",
            "disabled",
        ] {
            ids.insert(format!("gallery.pair.visual_acceptance.{suffix}"));
        }
        for suffix in [
            "nine_slice.panel",
            "nine_slice.small",
            "nine_slice.medium",
            "nine_slice.large",
            "tile_x",
            "tile_y",
            "tile_both",
            "atlas_red",
            "atlas_green",
            "atlas_blue",
            "atlas_yellow",
        ] {
            ids.insert(format!("gallery.pair.image_mode.{suffix}"));
        }
        for suffix in [
            "large_title",
            "section_title",
            "subtitle",
            "body",
            "caption",
            "button",
            "regular",
            "medium",
            "bold",
            "mixed",
            "long_word",
            "long_cjk",
            "clip",
            "ellipsis",
            "centered",
            "missing_glyph",
        ] {
            ids.insert(format!("gallery.pair.typography.{suffix}"));
        }
        for (prefix, suffixes) in [
            (
                "button",
                &[
                    "primary",
                    "secondary",
                    "focused",
                    "selected",
                    "loading",
                    "disabled",
                    "unavailable",
                    "action",
                ][..],
            ),
            (
                "icon",
                &[
                    "add",
                    "remove",
                    "help",
                    "close",
                    "loading",
                    "previous",
                    "next",
                    "tintable",
                    "full_color",
                    "missing",
                ][..],
            ),
            (
                "icon_state",
                &[
                    "idle", "hovered", "pressed", "focused", "selected", "disabled", "loading",
                ][..],
            ),
            (
                "style",
                &[
                    "global",
                    "parent",
                    "nested",
                    "restored",
                    "selected_button",
                    "selected_icon",
                ][..],
            ),
            (
                "effect",
                &[
                    "box_shadow",
                    "text_shadow",
                    "gradient",
                    "composite",
                    "material",
                ][..],
            ),
            (
                "animation",
                &[
                    "control",
                    "page_entry",
                    "dialog_entry",
                    "dialog_exit",
                    "loading_loop",
                    "layout_size",
                    "color_transition",
                    "alpha_transition",
                ][..],
            ),
        ] {
            ids.extend(
                suffixes
                    .iter()
                    .map(|suffix| format!("gallery.pair.{prefix}.{suffix}")),
            );
        }
        for (family, suffixes) in [
            (
                "components.badge",
                &[
                    "normal", "selected", "disabled", "loading", "empty", "error",
                ][..],
            ),
            (
                "components.progress",
                &["normal", "disabled", "loading", "empty", "error"][..],
            ),
            (
                "components.tab",
                &[
                    "normal", "hovered", "pressed", "focused", "selected", "disabled", "loading",
                ][..],
            ),
            (
                "components.dropdown",
                &[
                    "hovered", "pressed", "focused", "selected", "disabled", "loading", "empty",
                    "error",
                ][..],
            ),
            ("components.tooltip", &["error", "disabled"][..]),
            (
                "selection.checkbox",
                &["unchecked", "checked", "disabled"][..],
            ),
            ("selection.toggle", &["off", "on", "disabled"][..]),
            (
                "numeric",
                &["volume", "slider_disabled", "players", "stepper_disabled"][..],
            ),
            (
                "input",
                &[
                    "player_name",
                    "required",
                    "error",
                    "note",
                    "readonly",
                    "disabled",
                    "short_code",
                    "empty",
                ][..],
            ),
            (
                "binding",
                &["status", "notice", "bound_button", "update"][..],
            ),
            (
                "overlay",
                &[
                    "toast",
                    "loading",
                    "cancelable",
                    "hide",
                    "confirm",
                    "floating",
                    "close_top",
                ][..],
            ),
            ("images", &["dragon_01", "dragon_02"][..]),
            (
                "atlas_source",
                &[
                    "day_goal_tap",
                    "day_goal_tap2",
                    "puzzle_img1",
                    "puzzle_img_icon",
                    "puzzle_img_select",
                    "puzzle_img_select1",
                    "puzzle_img_time",
                ][..],
            ),
        ] {
            ids.extend(
                suffixes
                    .iter()
                    .map(|suffix| format!("gallery.pair.{family}.{suffix}")),
            );
        }
        ids.insert("gallery.select".to_owned());
        ids.insert("gallery.tooltip".to_owned());
        ids.insert("gallery.pair.selection.segmented".to_owned());
        for family in ["checkboxes", "toggles", "segmented"] {
            for (suffix, _) in gallery_matrix_states() {
                ids.insert(format!("gallery.pair.components.{family}.{suffix}"));
            }
        }
        for index in 1..=super::super::ui_gallery::GALLERY_STRESS_ITEM_COUNT {
            ids.insert(format!("gallery.pair.stress.item_{index:02}"));
        }
        ids
    }
}
