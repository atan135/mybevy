use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub const UI_GRID_MAX_TRACK_DEFINITIONS: usize = 32;
pub const UI_GRID_MAX_REPEAT: u16 = 16;
pub const UI_GRID_MAX_EXPANDED_TRACKS: usize = 64;
pub const UI_GRID_MAX_SPAN: u16 = 32;
pub const UI_LAYOUT_MAX_Z_INDEX: i32 = 1_000;
pub const UI_LAYOUT_MAX_SCROLLBAR_WIDTH: f32 = 64.0;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiLength {
    #[default]
    Auto,
    Px(f32),
    Percent(f32),
    Vw(f32),
    Vh(f32),
}

impl UiLength {
    fn to_bevy(self) -> Val {
        match self {
            Self::Auto => Val::Auto,
            Self::Px(value) => Val::Px(value),
            Self::Percent(value) => Val::Percent(value),
            Self::Vw(value) => Val::Vw(value),
            Self::Vh(value) => Val::Vh(value),
        }
    }

    fn numeric(self) -> Option<(u8, f32)> {
        match self {
            Self::Auto => None,
            Self::Px(value) => Some((0, value)),
            Self::Percent(value) => Some((1, value)),
            Self::Vw(value) => Some((2, value)),
            Self::Vh(value) => Some((3, value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiInsets {
    #[serde(default = "zero_length")]
    pub all: UiLength,
    #[serde(default)]
    pub left: Option<UiLength>,
    #[serde(default)]
    pub right: Option<UiLength>,
    #[serde(default)]
    pub top: Option<UiLength>,
    #[serde(default)]
    pub bottom: Option<UiLength>,
}

impl Default for UiInsets {
    fn default() -> Self {
        Self {
            all: zero_length(),
            left: None,
            right: None,
            top: None,
            bottom: None,
        }
    }
}

impl UiInsets {
    fn to_bevy(self) -> UiRect {
        UiRect {
            left: self.left.unwrap_or(self.all).to_bevy(),
            right: self.right.unwrap_or(self.all).to_bevy(),
            top: self.top.unwrap_or(self.all).to_bevy(),
            bottom: self.bottom.unwrap_or(self.all).to_bevy(),
        }
    }
}

const fn zero_length() -> UiLength {
    UiLength::Px(0.0)
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiDisplay {
    #[default]
    Flex,
    Grid,
    None,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiFlexDirection {
    Row,
    RowReverse,
    #[default]
    Column,
    ColumnReverse,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiFlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiAlignItems {
    #[default]
    Stretch,
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiAlignSelf {
    #[default]
    Auto,
    Stretch,
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiContentAlignment {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiOverflowAxis {
    #[default]
    Visible,
    Clip,
    Scroll,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiOverflow {
    #[serde(default)]
    pub x: UiOverflowAxis,
    #[serde(default)]
    pub y: UiOverflowAxis,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiGridTrackSize {
    #[default]
    Auto,
    Px(f32),
    Percent(f32),
    Vw(f32),
    Vh(f32),
    Fr(f32),
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiGridTrack {
    #[serde(default = "one_repeat")]
    pub repeat: u16,
    pub size: UiGridTrackSize,
}

const fn one_repeat() -> u16 {
    1
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiGridPlacement {
    #[serde(default)]
    pub start: Option<i16>,
    #[serde(default = "one_span")]
    pub span: u16,
}

impl Default for UiGridPlacement {
    fn default() -> Self {
        Self {
            start: None,
            span: one_span(),
        }
    }
}

const fn one_span() -> u16 {
    1
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiGridAutoFlow {
    #[default]
    Row,
    Column,
    RowDense,
    ColumnDense,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiAbsoluteContainingBlock {
    #[default]
    ParentBorderBox,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiAbsolutePosition {
    #[serde(default)]
    pub containing_block: UiAbsoluteContainingBlock,
    #[serde(default)]
    pub left: Option<UiLength>,
    #[serde(default)]
    pub right: Option<UiLength>,
    #[serde(default)]
    pub top: Option<UiLength>,
    #[serde(default)]
    pub bottom: Option<UiLength>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiLayoutPosition {
    #[default]
    Relative,
    Absolute(UiAbsolutePosition),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiLayout {
    #[serde(default)]
    pub display: UiDisplay,
    #[serde(default)]
    pub position: UiLayoutPosition,
    #[serde(default)]
    pub width: UiLength,
    #[serde(default)]
    pub height: UiLength,
    #[serde(default)]
    pub min_width: UiLength,
    #[serde(default)]
    pub min_height: UiLength,
    #[serde(default)]
    pub max_width: UiLength,
    #[serde(default)]
    pub max_height: UiLength,
    #[serde(default)]
    pub aspect_ratio: Option<f32>,
    #[serde(default)]
    pub margin: UiInsets,
    #[serde(default)]
    pub padding: UiInsets,
    #[serde(default)]
    pub border: UiInsets,
    #[serde(default = "zero_length")]
    pub gap: UiLength,
    #[serde(default)]
    pub row_gap: Option<UiLength>,
    #[serde(default)]
    pub column_gap: Option<UiLength>,
    #[serde(default)]
    pub align_items: UiAlignItems,
    #[serde(default)]
    pub justify_items: UiAlignItems,
    #[serde(default)]
    pub align_self: UiAlignSelf,
    #[serde(default)]
    pub justify_self: UiAlignSelf,
    #[serde(default)]
    pub align_content: UiContentAlignment,
    #[serde(default)]
    pub justify_content: UiContentAlignment,
    #[serde(default)]
    pub direction: UiFlexDirection,
    #[serde(default)]
    pub wrap: UiFlexWrap,
    #[serde(default)]
    pub flex_grow: f32,
    #[serde(default = "one_f32")]
    pub flex_shrink: f32,
    #[serde(default)]
    pub flex_basis: UiLength,
    #[serde(default)]
    pub overflow: UiOverflow,
    #[serde(default)]
    pub scrollbar_width: f32,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default)]
    pub grid_columns: Vec<UiGridTrack>,
    #[serde(default)]
    pub grid_rows: Vec<UiGridTrack>,
    #[serde(default)]
    pub grid_auto_columns: Vec<UiGridTrackSize>,
    #[serde(default)]
    pub grid_auto_rows: Vec<UiGridTrackSize>,
    #[serde(default)]
    pub grid_auto_flow: UiGridAutoFlow,
    #[serde(default)]
    pub grid_column: UiGridPlacement,
    #[serde(default)]
    pub grid_row: UiGridPlacement,
}

impl Default for UiLayout {
    fn default() -> Self {
        Self {
            display: default(),
            position: default(),
            width: default(),
            height: default(),
            min_width: default(),
            min_height: default(),
            max_width: default(),
            max_height: default(),
            aspect_ratio: None,
            margin: default(),
            padding: default(),
            border: default(),
            gap: zero_length(),
            row_gap: None,
            column_gap: None,
            align_items: default(),
            justify_items: default(),
            align_self: default(),
            justify_self: default(),
            align_content: default(),
            justify_content: default(),
            direction: default(),
            wrap: default(),
            flex_grow: 0.0,
            flex_shrink: one_f32(),
            flex_basis: default(),
            overflow: default(),
            scrollbar_width: 0.0,
            z_index: 0,
            grid_columns: Vec::new(),
            grid_rows: Vec::new(),
            grid_auto_columns: Vec::new(),
            grid_auto_rows: Vec::new(),
            grid_auto_flow: default(),
            grid_column: default(),
            grid_row: default(),
        }
    }
}

const fn one_f32() -> f32 {
    1.0
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UiLayoutFieldError {
    pub code: &'static str,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiBevyLayout {
    pub node: Node,
    pub z_index: Option<ZIndex>,
}

impl UiLayout {
    pub fn to_bevy_layout(&self) -> Result<UiBevyLayout, Vec<UiLayoutFieldError>> {
        let errors = self.validate_fields();
        if !errors.is_empty() {
            return Err(errors);
        }

        let (position_type, left, right, top, bottom) = match self.position {
            UiLayoutPosition::Relative => (
                PositionType::Relative,
                Val::Auto,
                Val::Auto,
                Val::Auto,
                Val::Auto,
            ),
            UiLayoutPosition::Absolute(absolute) => (
                PositionType::Absolute,
                absolute.left.unwrap_or_default().to_bevy(),
                absolute.right.unwrap_or_default().to_bevy(),
                absolute.top.unwrap_or_default().to_bevy(),
                absolute.bottom.unwrap_or_default().to_bevy(),
            ),
        };
        let node = Node {
            display: map_display(self.display),
            position_type,
            left,
            right,
            top,
            bottom,
            width: self.width.to_bevy(),
            height: self.height.to_bevy(),
            min_width: self.min_width.to_bevy(),
            min_height: self.min_height.to_bevy(),
            max_width: self.max_width.to_bevy(),
            max_height: self.max_height.to_bevy(),
            aspect_ratio: self.aspect_ratio,
            margin: self.margin.to_bevy(),
            padding: self.padding.to_bevy(),
            border: self.border.to_bevy(),
            row_gap: self.row_gap.unwrap_or(self.gap).to_bevy(),
            column_gap: self.column_gap.unwrap_or(self.gap).to_bevy(),
            align_items: map_align_items(self.align_items),
            justify_items: map_justify_items(self.justify_items),
            align_self: map_align_self(self.align_self),
            justify_self: map_justify_self(self.justify_self),
            align_content: map_align_content(self.align_content),
            justify_content: map_justify_content(self.justify_content),
            flex_direction: map_direction(self.direction),
            flex_wrap: map_wrap(self.wrap),
            flex_grow: self.flex_grow,
            flex_shrink: self.flex_shrink,
            flex_basis: self.flex_basis.to_bevy(),
            overflow: Overflow {
                x: map_overflow(self.overflow.x),
                y: map_overflow(self.overflow.y),
            },
            scrollbar_width: self.scrollbar_width,
            grid_template_columns: map_repeated_tracks(&self.grid_columns),
            grid_template_rows: map_repeated_tracks(&self.grid_rows),
            grid_auto_columns: self
                .grid_auto_columns
                .iter()
                .copied()
                .map(map_grid_track)
                .collect(),
            grid_auto_rows: self
                .grid_auto_rows
                .iter()
                .copied()
                .map(map_grid_track)
                .collect(),
            grid_auto_flow: map_grid_flow(self.grid_auto_flow),
            grid_column: map_grid_placement(self.grid_column),
            grid_row: map_grid_placement(self.grid_row),
            ..default()
        };
        Ok(UiBevyLayout {
            node,
            z_index: (self.z_index != 0).then_some(ZIndex(self.z_index)),
        })
    }

    pub(crate) fn validate_fields(&self) -> Vec<UiLayoutFieldError> {
        let mut errors = Vec::new();
        for (path, value) in [
            ("width", self.width),
            ("height", self.height),
            ("min_width", self.min_width),
            ("min_height", self.min_height),
            ("max_width", self.max_width),
            ("max_height", self.max_height),
            ("gap", self.gap),
        ] {
            validate_length(path, value, path != "gap", true, &mut errors);
        }
        for (path, value) in [("row_gap", self.row_gap), ("column_gap", self.column_gap)] {
            if let Some(value) = value {
                validate_length(path, value, false, true, &mut errors);
            }
        }
        validate_insets("margin", self.margin, &mut errors);
        validate_insets("padding", self.padding, &mut errors);
        validate_insets("border", self.border, &mut errors);
        validate_scalar("aspect_ratio", self.aspect_ratio, true, &mut errors);
        validate_scalar("flex_grow", Some(self.flex_grow), false, &mut errors);
        validate_scalar("flex_shrink", Some(self.flex_shrink), false, &mut errors);
        validate_length("flex_basis", self.flex_basis, true, true, &mut errors);
        validate_constraints(
            "width",
            self.min_width,
            self.width,
            self.max_width,
            &mut errors,
        );
        validate_constraints(
            "height",
            self.min_height,
            self.height,
            self.max_height,
            &mut errors,
        );

        if !self.scrollbar_width.is_finite() {
            push(&mut errors, "UI_LAYOUT_VALUE_NON_FINITE", "scrollbar_width");
        } else if !(0.0..=UI_LAYOUT_MAX_SCROLLBAR_WIDTH).contains(&self.scrollbar_width) {
            push(
                &mut errors,
                "UI_LAYOUT_SCROLLBAR_OUT_OF_RANGE",
                "scrollbar_width",
            );
        }
        if self.z_index.unsigned_abs() > UI_LAYOUT_MAX_Z_INDEX as u32 {
            push(&mut errors, "UI_LAYOUT_Z_INDEX_OUT_OF_RANGE", "z_index");
        }

        validate_grid(self, &mut errors);
        if let UiLayoutPosition::Absolute(absolute) = self.position {
            validate_absolute(self, absolute, &mut errors);
        }
        errors
    }
}

impl super::UiLayoutPatch {
    pub(crate) fn validate_fields(&self) -> Vec<UiLayoutFieldError> {
        let mut errors = Vec::new();
        for (path, value) in [
            ("width", self.width),
            ("height", self.height),
            ("min_width", self.min_width),
            ("min_height", self.min_height),
            ("max_width", self.max_width),
            ("max_height", self.max_height),
            ("flex_basis", self.flex_basis),
        ] {
            if let Some(value) = value {
                validate_length(path, value, true, true, &mut errors);
            }
        }
        for (path, value) in [
            ("gap", self.gap),
            ("row_gap", self.row_gap),
            ("column_gap", self.column_gap),
        ] {
            if let Some(value) = value {
                validate_length(path, value, false, true, &mut errors);
            }
        }
        for (path, value) in [
            ("margin", self.margin),
            ("padding", self.padding),
            ("border", self.border),
        ] {
            if let Some(value) = value {
                validate_insets(path, value, &mut errors);
            }
        }
        validate_scalar("aspect_ratio", self.aspect_ratio, true, &mut errors);
        validate_scalar("flex_grow", self.flex_grow, false, &mut errors);
        validate_scalar("flex_shrink", self.flex_shrink, false, &mut errors);
        validate_patch_constraints(
            "width",
            self.min_width,
            self.width,
            self.max_width,
            &mut errors,
        );
        validate_patch_constraints(
            "height",
            self.min_height,
            self.height,
            self.max_height,
            &mut errors,
        );
        if let Some(value) = self.scrollbar_width {
            if !value.is_finite() {
                push(&mut errors, "UI_LAYOUT_VALUE_NON_FINITE", "scrollbar_width");
            } else if !(0.0..=UI_LAYOUT_MAX_SCROLLBAR_WIDTH).contains(&value) {
                push(
                    &mut errors,
                    "UI_LAYOUT_SCROLLBAR_OUT_OF_RANGE",
                    "scrollbar_width",
                );
            }
        }
        if self
            .z_index
            .is_some_and(|value| value.unsigned_abs() > UI_LAYOUT_MAX_Z_INDEX as u32)
        {
            push(&mut errors, "UI_LAYOUT_Z_INDEX_OUT_OF_RANGE", "z_index");
        }
        if let Some(UiLayoutPosition::Absolute(absolute)) = self.position {
            for (name, value) in [
                ("position.absolute.left", absolute.left),
                ("position.absolute.right", absolute.right),
                ("position.absolute.top", absolute.top),
                ("position.absolute.bottom", absolute.bottom),
            ] {
                if let Some(value) = value {
                    validate_length(name, value, false, false, &mut errors);
                }
            }
        }

        let has_grid_container_fields = self.grid_columns.is_some()
            || self.grid_rows.is_some()
            || self.grid_auto_columns.is_some()
            || self.grid_auto_rows.is_some();
        if has_grid_container_fields
            && self
                .display
                .is_some_and(|display| display != UiDisplay::Grid)
        {
            push(&mut errors, "UI_LAYOUT_FIELD_NOT_APPLICABLE", "display");
        }

        let grid_probe = UiLayout {
            display: UiDisplay::Grid,
            grid_columns: self.grid_columns.clone().unwrap_or_default(),
            grid_rows: self.grid_rows.clone().unwrap_or_default(),
            grid_auto_columns: self.grid_auto_columns.clone().unwrap_or_default(),
            grid_auto_rows: self.grid_auto_rows.clone().unwrap_or_default(),
            grid_column: self.grid_column.unwrap_or_default(),
            grid_row: self.grid_row.unwrap_or_default(),
            ..default()
        };
        validate_grid(&grid_probe, &mut errors);
        errors
    }
}

fn validate_patch_constraints(
    axis: &str,
    min: Option<UiLength>,
    value: Option<UiLength>,
    max: Option<UiLength>,
    errors: &mut Vec<UiLayoutFieldError>,
) {
    for (left_name, left, right_name, right) in [
        ("min", min, "max", max),
        ("min", min, "value", value),
        ("value", value, "max", max),
    ] {
        let (Some(left), Some(right)) = (left, right) else {
            continue;
        };
        if let (Some((left_unit, left)), Some((right_unit, right))) =
            (left.numeric(), right.numeric())
            && left_unit == right_unit
            && left > right
        {
            push(
                errors,
                "UI_LAYOUT_CONSTRAINT_CONTRADICTION",
                &format!("{axis}.{left_name}_{right_name}"),
            );
        }
    }
}

fn validate_length(
    path: &str,
    value: UiLength,
    allow_auto: bool,
    nonnegative: bool,
    errors: &mut Vec<UiLayoutFieldError>,
) {
    let Some((unit, number)) = value.numeric() else {
        if !allow_auto {
            push(errors, "UI_LAYOUT_AUTO_NOT_ALLOWED", path);
        }
        return;
    };
    if !number.is_finite() {
        push(errors, "UI_LAYOUT_VALUE_NON_FINITE", path);
    } else if nonnegative && number < 0.0 {
        push(errors, "UI_LAYOUT_LENGTH_NEGATIVE", path);
    } else if unit != 0 && !(0.0..=100.0).contains(&number) {
        push(errors, "UI_LAYOUT_PERCENT_OUT_OF_RANGE", path);
    }
}

fn validate_insets(prefix: &str, insets: UiInsets, errors: &mut Vec<UiLayoutFieldError>) {
    validate_length(&format!("{prefix}.all"), insets.all, false, true, errors);
    for (name, value) in [
        ("left", insets.left),
        ("right", insets.right),
        ("top", insets.top),
        ("bottom", insets.bottom),
    ] {
        if let Some(value) = value {
            validate_length(&format!("{prefix}.{name}"), value, false, true, errors);
        }
    }
}

fn validate_scalar(
    path: &str,
    value: Option<f32>,
    strictly_positive: bool,
    errors: &mut Vec<UiLayoutFieldError>,
) {
    let Some(value) = value else { return };
    if !value.is_finite() {
        push(errors, "UI_LAYOUT_VALUE_NON_FINITE", path);
    } else if (strictly_positive && value <= 0.0) || (!strictly_positive && value < 0.0) {
        push(errors, "UI_LAYOUT_LENGTH_NEGATIVE", path);
    }
}

fn validate_constraints(
    axis: &str,
    min: UiLength,
    value: UiLength,
    max: UiLength,
    errors: &mut Vec<UiLayoutFieldError>,
) {
    for (left_name, left, right_name, right) in [
        ("min", min, "max", max),
        ("min", min, "value", value),
        ("value", value, "max", max),
    ] {
        if let (Some((left_unit, left)), Some((right_unit, right))) =
            (left.numeric(), right.numeric())
            && left_unit == right_unit
            && left > right
        {
            push(
                errors,
                "UI_LAYOUT_CONSTRAINT_CONTRADICTION",
                &format!("{axis}.{left_name}_{right_name}"),
            );
        }
    }
}

fn validate_grid(layout: &UiLayout, errors: &mut Vec<UiLayoutFieldError>) {
    let has_grid_container_fields = !layout.grid_columns.is_empty()
        || !layout.grid_rows.is_empty()
        || !layout.grid_auto_columns.is_empty()
        || !layout.grid_auto_rows.is_empty();
    if has_grid_container_fields && layout.display != UiDisplay::Grid {
        push(&mut *errors, "UI_LAYOUT_FIELD_NOT_APPLICABLE", "display");
    }
    for (name, tracks) in [
        ("grid_columns", layout.grid_columns.as_slice()),
        ("grid_rows", layout.grid_rows.as_slice()),
    ] {
        if tracks.len() > UI_GRID_MAX_TRACK_DEFINITIONS {
            push(errors, "UI_LAYOUT_GRID_TRACK_LIMIT", name);
        }
        let mut expanded = 0usize;
        for (index, track) in tracks.iter().enumerate() {
            if track.repeat == 0 || track.repeat > UI_GRID_MAX_REPEAT {
                push(
                    errors,
                    "UI_LAYOUT_GRID_REPEAT_INVALID",
                    &format!("{name}[{index}].repeat"),
                );
            }
            expanded = expanded.saturating_add(usize::from(track.repeat));
            validate_track_size(&format!("{name}[{index}].size"), track.size, errors);
        }
        if expanded > UI_GRID_MAX_EXPANDED_TRACKS {
            push(errors, "UI_LAYOUT_GRID_TRACK_LIMIT", name);
        }
    }
    for (name, tracks) in [
        ("grid_auto_columns", layout.grid_auto_columns.as_slice()),
        ("grid_auto_rows", layout.grid_auto_rows.as_slice()),
    ] {
        if tracks.len() > UI_GRID_MAX_TRACK_DEFINITIONS {
            push(errors, "UI_LAYOUT_GRID_TRACK_LIMIT", name);
        }
        for (index, size) in tracks.iter().copied().enumerate() {
            validate_track_size(&format!("{name}[{index}]"), size, errors);
        }
    }
    for (name, placement) in [
        ("grid_column", layout.grid_column),
        ("grid_row", layout.grid_row),
    ] {
        if placement.start == Some(0) {
            push(
                errors,
                "UI_LAYOUT_GRID_PLACEMENT_INVALID",
                &format!("{name}.start"),
            );
        }
        if placement.span == 0 || placement.span > UI_GRID_MAX_SPAN {
            push(
                errors,
                "UI_LAYOUT_GRID_SPAN_INVALID",
                &format!("{name}.span"),
            );
        }
    }
}

fn validate_track_size(path: &str, size: UiGridTrackSize, errors: &mut Vec<UiLayoutFieldError>) {
    let (unit, value) = match size {
        UiGridTrackSize::Auto => return,
        UiGridTrackSize::Px(value) => (0, value),
        UiGridTrackSize::Percent(value) => (1, value),
        UiGridTrackSize::Vw(value) => (2, value),
        UiGridTrackSize::Vh(value) => (3, value),
        UiGridTrackSize::Fr(value) => (4, value),
    };
    if !value.is_finite() {
        push(errors, "UI_LAYOUT_VALUE_NON_FINITE", path);
    } else if value < 0.0 {
        push(errors, "UI_LAYOUT_LENGTH_NEGATIVE", path);
    } else if matches!(unit, 1..=3) && value > 100.0 {
        push(errors, "UI_LAYOUT_PERCENT_OUT_OF_RANGE", path);
    }
}

fn validate_absolute(
    layout: &UiLayout,
    absolute: UiAbsolutePosition,
    errors: &mut Vec<UiLayoutFieldError>,
) {
    for (name, value) in [
        ("position.absolute.left", absolute.left),
        ("position.absolute.right", absolute.right),
        ("position.absolute.top", absolute.top),
        ("position.absolute.bottom", absolute.bottom),
    ] {
        if let Some(value) = value {
            validate_length(name, value, false, false, errors);
        }
    }
    validate_absolute_axis(
        "horizontal",
        layout.width != UiLength::Auto,
        absolute.left.is_some(),
        absolute.right.is_some(),
        errors,
    );
    validate_absolute_axis(
        "vertical",
        layout.height != UiLength::Auto,
        absolute.top.is_some(),
        absolute.bottom.is_some(),
        errors,
    );
}

fn validate_absolute_axis(
    axis: &str,
    has_size: bool,
    has_start: bool,
    has_end: bool,
    errors: &mut Vec<UiLayoutFieldError>,
) {
    let anchors = u8::from(has_start) + u8::from(has_end);
    if has_size && anchors == 2 {
        push(
            errors,
            "UI_LAYOUT_ABSOLUTE_AXIS_OVERCONSTRAINED",
            &format!("position.absolute.{axis}"),
        );
    } else if (has_size && anchors != 1) || (!has_size && anchors != 2) {
        push(
            errors,
            "UI_LAYOUT_ABSOLUTE_AXIS_UNDERCONSTRAINED",
            &format!("position.absolute.{axis}"),
        );
    }
}

fn push(errors: &mut Vec<UiLayoutFieldError>, code: &'static str, path: &str) {
    errors.push(UiLayoutFieldError {
        code,
        path: path.to_owned(),
    });
}

fn map_display(value: UiDisplay) -> Display {
    match value {
        UiDisplay::Flex => Display::Flex,
        UiDisplay::Grid => Display::Grid,
        UiDisplay::None => Display::None,
    }
}

fn map_direction(value: UiFlexDirection) -> FlexDirection {
    match value {
        UiFlexDirection::Row => FlexDirection::Row,
        UiFlexDirection::RowReverse => FlexDirection::RowReverse,
        UiFlexDirection::Column => FlexDirection::Column,
        UiFlexDirection::ColumnReverse => FlexDirection::ColumnReverse,
    }
}

fn map_wrap(value: UiFlexWrap) -> FlexWrap {
    match value {
        UiFlexWrap::NoWrap => FlexWrap::NoWrap,
        UiFlexWrap::Wrap => FlexWrap::Wrap,
        UiFlexWrap::WrapReverse => FlexWrap::WrapReverse,
    }
}

fn map_align_items(value: UiAlignItems) -> AlignItems {
    match value {
        UiAlignItems::Stretch => AlignItems::Stretch,
        UiAlignItems::Start => AlignItems::Start,
        UiAlignItems::Center => AlignItems::Center,
        UiAlignItems::End => AlignItems::End,
    }
}

fn map_justify_items(value: UiAlignItems) -> JustifyItems {
    match value {
        UiAlignItems::Stretch => JustifyItems::Stretch,
        UiAlignItems::Start => JustifyItems::Start,
        UiAlignItems::Center => JustifyItems::Center,
        UiAlignItems::End => JustifyItems::End,
    }
}

fn map_align_self(value: UiAlignSelf) -> AlignSelf {
    match value {
        UiAlignSelf::Auto => AlignSelf::Auto,
        UiAlignSelf::Stretch => AlignSelf::Stretch,
        UiAlignSelf::Start => AlignSelf::Start,
        UiAlignSelf::Center => AlignSelf::Center,
        UiAlignSelf::End => AlignSelf::End,
    }
}

fn map_justify_self(value: UiAlignSelf) -> JustifySelf {
    match value {
        UiAlignSelf::Auto => JustifySelf::Auto,
        UiAlignSelf::Stretch => JustifySelf::Stretch,
        UiAlignSelf::Start => JustifySelf::Start,
        UiAlignSelf::Center => JustifySelf::Center,
        UiAlignSelf::End => JustifySelf::End,
    }
}

fn map_align_content(value: UiContentAlignment) -> AlignContent {
    match value {
        UiContentAlignment::Start => AlignContent::Start,
        UiContentAlignment::Center => AlignContent::Center,
        UiContentAlignment::End => AlignContent::End,
        UiContentAlignment::SpaceBetween => AlignContent::SpaceBetween,
        UiContentAlignment::SpaceAround => AlignContent::SpaceAround,
        UiContentAlignment::SpaceEvenly => AlignContent::SpaceEvenly,
    }
}

fn map_justify_content(value: UiContentAlignment) -> JustifyContent {
    match value {
        UiContentAlignment::Start => JustifyContent::Start,
        UiContentAlignment::Center => JustifyContent::Center,
        UiContentAlignment::End => JustifyContent::End,
        UiContentAlignment::SpaceBetween => JustifyContent::SpaceBetween,
        UiContentAlignment::SpaceAround => JustifyContent::SpaceAround,
        UiContentAlignment::SpaceEvenly => JustifyContent::SpaceEvenly,
    }
}

fn map_overflow(value: UiOverflowAxis) -> OverflowAxis {
    match value {
        UiOverflowAxis::Visible => OverflowAxis::Visible,
        UiOverflowAxis::Clip => OverflowAxis::Clip,
        UiOverflowAxis::Scroll => OverflowAxis::Scroll,
    }
}

fn map_grid_flow(value: UiGridAutoFlow) -> GridAutoFlow {
    match value {
        UiGridAutoFlow::Row => GridAutoFlow::Row,
        UiGridAutoFlow::Column => GridAutoFlow::Column,
        UiGridAutoFlow::RowDense => GridAutoFlow::RowDense,
        UiGridAutoFlow::ColumnDense => GridAutoFlow::ColumnDense,
    }
}

fn map_repeated_tracks(tracks: &[UiGridTrack]) -> Vec<RepeatedGridTrack> {
    tracks
        .iter()
        .map(|track| match track.size {
            UiGridTrackSize::Auto => RepeatedGridTrack::auto(track.repeat),
            UiGridTrackSize::Px(value) => RepeatedGridTrack::px(track.repeat, value),
            UiGridTrackSize::Percent(value) => RepeatedGridTrack::percent(track.repeat, value),
            UiGridTrackSize::Vw(value) => RepeatedGridTrack::vw(track.repeat, value),
            UiGridTrackSize::Vh(value) => RepeatedGridTrack::vh(track.repeat, value),
            UiGridTrackSize::Fr(value) => RepeatedGridTrack::flex(track.repeat, value),
        })
        .collect()
}

fn map_grid_track(size: UiGridTrackSize) -> GridTrack {
    match size {
        UiGridTrackSize::Auto => GridTrack::auto(),
        UiGridTrackSize::Px(value) => GridTrack::px(value),
        UiGridTrackSize::Percent(value) => GridTrack::percent(value),
        UiGridTrackSize::Vw(value) => GridTrack::vw(value),
        UiGridTrackSize::Vh(value) => GridTrack::vh(value),
        UiGridTrackSize::Fr(value) => GridTrack::flex(value),
    }
}

fn map_grid_placement(value: UiGridPlacement) -> GridPlacement {
    match value.start {
        Some(start) => GridPlacement::start_span(start, value.span),
        None if value.span == 1 => GridPlacement::auto(),
        None => GridPlacement::span(value.span),
    }
}
