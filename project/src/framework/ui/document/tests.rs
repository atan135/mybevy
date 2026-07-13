use super::*;
use bevy::prelude::*;
use serde_json::{Value, json};
use std::{fs, path::PathBuf, str::FromStr};

const MINIMAL_DOCUMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/fixtures/minimal_page.v1.json"
));
const DUPLICATE_NODE_DOCUMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/fixtures/invalid/duplicate_node_id.v1.json"
));
const ILLEGAL_ID_DOCUMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/fixtures/invalid/illegal_id.v1.json"
));
const FUTURE_VERSION_DOCUMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/fixtures/invalid/future_version.v1.json"
));
const MISSING_ROOT_DOCUMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/fixtures/invalid/missing_root.v1.json"
));
const CANONICAL_DOCUMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/fixtures/minimal_page.v1.canonical.json"
));
const DOCUMENT_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/schema/ui_document.v1.schema.json"
));
const LAYOUT_DOCUMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/fixtures/layout_protocol.v1.json"
));
const LAYOUT_CANONICAL_DOCUMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/fixtures/layout_protocol.v1.canonical.json"
));
const STYLE_RESOURCE_DOCUMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/fixtures/style_resources.v1.json"
));
const STYLE_RESOURCE_CANONICAL_DOCUMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/fixtures/style_resources.v1.canonical.json"
));

#[test]
fn ui_document_parses_stage_one_fixture_and_indexes_nodes() {
    let validated = UiDocument::parse_and_validate_json(MINIMAL_DOCUMENT).unwrap();
    let document = validated.document();

    assert_eq!(document.schema_version, CURRENT_SCHEMA_VERSION);
    assert_eq!(document.document_id.as_str(), "example.minimal_page");
    assert_eq!(document.metadata.budget_profile, "mobile_baseline_v1");
    assert!(document.states.is_empty());
    assert_eq!(document.responsive.len(), 1);

    let hero_id = UiNodeId::from_str("page.hero").unwrap();
    assert_eq!(validated.node_path(&hero_id), Some("$.root.children[1]"));
    assert_eq!(validated.node_marker(&hero_id).unwrap().node_id, hero_id);
    assert_eq!(
        validated.audit_metadata(&hero_id).unwrap().document_path,
        "$.root.children[1]"
    );
}

#[test]
fn ui_document_rejects_duplicate_node_ids() {
    let error = UiDocument::parse_and_validate_json(DUPLICATE_NODE_DOCUMENT).unwrap_err();

    assert_eq!(error.code(), "UI_NODE_ID_DUPLICATE");
    assert!(matches!(
        error,
        UiDocumentError::DuplicateNodeId {
            first_path,
            duplicate_path,
            ..
        } if first_path == "$.root" && duplicate_path == "$.root.children[0]"
    ));
}

#[test]
fn ui_document_rejects_illegal_ids_during_parse() {
    let error = UiDocument::parse_and_validate_json(ILLEGAL_ID_DOCUMENT).unwrap_err();
    assert_eq!(error.code(), "UI_DOCUMENT_PARSE_FAILED");

    assert!(UiDocumentId::from_str("missing_namespace").is_err());
    assert!(UiNodeId::from_str("Page.root").is_err());
    assert!(UiAssetId::from_str("hero-image").is_err());
    assert!(UiStyleId::from_str("").is_err());
    assert!(UiActionId::from_str("example..continue").is_err());
}

#[test]
fn ui_document_rejects_unknown_versions() {
    let error = UiDocument::parse_and_validate_json(FUTURE_VERSION_DOCUMENT).unwrap_err();
    assert_eq!(error.code(), "UI_SCHEMA_FUTURE_VERSION");
    assert!(matches!(
        error,
        UiDocumentError::FutureSchemaVersion {
            found: 2,
            current: CURRENT_SCHEMA_VERSION
        }
    ));

    let invalid = UiDocument::parse_and_validate_json("{}").unwrap_err();
    assert_eq!(invalid.code(), "UI_SCHEMA_VERSION_INVALID");
}

#[test]
fn ui_document_rejects_missing_root_and_unknown_fields() {
    let missing_root = UiDocument::parse_and_validate_json(MISSING_ROOT_DOCUMENT).unwrap_err();
    assert_eq!(missing_root.code(), "UI_DOCUMENT_PARSE_FAILED");
    assert!(missing_root.to_string().contains("missing field `root`"));

    let source = MINIMAL_DOCUMENT.replacen(
        "\"schema_version\": 1,",
        "\"schema_version\": 1, \"unknown_capability\": true,",
        1,
    );
    let unknown = UiDocument::parse_and_validate_json(&source).unwrap_err();
    assert_eq!(unknown.code(), "UI_DOCUMENT_PARSE_FAILED");
    assert!(
        unknown
            .to_string()
            .contains("unknown field `unknown_capability`")
    );
}

#[test]
fn ui_document_canonical_json_matches_golden_and_round_trips() {
    let document = UiDocument::parse_and_validate_json(MINIMAL_DOCUMENT)
        .unwrap()
        .into_document();
    let canonical = document.to_canonical_json_pretty().unwrap();

    maybe_update_golden(
        "UPDATE_UI_DOCUMENT_GOLDENS",
        "assets/ui/documents/fixtures/minimal_page.v1.canonical.json",
        &canonical,
    );
    assert_eq!(canonical, CANONICAL_DOCUMENT);

    let reparsed = UiDocument::parse_and_validate_json(&canonical)
        .unwrap()
        .into_document();
    assert_eq!(reparsed, document);
    assert_eq!(
        document.to_canonical_json().unwrap(),
        reparsed.to_canonical_json().unwrap()
    );

    let canonical_value: Value = serde_json::from_str(&canonical).unwrap();
    assert_eq!(canonical_value["states"], json!([]));
    assert_eq!(
        canonical_value["root"]["children"][0]["layout"]["gap"],
        json!({ "px": 0.0 })
    );
    assert_eq!(
        canonical_value["root"]["children"][0]["style"]["role"],
        Value::Null
    );
}

#[test]
fn ui_document_schema_matches_rust_model() {
    let schema = generated_schema();
    maybe_update_golden(
        "UPDATE_UI_DOCUMENT_GOLDENS",
        "assets/ui/documents/schema/ui_document.v1.schema.json",
        &schema,
    );
    assert_eq!(schema, DOCUMENT_SCHEMA);

    let value: Value = serde_json::from_str(&schema).unwrap();
    assert_eq!(value["title"], "UiDocument");
    assert_eq!(value["additionalProperties"], false);
    assert!(
        value["required"]
            .as_array()
            .unwrap()
            .contains(&json!("root"))
    );
    assert_eq!(
        value["$defs"]["UiDocumentId"]["pattern"],
        UI_NAMESPACED_ID_PATTERN
    );
    assert_eq!(value["properties"]["schema_version"]["minimum"], 1);
    assert_eq!(value["properties"]["schema_version"]["maximum"], 1);
}

#[test]
fn ui_document_layout_lengths_map_to_bevy_vals() {
    let cases = [
        (UiLength::Auto, Val::Auto),
        (UiLength::Px(12.5), Val::Px(12.5)),
        (UiLength::Percent(25.0), Val::Percent(25.0)),
        (UiLength::Vw(30.0), Val::Vw(30.0)),
        (UiLength::Vh(40.0), Val::Vh(40.0)),
    ];
    for (length, expected) in cases {
        let layout = UiLayout {
            width: length,
            ..default()
        };
        assert_eq!(layout.to_bevy_layout().unwrap().node.width, expected);
    }
}

#[test]
fn ui_document_layout_flex_hidden_scroll_and_z_index_map_deterministically() {
    let layout = UiLayout {
        display: UiDisplay::Flex,
        width: UiLength::Percent(80.0),
        min_width: UiLength::Px(120.0),
        max_width: UiLength::Px(640.0),
        height: UiLength::Vh(50.0),
        aspect_ratio: Some(1.5),
        margin: UiInsets {
            all: UiLength::Px(2.0),
            left: Some(UiLength::Px(4.0)),
            ..default()
        },
        padding: UiInsets {
            all: UiLength::Percent(3.0),
            ..default()
        },
        border: UiInsets {
            all: UiLength::Px(1.0),
            ..default()
        },
        gap: UiLength::Px(8.0),
        row_gap: Some(UiLength::Vh(2.0)),
        align_items: UiAlignItems::Center,
        justify_items: UiAlignItems::End,
        align_self: UiAlignSelf::Start,
        justify_self: UiAlignSelf::Center,
        align_content: UiContentAlignment::SpaceAround,
        justify_content: UiContentAlignment::SpaceBetween,
        direction: UiFlexDirection::RowReverse,
        wrap: UiFlexWrap::Wrap,
        flex_grow: 2.0,
        flex_shrink: 0.5,
        flex_basis: UiLength::Px(24.0),
        overflow: UiOverflow {
            x: UiOverflowAxis::Clip,
            y: UiOverflowAxis::Scroll,
        },
        scrollbar_width: 10.0,
        z_index: 12,
        ..default()
    };
    let resolved = layout.to_bevy_layout().unwrap();
    let node = resolved.node;
    assert_eq!(node.display, Display::Flex);
    assert_eq!(node.flex_direction, FlexDirection::RowReverse);
    assert_eq!(node.flex_wrap, FlexWrap::Wrap);
    assert_eq!(node.width, Val::Percent(80.0));
    assert_eq!(node.min_width, Val::Px(120.0));
    assert_eq!(node.max_width, Val::Px(640.0));
    assert_eq!(node.height, Val::Vh(50.0));
    assert_eq!(node.aspect_ratio, Some(1.5));
    assert_eq!(node.margin.left, Val::Px(4.0));
    assert_eq!(node.margin.right, Val::Px(2.0));
    assert_eq!(node.padding.top, Val::Percent(3.0));
    assert_eq!(node.border.bottom, Val::Px(1.0));
    assert_eq!(node.row_gap, Val::Vh(2.0));
    assert_eq!(node.column_gap, Val::Px(8.0));
    assert_eq!(node.align_items, AlignItems::Center);
    assert_eq!(node.justify_content, JustifyContent::SpaceBetween);
    assert_eq!(node.overflow.x, OverflowAxis::Clip);
    assert_eq!(node.overflow.y, OverflowAxis::Scroll);
    assert_eq!(resolved.z_index, Some(ZIndex(12)));

    let hidden = UiLayout {
        display: UiDisplay::None,
        ..default()
    }
    .to_bevy_layout()
    .unwrap();
    assert_eq!(hidden.node.display, Display::None);
    assert_eq!(hidden.z_index, None);
}

#[test]
fn ui_document_layout_grid_tracks_repeat_and_span_map_to_bevy() {
    let layout = UiLayout {
        display: UiDisplay::Grid,
        grid_columns: vec![
            UiGridTrack {
                repeat: 3,
                size: UiGridTrackSize::Fr(1.0),
            },
            UiGridTrack {
                repeat: 1,
                size: UiGridTrackSize::Percent(25.0),
            },
        ],
        grid_rows: vec![UiGridTrack {
            repeat: 2,
            size: UiGridTrackSize::Vh(10.0),
        }],
        grid_auto_columns: vec![UiGridTrackSize::Auto, UiGridTrackSize::Px(40.0)],
        grid_auto_rows: vec![UiGridTrackSize::Vw(5.0)],
        grid_auto_flow: UiGridAutoFlow::ColumnDense,
        grid_column: UiGridPlacement {
            start: Some(2),
            span: 3,
        },
        grid_row: UiGridPlacement {
            start: None,
            span: 2,
        },
        ..default()
    };
    let node = layout.to_bevy_layout().unwrap().node;
    assert_eq!(node.display, Display::Grid);
    assert_eq!(
        node.grid_template_columns,
        vec![
            RepeatedGridTrack::flex(3, 1.0),
            RepeatedGridTrack::percent(1, 25.0),
        ]
    );
    assert_eq!(
        node.grid_template_rows,
        vec![RepeatedGridTrack::vh(2, 10.0)]
    );
    assert_eq!(
        node.grid_auto_columns,
        vec![GridTrack::auto(), GridTrack::px(40.0)]
    );
    assert_eq!(node.grid_auto_rows, vec![GridTrack::vw(5.0)]);
    assert_eq!(node.grid_auto_flow, GridAutoFlow::ColumnDense);
    assert_eq!(node.grid_column, GridPlacement::start_span(2, 3));
    assert_eq!(node.grid_row, GridPlacement::span(2));
}

#[test]
fn ui_document_layout_absolute_uses_parent_border_box_and_requires_solvable_axes() {
    let valid = UiLayout {
        position: UiLayoutPosition::Absolute(UiAbsolutePosition {
            containing_block: UiAbsoluteContainingBlock::ParentBorderBox,
            left: Some(UiLength::Px(8.0)),
            top: Some(UiLength::Percent(10.0)),
            ..default()
        }),
        width: UiLength::Px(100.0),
        height: UiLength::Vh(20.0),
        ..default()
    };
    let node = valid.to_bevy_layout().unwrap().node;
    assert_eq!(node.position_type, PositionType::Absolute);
    assert_eq!(node.left, Val::Px(8.0));
    assert_eq!(node.top, Val::Percent(10.0));

    let inferred = UiLayout {
        position: UiLayoutPosition::Absolute(UiAbsolutePosition {
            left: Some(UiLength::Px(8.0)),
            right: Some(UiLength::Px(8.0)),
            top: Some(UiLength::Px(4.0)),
            bottom: Some(UiLength::Px(4.0)),
            ..default()
        }),
        ..default()
    };
    assert!(inferred.to_bevy_layout().is_ok());

    let underconstrained = UiLayout {
        position: UiLayoutPosition::Absolute(UiAbsolutePosition {
            left: Some(UiLength::Px(8.0)),
            ..default()
        }),
        ..default()
    };
    assert_layout_error(
        &underconstrained,
        "UI_LAYOUT_ABSOLUTE_AXIS_UNDERCONSTRAINED",
        "position.absolute.horizontal",
    );

    let overconstrained = UiLayout {
        position: UiLayoutPosition::Absolute(UiAbsolutePosition {
            left: Some(UiLength::Px(0.0)),
            right: Some(UiLength::Px(0.0)),
            top: Some(UiLength::Px(0.0)),
            ..default()
        }),
        width: UiLength::Px(100.0),
        height: UiLength::Px(40.0),
        ..default()
    };
    assert_layout_error(
        &overconstrained,
        "UI_LAYOUT_ABSOLUTE_AXIS_OVERCONSTRAINED",
        "position.absolute.horizontal",
    );
}

#[test]
fn ui_document_layout_rejects_invalid_values_with_stable_field_errors() {
    let mut layout = UiLayout {
        width: UiLength::Px(-1.0),
        height: UiLength::Percent(101.0),
        min_width: UiLength::Px(300.0),
        max_width: UiLength::Px(200.0),
        aspect_ratio: Some(f32::NAN),
        flex_grow: f32::INFINITY,
        z_index: UI_LAYOUT_MAX_Z_INDEX + 1,
        ..default()
    };
    let errors = layout.to_bevy_layout().unwrap_err();
    for (code, path) in [
        ("UI_LAYOUT_LENGTH_NEGATIVE", "width"),
        ("UI_LAYOUT_PERCENT_OUT_OF_RANGE", "height"),
        ("UI_LAYOUT_CONSTRAINT_CONTRADICTION", "width.min_max"),
        ("UI_LAYOUT_VALUE_NON_FINITE", "aspect_ratio"),
        ("UI_LAYOUT_VALUE_NON_FINITE", "flex_grow"),
        ("UI_LAYOUT_Z_INDEX_OUT_OF_RANGE", "z_index"),
    ] {
        assert!(
            errors
                .iter()
                .any(|error| error.code == code && error.path == path)
        );
    }

    layout = UiLayout {
        display: UiDisplay::Grid,
        grid_columns: vec![UiGridTrack {
            repeat: UI_GRID_MAX_REPEAT + 1,
            size: UiGridTrackSize::Fr(1.0),
        }],
        grid_row: UiGridPlacement {
            start: Some(0),
            span: UI_GRID_MAX_SPAN + 1,
        },
        ..default()
    };
    assert_layout_error(
        &layout,
        "UI_LAYOUT_GRID_REPEAT_INVALID",
        "grid_columns[0].repeat",
    );
    assert_layout_error(
        &layout,
        "UI_LAYOUT_GRID_PLACEMENT_INVALID",
        "grid_row.start",
    );
    assert_layout_error(&layout, "UI_LAYOUT_GRID_SPAN_INVALID", "grid_row.span");

    layout.grid_columns = (0..=UI_GRID_MAX_TRACK_DEFINITIONS)
        .map(|_| UiGridTrack {
            repeat: 1,
            size: UiGridTrackSize::Auto,
        })
        .collect();
    assert_layout_error(&layout, "UI_LAYOUT_GRID_TRACK_LIMIT", "grid_columns");
}

#[test]
fn ui_document_layout_errors_include_document_field_path() {
    let mut document = UiDocument::parse_and_validate_json(MINIMAL_DOCUMENT)
        .unwrap()
        .into_document();
    if let UiNode::Container { layout, .. } = &mut document.root {
        layout.width = UiLength::Px(-10.0);
    }
    let error = ValidatedUiDocument::new(document).unwrap_err();
    assert_eq!(error.code(), "UI_LAYOUT_INVALID");
    assert!(matches!(
        error,
        UiDocumentError::InvalidLayout { errors }
            if errors.iter().any(|error| error.code == "UI_LAYOUT_LENGTH_NEGATIVE"
                && error.path == "$.root.layout.width")
    ));

    let mut document = UiDocument::parse_and_validate_json(MINIMAL_DOCUMENT)
        .unwrap()
        .into_document();
    document.responsive[0].overrides[0]
        .set
        .layout
        .as_mut()
        .unwrap()
        .gap = Some(UiLength::Px(-1.0));
    let error = ValidatedUiDocument::new(document).unwrap_err();
    assert!(matches!(
        error,
        UiDocumentError::InvalidLayout { errors }
            if errors.iter().any(|error| error.code == "UI_LAYOUT_LENGTH_NEGATIVE"
                && error.path == "$.responsive[0].overrides[0].set.layout.gap")
    ));

    let mut document = UiDocument::parse_and_validate_json(MINIMAL_DOCUMENT)
        .unwrap()
        .into_document();
    let patch = document.responsive[0].overrides[0]
        .set
        .layout
        .as_mut()
        .unwrap();
    patch.min_width = Some(UiLength::Px(300.0));
    patch.max_width = Some(UiLength::Px(200.0));
    patch.display = Some(UiDisplay::Flex);
    patch.grid_columns = Some(vec![UiGridTrack {
        repeat: 1,
        size: UiGridTrackSize::Fr(1.0),
    }]);
    let error = ValidatedUiDocument::new(document).unwrap_err();
    let UiDocumentError::InvalidLayout { errors } = error else {
        panic!("expected layout errors");
    };
    assert!(errors.iter().any(|error| {
        error.code == "UI_LAYOUT_CONSTRAINT_CONTRADICTION"
            && error.path == "$.responsive[0].overrides[0].set.layout.width.min_max"
    }));
    assert!(errors.iter().any(|error| {
        error.code == "UI_LAYOUT_FIELD_NOT_APPLICABLE"
            && error.path == "$.responsive[0].overrides[0].set.layout.display"
    }));

    let mut document = UiDocument::parse_and_validate_json(MINIMAL_DOCUMENT)
        .unwrap()
        .into_document();
    document.states.push(UiStateDefinition {
        id: "invalid_constraints".to_owned(),
        overrides: vec![UiNodeOverride {
            node_id: UiNodeId::from_str("page.hero").unwrap(),
            set: UiNodePatch {
                layout: Some(UiLayoutPatch {
                    min_height: Some(UiLength::Vh(80.0)),
                    height: Some(UiLength::Vh(50.0)),
                    ..default()
                }),
                ..default()
            },
        }],
    });
    let error = ValidatedUiDocument::new(document).unwrap_err();
    assert!(matches!(
        error,
        UiDocumentError::InvalidLayout { errors }
            if errors.iter().any(|error| {
                error.code == "UI_LAYOUT_CONSTRAINT_CONTRADICTION"
                    && error.path == "$.states[0].overrides[0].set.layout.height.min_value"
            })
    ));
}

#[test]
fn ui_document_layout_json_matches_golden_and_maps_fixture() {
    let document = UiDocument::parse_and_validate_json(LAYOUT_DOCUMENT)
        .unwrap()
        .into_document();
    let canonical = document.to_canonical_json_pretty().unwrap();
    maybe_update_golden(
        "UPDATE_UI_DOCUMENT_GOLDENS",
        "assets/ui/documents/fixtures/layout_protocol.v1.canonical.json",
        &canonical,
    );
    assert_eq!(canonical, LAYOUT_CANONICAL_DOCUMENT);

    let root = document.root.layout().to_bevy_layout().unwrap();
    assert_eq!(root.node.display, Display::Grid);
    assert_eq!(root.node.width, Val::Vw(100.0));
    assert_eq!(root.node.overflow.y, OverflowAxis::Scroll);
    assert_eq!(root.z_index, Some(ZIndex(7)));

    let mut negative_zero = document;
    if let UiNode::Container { layout, .. } = &mut negative_zero.root {
        layout.flex_grow = -0.0;
    }
    assert!(!negative_zero.to_canonical_json().unwrap().contains("-0.0"));
}

#[test]
fn ui_document_style_merge_color_canonical_and_image_modes_match_golden() {
    let document = UiDocument::parse_and_validate_json(STYLE_RESOURCE_DOCUMENT)
        .unwrap()
        .into_document();
    let canonical = document.to_canonical_json_pretty().unwrap();
    maybe_update_golden(
        "UPDATE_UI_DOCUMENT_GOLDENS",
        "assets/ui/documents/fixtures/style_resources.v1.canonical.json",
        &canonical,
    );
    assert_eq!(canonical, STYLE_RESOURCE_CANONICAL_DOCUMENT);
    assert!(canonical.contains("#ff800080"));
    assert!(!canonical.contains("\"srgb\""));

    let resolved = document
        .resolve_style(document.root.style(), "$.root.style")
        .unwrap();
    assert_eq!(
        resolved.properties.background,
        Some(UiResolvedBackground::Solid(UiColor::from_rgba8(
            255, 128, 0, 128
        )))
    );
    assert_eq!(resolved.properties.border.unwrap().width, 2.0);
    assert_eq!(resolved.properties.corner_radius, Some([12.0; 4]));
    assert_eq!(resolved.properties.opacity, Some(0.8));
    assert!(matches!(
        resolved.properties.material.unwrap().parameters,
        UiResolvedMaterialParameters::FrostedPanelV1 { .. }
    ));
    let component_only = document
        .resolve_style(
            &UiStyle {
                component: Some(style_id("panel_frosted")),
                ..default()
            },
            "$.test.component_only",
        )
        .unwrap();
    assert_eq!(
        component_only.properties.background,
        Some(UiResolvedBackground::Solid(UiColor::from_rgba8(
            32, 64, 96, 255
        )))
    );

    let mut merge_document = document.clone();
    merge_document.styles.insert(
        style_id("text_override"),
        UiStyleDefinition {
            extends: Some(style_id("panel_base")),
            properties: UiStyleProperties {
                text: Some(UiTextVisualStyle {
                    color: Some(UiColorValue::Literal {
                        value: UiColor::from_rgba8(12, 34, 56, 255),
                    }),
                    ..default()
                }),
                ..default()
            },
        },
    );
    let inherited_text = merge_document
        .resolve_style(
            &UiStyle {
                component: Some(style_id("text_override")),
                ..default()
            },
            "$.test.inherited_text",
        )
        .unwrap()
        .properties
        .text
        .unwrap();
    assert_eq!(
        inherited_text.color,
        Some(UiColor::from_rgba8(12, 34, 56, 255))
    );
    assert_eq!(inherited_text.font.unwrap().as_str(), "body_font");
    assert_eq!(inherited_text.font_size, Some(18.0));
    assert_eq!(inherited_text.line_height, Some(1.3));
    assert_eq!(inherited_text.weight, Some(UiTextWeight::Medium));

    let inline_text = merge_document
        .resolve_style(
            &UiStyle {
                component: Some(style_id("panel_base")),
                inline: UiStyleProperties {
                    text: Some(UiTextVisualStyle {
                        color: Some(UiColorValue::Literal {
                            value: UiColor::from_rgba8(70, 80, 90, 255),
                        }),
                        ..default()
                    }),
                    ..default()
                },
                ..default()
            },
            "$.test.inline_text",
        )
        .unwrap()
        .properties
        .text
        .unwrap();
    assert_eq!(
        inline_text.color,
        Some(UiColor::from_rgba8(70, 80, 90, 255))
    );
    assert_eq!(inline_text.font.unwrap().as_str(), "body_font");
    assert_eq!(inline_text.font_size, Some(18.0));
    assert_eq!(inline_text.line_height, Some(1.3));
    assert_eq!(inline_text.weight, Some(UiTextWeight::Medium));

    let children = document.root.children();
    let UiNode::Image { presentation, .. } = &children[0] else {
        panic!("cover fixture must be an image")
    };
    assert!(matches!(
        presentation.to_widget_fit(),
        Some(crate::framework::ui::widgets::UiImageFit::Cover { .. })
    ));
    let UiNode::Image { presentation, .. } = &children[1] else {
        panic!("slice fixture must be an image")
    };
    assert!(matches!(
        presentation.to_widget_advanced_mode(),
        Some(crate::framework::ui::widgets::UiAdvancedImageMode::NineSlice(_))
    ));
    let UiNode::Image { presentation, .. } = &children[2] else {
        panic!("tile fixture must be an image")
    };
    assert!(matches!(
        presentation.to_widget_advanced_mode(),
        Some(crate::framework::ui::widgets::UiAdvancedImageMode::Tiled(_))
    ));
    assert!(matches!(
        &children[3],
        UiNode::Image {
            presentation: UiImagePresentation::AtlasFrame { frame, .. },
            ..
        } if frame.as_str() == "play"
    ));
}

#[test]
fn ui_document_style_rejects_unknown_and_cyclic_references() {
    let mut document = style_resource_document();
    if let UiNode::Container { style, .. } = &mut document.root {
        style.inline.opacity = Some(UiScalarValue::Token {
            token: style_id("missing"),
        });
    }
    assert_visual_error(
        ValidatedUiDocument::new(document).unwrap_err(),
        "UI_STYLE_TOKEN_UNKNOWN",
    );

    let mut document = style_resource_document();
    document.tokens.insert(
        style_id("cycle_a"),
        UiTokenValue::Reference {
            token: style_id("cycle_b"),
        },
    );
    document.tokens.insert(
        style_id("cycle_b"),
        UiTokenValue::Reference {
            token: style_id("cycle_a"),
        },
    );
    assert_visual_error(
        ValidatedUiDocument::new(document).unwrap_err(),
        "UI_STYLE_TOKEN_CYCLE",
    );

    let mut document = style_resource_document();
    document.styles.insert(
        style_id("cycle_a"),
        UiStyleDefinition {
            extends: Some(style_id("cycle_b")),
            ..default()
        },
    );
    document.styles.insert(
        style_id("cycle_b"),
        UiStyleDefinition {
            extends: Some(style_id("cycle_a")),
            ..default()
        },
    );
    assert_visual_error(
        ValidatedUiDocument::new(document).unwrap_err(),
        "UI_STYLE_REFERENCE_CYCLE",
    );
}

#[test]
fn ui_document_assets_reject_path_escape_kind_mismatch_and_shader_input() {
    for invalid_path in [
        "../escape.png",
        "ui/../escape.png",
        "ui\\escape.png",
        "C:/escape.png",
        "http://example.test/image.png",
        "data:image/png;base64,aaaa",
        "ui/%2fescape.png",
        "ui/image bad.png",
        "ui/image\tbad.png",
        "ui/image\nbad.png",
        "ui/image?.png",
        "ui/image#fragment.png",
        "ui/image%20bad.png",
        "other/image.png",
        "ui/Upper.png",
    ] {
        let mut value: Value = serde_json::from_str(STYLE_RESOURCE_DOCUMENT).unwrap();
        value["assets"]["hero_image"]["source"]["path"] = json!(invalid_path);
        assert_visual_error(
            UiDocument::parse_and_validate_json(&serde_json::to_string(&value).unwrap())
                .unwrap_err(),
            "UI_ASSET_PATH_INVALID",
        );
    }

    let mut value: Value = serde_json::from_str(STYLE_RESOURCE_DOCUMENT).unwrap();
    value["root"]["children"][0]["asset"] = json!("body_font");
    assert_visual_error(
        UiDocument::parse_and_validate_json(&serde_json::to_string(&value).unwrap()).unwrap_err(),
        "UI_ASSET_KIND_MISMATCH",
    );

    let mut value: Value = serde_json::from_str(STYLE_RESOURCE_DOCUMENT).unwrap();
    value["root"]["children"][0]["asset"] = json!("action_icon");
    assert_visual_error(
        UiDocument::parse_and_validate_json(&serde_json::to_string(&value).unwrap()).unwrap_err(),
        "UI_ASSET_KIND_MISMATCH",
    );

    let mut value: Value = serde_json::from_str(STYLE_RESOURCE_DOCUMENT).unwrap();
    value["assets"]["frosted_panel"]["source"]["material"] = json!("arbitrary_shader");
    assert_visual_error(
        UiDocument::parse_and_validate_json(&serde_json::to_string(&value).unwrap()).unwrap_err(),
        "UI_MATERIAL_NOT_ALLOWLISTED",
    );

    let mut value: Value = serde_json::from_str(STYLE_RESOURCE_DOCUMENT).unwrap();
    value["assets"]["frosted_panel"]["source"]["shader"] = json!("shaders/foreign.wgsl");
    let error =
        UiDocument::parse_and_validate_json(&serde_json::to_string(&value).unwrap()).unwrap_err();
    assert_eq!(error.code(), "UI_DOCUMENT_PARSE_FAILED");
    assert!(error.to_string().contains("unknown field `shader`"));

    let mut value: Value = serde_json::from_str(STYLE_RESOURCE_DOCUMENT).unwrap();
    value["assets"]["hero_image"]["source"]["path"] = json!("ui/images/code.exe");
    assert_visual_error(
        UiDocument::parse_and_validate_json(&serde_json::to_string(&value).unwrap()).unwrap_err(),
        "UI_ASSET_EXTENSION_MISMATCH",
    );
}

#[test]
fn ui_document_visual_budgets_reject_excessive_effects_materials_and_sizes() {
    let mut value: Value = serde_json::from_str(STYLE_RESOURCE_DOCUMENT).unwrap();
    let shadow = value["styles"]["panel_base"]["properties"]["shadows"][0].clone();
    value["styles"]["panel_base"]["properties"]["shadows"] =
        Value::Array(vec![shadow.clone(), shadow.clone(), shadow.clone(), shadow]);
    assert_visual_error(
        UiDocument::parse_and_validate_json(&serde_json::to_string(&value).unwrap()).unwrap_err(),
        "UI_STYLE_SHADOW_BUDGET_EXCEEDED",
    );

    let mut value: Value = serde_json::from_str(STYLE_RESOURCE_DOCUMENT).unwrap();
    value["root"]["style"]["inline"]["background"] = json!({
        "kind": "linear_gradient",
        "angle_degrees": 90.0,
        "stops": (0..7).map(|index| json!({
            "position": index as f32 / 6.0,
            "color": { "kind": "literal", "value": "#ffffffff" }
        })).collect::<Vec<_>>()
    });
    assert_visual_error(
        UiDocument::parse_and_validate_json(&serde_json::to_string(&value).unwrap()).unwrap_err(),
        "UI_STYLE_GRADIENT_STOP_BUDGET_EXCEEDED",
    );

    let mut value: Value = serde_json::from_str(STYLE_RESOURCE_DOCUMENT).unwrap();
    value["assets"]["hero_image"]["declared_size"]["width"] = json!(UI_ASSET_MAX_DIMENSION + 1);
    value["assets"]["hero_image"]["declared_size"]["decoded_bytes"] =
        json!(UI_ASSET_MAX_DECODED_BYTES + 1);
    let error =
        UiDocument::parse_and_validate_json(&serde_json::to_string(&value).unwrap()).unwrap_err();
    assert_visual_error_contains(&error, "UI_ASSET_DIMENSION_BUDGET_EXCEEDED");
    assert_visual_error_contains(&error, "UI_ASSET_DECODED_BYTES_BUDGET_EXCEEDED");

    let mut value: Value = serde_json::from_str(STYLE_RESOURCE_DOCUMENT).unwrap();
    let material = value["assets"]["frosted_panel"].clone();
    for index in 0..UI_DOCUMENT_MAX_MATERIALS {
        value["assets"][format!("extra_material_{index}")] = material.clone();
    }
    assert_visual_error(
        UiDocument::parse_and_validate_json(&serde_json::to_string(&value).unwrap()).unwrap_err(),
        "UI_ASSET_MATERIAL_BUDGET_EXCEEDED",
    );

    let mut value: Value = serde_json::from_str(STYLE_RESOURCE_DOCUMENT).unwrap();
    let image = value["assets"]["hero_image"].clone();
    for index in 0..4 {
        let mut extra = image.clone();
        extra["declared_size"]["decoded_bytes"] = json!(UI_ASSET_MAX_DECODED_BYTES);
        value["assets"][format!("extra_image_{index}")] = extra;
    }
    assert_visual_error(
        UiDocument::parse_and_validate_json(&serde_json::to_string(&value).unwrap()).unwrap_err(),
        "UI_ASSET_TOTAL_DECODED_BYTES_BUDGET_EXCEEDED",
    );
}

#[test]
fn ui_document_image_and_resource_validation_reports_stable_field_errors() {
    let mut value: Value = serde_json::from_str(STYLE_RESOURCE_DOCUMENT).unwrap();
    value["root"]["children"][0]["presentation"]["focus"]["x"] = json!(1.5);
    assert_visual_error(
        UiDocument::parse_and_validate_json(&serde_json::to_string(&value).unwrap()).unwrap_err(),
        "UI_IMAGE_FOCUS_INVALID",
    );

    for fit in ["contain", "stretch"] {
        let mut value: Value = serde_json::from_str(STYLE_RESOURCE_DOCUMENT).unwrap();
        value["root"]["children"][0]["presentation"]["fit"] = json!(fit);
        value["root"]["children"][0]["presentation"]["focus"]["y"] = json!(-0.1);
        assert_visual_error(
            UiDocument::parse_and_validate_json(&serde_json::to_string(&value).unwrap())
                .unwrap_err(),
            "UI_IMAGE_FOCUS_INVALID",
        );
    }

    let mut value: Value = serde_json::from_str(STYLE_RESOURCE_DOCUMENT).unwrap();
    value["root"]["children"][3]["presentation"]["focus"]["x"] = json!(1.1);
    assert_visual_error(
        UiDocument::parse_and_validate_json(&serde_json::to_string(&value).unwrap()).unwrap_err(),
        "UI_IMAGE_FOCUS_INVALID",
    );

    let mut document = style_resource_document();
    let UiNode::Container { children, .. } = &mut document.root else {
        panic!("fixture root must be a container")
    };
    let UiNode::Image { presentation, .. } = &mut children[0] else {
        panic!("fixture child must be an image")
    };
    let UiImagePresentation::Fit { focus, .. } = presentation else {
        panic!("fixture child must use fit presentation")
    };
    focus.x = f32::NAN;
    assert_visual_error(
        ValidatedUiDocument::new(document).unwrap_err(),
        "UI_IMAGE_FOCUS_INVALID",
    );

    let mut value: Value = serde_json::from_str(STYLE_RESOURCE_DOCUMENT).unwrap();
    value["root"]["children"][3]["presentation"]["frame"] = json!("missing");
    assert_visual_error(
        UiDocument::parse_and_validate_json(&serde_json::to_string(&value).unwrap()).unwrap_err(),
        "UI_ASSET_ATLAS_FRAME_UNKNOWN",
    );

    let mut value: Value = serde_json::from_str(STYLE_RESOURCE_DOCUMENT).unwrap();
    value["styles"]["panel_base"]["properties"]["text"]["font"] = json!("hero_image");
    assert_visual_error(
        UiDocument::parse_and_validate_json(&serde_json::to_string(&value).unwrap()).unwrap_err(),
        "UI_ASSET_KIND_MISMATCH",
    );

    for invalid_color in [
        json!("204060"),
        json!("#12345"),
        json!({
            "srgb": { "red": 1.1, "green": 0.0, "blue": 0.0, "alpha": 1.0 }
        }),
        json!({
            "srgb": { "red": 1.0, "green": 0.0, "blue": 0.0 }
        }),
    ] {
        let mut value: Value = serde_json::from_str(STYLE_RESOURCE_DOCUMENT).unwrap();
        value["tokens"]["surface"]["value"] = invalid_color;
        let error = UiDocument::parse_and_validate_json(&serde_json::to_string(&value).unwrap())
            .unwrap_err();
        assert_eq!(error.code(), "UI_DOCUMENT_PARSE_FAILED");
    }
}

fn style_resource_document() -> UiDocument {
    UiDocument::parse_and_validate_json(STYLE_RESOURCE_DOCUMENT)
        .unwrap()
        .into_document()
}

fn style_id(value: &str) -> UiStyleId {
    UiStyleId::from_str(value).unwrap()
}

fn assert_visual_error(error: UiDocumentError, code: &str) {
    assert_visual_error_contains(&error, code);
}

fn assert_visual_error_contains(error: &UiDocumentError, code: &str) {
    let UiDocumentError::InvalidVisual { errors } = error else {
        panic!("expected visual error {code}, got {error:?}");
    };
    assert!(
        errors.iter().any(|error| error.code == code),
        "missing {code}: {errors:?}"
    );
}

fn assert_layout_error(layout: &UiLayout, code: &str, path: &str) {
    let errors = layout.to_bevy_layout().unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.code == code && error.path == path),
        "missing {code} at {path}: {errors:?}"
    );
}

const UI_NAMESPACED_ID_PATTERN: &str = "^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)+$";

fn generated_schema() -> String {
    let schema = serde_json::to_value(schemars::schema_for!(UiDocument)).unwrap();
    let mut output = serde_json::to_string_pretty(&schema).unwrap();
    output.push('\n');
    output
}

fn maybe_update_golden(variable: &str, relative_path: &str, contents: &str) {
    if std::env::var_os(variable).is_none() {
        return;
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}
