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
