use std::{fmt, path::Component};

use bevy::{
    asset::LoadState,
    prelude::*,
    sprite::{BorderRect, SliceScaleMode, TextureSlicer},
};
use serde::{Deserialize, Serialize};

const INVALID_IMAGE_FALLBACK_WIDTH: f32 = 96.0;
const INVALID_IMAGE_FALLBACK_HEIGHT: f32 = 64.0;
const MIN_TILE_STRETCH_VALUE: f32 = 0.001;
pub(crate) const MAX_SLICE_REPEAT_BUDGET: u32 = 4_096;
pub(crate) const MAX_TILE_REPEAT_BUDGET: u32 = 65_536;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum UiImageFit {
    Natural,
    Stretch,
    Contain,
    Cover { focus: UiImageFocus },
}

impl UiImageFit {
    pub(crate) const fn cover(focus: UiImageFocus) -> Self {
        Self::Cover { focus }
    }

    pub(crate) const fn to_node_image_mode(self) -> NodeImageMode {
        match self {
            Self::Natural => NodeImageMode::Auto,
            Self::Stretch | Self::Contain | Self::Cover { .. } => NodeImageMode::Stretch,
        }
    }

    pub(crate) const fn to_align_self(self) -> AlignSelf {
        match self {
            Self::Natural | Self::Contain => AlignSelf::Center,
            Self::Stretch | Self::Cover { .. } => AlignSelf::Stretch,
        }
    }

    fn validation_error(self) -> Option<UiImageError> {
        match self {
            Self::Cover { focus } if !focus.is_finite() => Some(UiImageError::NonFiniteFocus),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct UiImagePixelSize {
    pub width: u32,
    pub height: u32,
}

impl UiImagePixelSize {
    pub(crate) const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    const fn is_zero(self) -> bool {
        self.width == 0 || self.height == 0
    }

    fn as_vec2(self) -> Vec2 {
        Vec2::new(self.width as f32, self.height as f32)
    }

    fn as_uvec2(self) -> UVec2 {
        UVec2::new(self.width, self.height)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct UiImagePixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl UiImagePixelRect {
    pub(crate) const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    const fn size(self) -> UiImagePixelSize {
        UiImagePixelSize::new(self.width, self.height)
    }

    fn as_rect(self) -> Rect {
        Rect::from_corners(
            Vec2::new(self.x as f32, self.y as f32),
            Vec2::new(
                self.x.saturating_add(self.width) as f32,
                self.y.saturating_add(self.height) as f32,
            ),
        )
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct UiImageTextureSource {
    pub path: String,
    pub size: UiImagePixelSize,
}

impl UiImageTextureSource {
    pub(crate) fn new(path: impl Into<String>, size: UiImagePixelSize) -> Self {
        Self {
            path: path.into(),
            size,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), UiImageError> {
        let path = self.path.as_str();
        if path.is_empty()
            || path.trim() != path
            || path.contains('\\')
            || path.contains(':')
            || path
                .split('/')
                .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
            || std::path::Path::new(path).is_absolute()
            || std::path::Path::new(path).components().any(|component| {
                matches!(
                    component,
                    Component::Prefix(_)
                        | Component::RootDir
                        | Component::CurDir
                        | Component::ParentDir
                )
            })
        {
            return Err(UiImageError::InvalidSourcePath);
        }
        if self.size.is_zero() {
            return Err(UiImageError::ZeroDeclaredSourceSize);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct UiImagePivot {
    pub x: f32,
    pub y: f32,
}

impl UiImagePivot {
    /// Normalized untrimmed-frame coordinates: (0, 0) is top-left and (1, 1) is bottom-right.
    pub(crate) const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    fn validate(self) -> Result<(), UiImageError> {
        if !self.x.is_finite() || !self.y.is_finite() {
            return Err(UiImageError::NonFinitePivot);
        }
        if !(0.0..=1.0).contains(&self.x) || !(0.0..=1.0).contains(&self.y) {
            return Err(UiImageError::PivotOutOfBounds);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct UiAtlasFrame {
    pub source: UiImageTextureSource,
    pub rect: UiImagePixelRect,
    pub original_size: UiImagePixelSize,
    pub pivot: Option<UiImagePivot>,
}

impl UiAtlasFrame {
    pub(crate) fn validate(&self) -> Result<(), UiImageError> {
        self.source.validate()?;
        if self.rect.size().is_zero() {
            return Err(UiImageError::ZeroFrameSize);
        }
        let Some(max_x) = self.rect.x.checked_add(self.rect.width) else {
            return Err(UiImageError::FrameOutOfBounds);
        };
        let Some(max_y) = self.rect.y.checked_add(self.rect.height) else {
            return Err(UiImageError::FrameOutOfBounds);
        };
        if max_x > self.source.size.width || max_y > self.source.size.height {
            return Err(UiImageError::FrameOutOfBounds);
        }
        if self.original_size.is_zero()
            || self.original_size.width < self.rect.width
            || self.original_size.height < self.rect.height
        {
            return Err(UiImageError::OriginalSizeSmallerThanFrame);
        }
        if let Some(pivot) = self.pivot {
            pivot.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct UiNineSliceInsets {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl UiNineSliceInsets {
    pub(crate) const fn all(value: f32) -> Self {
        Self {
            left: value,
            right: value,
            top: value,
            bottom: value,
        }
    }

    fn validate(self, source_size: Vec2) -> Result<(), UiImageError> {
        let values = [self.left, self.right, self.top, self.bottom];
        if values.iter().any(|value| !value.is_finite()) {
            return Err(UiImageError::NonFiniteSliceInset);
        }
        if values.iter().any(|value| *value < 0.0) {
            return Err(UiImageError::NegativeSliceInset);
        }
        if self.left + self.right >= source_size.x {
            return Err(UiImageError::SliceInsetsOutOfBounds(
                UiImageAxis::Horizontal,
            ));
        }
        if self.top + self.bottom >= source_size.y {
            return Err(UiImageError::SliceInsetsOutOfBounds(UiImageAxis::Vertical));
        }
        Ok(())
    }

    fn to_border_rect(self) -> BorderRect {
        BorderRect {
            min_inset: Vec2::new(self.left, self.top),
            max_inset: Vec2::new(self.right, self.bottom),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum UiSliceScaleMode {
    Stretch,
    Tile { stretch_value: f32 },
}

impl UiSliceScaleMode {
    fn validate(self) -> Result<(), UiImageError> {
        let Self::Tile { stretch_value } = self else {
            return Ok(());
        };
        if !stretch_value.is_finite() {
            return Err(UiImageError::NonFiniteSliceScale);
        }
        if !(MIN_TILE_STRETCH_VALUE..=1.0).contains(&stretch_value) {
            return Err(UiImageError::InvalidSliceScale);
        }
        Ok(())
    }

    fn to_bevy(self) -> SliceScaleMode {
        match self {
            Self::Stretch => SliceScaleMode::Stretch,
            Self::Tile { stretch_value } => SliceScaleMode::Tile { stretch_value },
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct UiNineSlice {
    pub insets: UiNineSliceInsets,
    pub center: UiSliceScaleMode,
    pub sides: UiSliceScaleMode,
    pub max_corner_scale: f32,
    pub max_generated_slices: u32,
}

impl UiNineSlice {
    pub(crate) const fn uniform(inset: f32) -> Self {
        Self {
            insets: UiNineSliceInsets::all(inset),
            center: UiSliceScaleMode::Stretch,
            sides: UiSliceScaleMode::Stretch,
            max_corner_scale: 1.0,
            max_generated_slices: 256,
        }
    }

    fn validate_static(self) -> Result<(), UiImageError> {
        self.center.validate()?;
        self.sides.validate()?;
        if !self.max_corner_scale.is_finite() {
            return Err(UiImageError::NonFiniteCornerScale);
        }
        if !(0.0..=1.0).contains(&self.max_corner_scale) || self.max_corner_scale == 0.0 {
            return Err(UiImageError::InvalidCornerScale);
        }
        if self.max_generated_slices == 0 || self.max_generated_slices > MAX_SLICE_REPEAT_BUDGET {
            return Err(UiImageError::InvalidRepeatBudget);
        }
        Ok(())
    }

    fn to_texture_slicer(self) -> TextureSlicer {
        TextureSlicer {
            border: self.insets.to_border_rect(),
            center_scale_mode: self.center.to_bevy(),
            sides_scale_mode: self.sides.to_bevy(),
            max_corner_scale: self.max_corner_scale,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiTileAxis {
    X,
    Y,
    Both,
}

impl UiTileAxis {
    const fn flags(self) -> (bool, bool) {
        match self {
            Self::X => (true, false),
            Self::Y => (false, true),
            Self::Both => (true, true),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct UiImageTiling {
    pub axis: UiTileAxis,
    pub stretch_value: f32,
    pub max_repeats: u32,
}

impl UiImageTiling {
    pub(crate) const fn new(axis: UiTileAxis) -> Self {
        Self {
            axis,
            stretch_value: 1.0,
            max_repeats: 256,
        }
    }

    fn validate_static(self) -> Result<(), UiImageError> {
        if !self.stretch_value.is_finite() {
            return Err(UiImageError::NonFiniteTileScale);
        }
        if !(MIN_TILE_STRETCH_VALUE..=1.0).contains(&self.stretch_value) {
            return Err(UiImageError::InvalidTileScale);
        }
        if self.max_repeats == 0 || self.max_repeats > MAX_TILE_REPEAT_BUDGET {
            return Err(UiImageError::InvalidRepeatBudget);
        }
        Ok(())
    }

    fn to_node_image_mode(self) -> NodeImageMode {
        let (tile_x, tile_y) = self.axis.flags();
        NodeImageMode::Tiled {
            tile_x,
            tile_y,
            stretch_value: self.stretch_value,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub(crate) enum UiAdvancedImageSource {
    Texture(UiImageTextureSource),
    AtlasFrame(UiAtlasFrame),
}

impl UiAdvancedImageSource {
    fn texture(&self) -> &UiImageTextureSource {
        match self {
            Self::Texture(source) => source,
            Self::AtlasFrame(frame) => &frame.source,
        }
    }

    fn validate(&self) -> Result<(), UiImageError> {
        match self {
            Self::Texture(source) => source.validate(),
            Self::AtlasFrame(frame) => frame.validate(),
        }
    }

    fn rect(&self) -> Option<Rect> {
        match self {
            Self::Texture(_) => None,
            Self::AtlasFrame(frame) => Some(frame.rect.as_rect()),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub(crate) enum UiAdvancedImageMode {
    Stretch,
    NineSlice(UiNineSlice),
    Tiled(UiImageTiling),
}

impl UiAdvancedImageMode {
    fn validate_static(self) -> Result<(), UiImageError> {
        match self {
            Self::Stretch => Ok(()),
            Self::NineSlice(slice) => slice.validate_static(),
            Self::Tiled(tiling) => tiling.validate_static(),
        }
    }

    fn initial_node_image_mode(self) -> NodeImageMode {
        match self {
            Self::Stretch => NodeImageMode::Stretch,
            Self::NineSlice(slice) => NodeImageMode::Sliced(slice.to_texture_slicer()),
            Self::Tiled(tiling) => tiling.to_node_image_mode(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct UiAdvancedImageSpec {
    pub source: UiAdvancedImageSource,
    pub mode: UiAdvancedImageMode,
}

impl UiAdvancedImageSpec {
    pub(crate) fn validate(&self) -> Result<(), UiImageError> {
        self.source.validate()?;
        self.mode.validate_static()?;
        if matches!(self.source, UiAdvancedImageSource::AtlasFrame(_))
            && !matches!(self.mode, UiAdvancedImageMode::Stretch)
        {
            return Err(UiImageError::IncompatibleAtlasMode);
        }
        if let UiAdvancedImageMode::NineSlice(slice) = self.mode {
            slice
                .insets
                .validate(self.source.texture().size.as_vec2())?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiNineSliceLayout {
    pub effective_corner_scale: f32,
    pub estimated_slices: u32,
    pub image_mode: NodeImageMode,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiTilingLayout {
    pub repeats: UVec2,
    pub total_repeats: u32,
    pub image_mode: NodeImageMode,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UiImageFocus {
    pub x: f32,
    pub y: f32,
}

impl UiImageFocus {
    pub(crate) const CENTER: Self = Self::new(0.5, 0.5);
    pub(crate) const TOP_LEFT: Self = Self::new(0.0, 0.0);
    pub(crate) const BOTTOM_RIGHT: Self = Self::new(1.0, 1.0);

    /// Creates a focus in source-image coordinates: (0, 0) is top-left and (1, 1) is bottom-right.
    pub(crate) const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub(crate) fn clamped(self) -> Result<Self, UiImageError> {
        if !self.is_finite() {
            return Err(UiImageError::NonFiniteFocus);
        }
        Ok(Self::new(self.x.clamp(0.0, 1.0), self.y.clamp(0.0, 1.0)))
    }

    const fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Default for UiImageFocus {
    fn default() -> Self {
        Self::CENTER
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum UiImageLength {
    Auto,
    Px(f32),
    Percent(f32),
}

impl UiImageLength {
    const fn to_val(self) -> Val {
        match self {
            Self::Auto => Val::Auto,
            Self::Px(value) => Val::Px(value),
            Self::Percent(value) => Val::Percent(value),
        }
    }

    const fn unit(self) -> Option<UiImageLengthUnit> {
        match self {
            Self::Auto => None,
            Self::Px(_) => Some(UiImageLengthUnit::Px),
            Self::Percent(_) => Some(UiImageLengthUnit::Percent),
        }
    }

    const fn value(self) -> Option<f32> {
        match self {
            Self::Auto => None,
            Self::Px(value) | Self::Percent(value) => Some(value),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UiImageConstraints {
    pub width: UiImageLength,
    pub height: UiImageLength,
    pub aspect_ratio: Option<f32>,
    pub min_width: Option<UiImageLength>,
    pub max_width: Option<UiImageLength>,
    pub min_height: Option<UiImageLength>,
    pub max_height: Option<UiImageLength>,
}

impl UiImageConstraints {
    pub(crate) const fn new(width: UiImageLength, height: UiImageLength) -> Self {
        Self {
            width,
            height,
            aspect_ratio: None,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
        }
    }

    pub(crate) const fn with_aspect_ratio(mut self, aspect_ratio: f32) -> Self {
        self.aspect_ratio = Some(aspect_ratio);
        self
    }

    pub(crate) const fn with_min_width(mut self, min_width: UiImageLength) -> Self {
        self.min_width = Some(min_width);
        self
    }

    pub(crate) const fn with_max_width(mut self, max_width: UiImageLength) -> Self {
        self.max_width = Some(max_width);
        self
    }

    pub(crate) const fn with_min_height(mut self, min_height: UiImageLength) -> Self {
        self.min_height = Some(min_height);
        self
    }

    pub(crate) const fn with_max_height(mut self, max_height: UiImageLength) -> Self {
        self.max_height = Some(max_height);
        self
    }

    pub(crate) fn validate(self) -> Result<(), UiImageError> {
        validate_image_length(self.width, UiImageConstraintField::Width, false, true)?;
        validate_image_length(self.height, UiImageConstraintField::Height, false, true)?;
        validate_optional_limit(self.min_width, UiImageConstraintField::MinWidth, true)?;
        validate_optional_limit(self.max_width, UiImageConstraintField::MaxWidth, false)?;
        validate_optional_limit(self.min_height, UiImageConstraintField::MinHeight, true)?;
        validate_optional_limit(self.max_height, UiImageConstraintField::MaxHeight, false)?;

        if let Some(aspect_ratio) = self.aspect_ratio {
            if !aspect_ratio.is_finite() {
                return Err(UiImageError::NonFiniteConstraint(
                    UiImageConstraintField::AspectRatio,
                ));
            }
            if aspect_ratio <= 0.0 {
                return Err(UiImageError::NonPositiveConstraint(
                    UiImageConstraintField::AspectRatio,
                ));
            }
            if self.width != UiImageLength::Auto && self.height != UiImageLength::Auto {
                return Err(UiImageError::AspectRatioOverconstrained);
            }
        }

        validate_limit_pair(self.min_width, self.max_width, UiImageAxis::Horizontal)?;
        validate_limit_pair(self.min_height, self.max_height, UiImageAxis::Vertical)?;
        Ok(())
    }

    fn to_node(self) -> Node {
        Node {
            width: self.width.to_val(),
            height: self.height.to_val(),
            min_width: self.min_width.map_or(Val::Auto, UiImageLength::to_val),
            max_width: self.max_width.map_or(Val::Auto, UiImageLength::to_val),
            min_height: self.min_height.map_or(Val::Auto, UiImageLength::to_val),
            max_height: self.max_height.map_or(Val::Auto, UiImageLength::to_val),
            aspect_ratio: self.aspect_ratio,
            flex_shrink: 0.0,
            ..default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum UiImageSize {
    FixedBox { width: f32, height: f32 },
    PercentBox { width: f32, height: f32 },
    FullWidthAspect { aspect_ratio: f32 },
    Constrained(UiImageConstraints),
}

impl UiImageSize {
    pub(crate) const fn constrained(constraints: UiImageConstraints) -> Self {
        Self::Constrained(constraints)
    }

    pub(crate) const fn constraints(self) -> UiImageConstraints {
        match self {
            Self::FixedBox { width, height } => {
                UiImageConstraints::new(UiImageLength::Px(width), UiImageLength::Px(height))
            }
            Self::PercentBox { width, height } => UiImageConstraints {
                max_width: Some(UiImageLength::Percent(100.0)),
                ..UiImageConstraints::new(
                    UiImageLength::Percent(width),
                    UiImageLength::Percent(height),
                )
            },
            Self::FullWidthAspect { aspect_ratio } => UiImageConstraints {
                max_width: Some(UiImageLength::Percent(100.0)),
                ..UiImageConstraints::new(UiImageLength::Percent(100.0), UiImageLength::Auto)
                    .with_aspect_ratio(aspect_ratio)
            },
            Self::Constrained(constraints) => constraints,
        }
    }

    pub(crate) fn validate(self) -> Result<(), UiImageError> {
        self.constraints().validate()
    }

    pub(crate) fn try_to_node(self) -> Result<Node, UiImageError> {
        self.validate()?;
        Ok(self.constraints().to_node())
    }

    fn to_node_or_fallback(self) -> (Node, Option<UiImageError>) {
        match self.try_to_node() {
            Ok(node) => (node, None),
            Err(error) => (
                Node {
                    width: px(INVALID_IMAGE_FALLBACK_WIDTH),
                    height: px(INVALID_IMAGE_FALLBACK_HEIGHT),
                    flex_shrink: 0.0,
                    ..default()
                },
                Some(error),
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UiImageLengthUnit {
    Px,
    Percent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UiImageAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UiImageConstraintField {
    Width,
    Height,
    AspectRatio,
    MinWidth,
    MaxWidth,
    MinHeight,
    MaxHeight,
    BorderRadius,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UiImageError {
    NonFiniteConstraint(UiImageConstraintField),
    NonPositiveConstraint(UiImageConstraintField),
    InvalidPercentage(UiImageConstraintField),
    AutoLimit(UiImageConstraintField),
    MixedLimitUnits(UiImageAxis),
    MinExceedsMax(UiImageAxis),
    AspectRatioOverconstrained,
    NonFiniteFocus,
    ZeroSourceSize,
    ZeroContainerSize,
    InvalidSourcePath,
    ZeroDeclaredSourceSize,
    SourceSizeMismatch,
    ZeroFrameSize,
    FrameOutOfBounds,
    OriginalSizeSmallerThanFrame,
    NonFinitePivot,
    PivotOutOfBounds,
    NonFiniteSliceInset,
    NegativeSliceInset,
    SliceInsetsOutOfBounds(UiImageAxis),
    NonFiniteSliceScale,
    InvalidSliceScale,
    NonFiniteCornerScale,
    InvalidCornerScale,
    NonFiniteTileScale,
    InvalidTileScale,
    InvalidRepeatBudget,
    RepeatBudgetExceeded,
    NonFiniteDeviceScale,
    NonPositiveDeviceScale,
    TargetBelowPhysicalPixel(UiImageAxis),
    IncompatibleAtlasMode,
}

impl UiImageError {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::NonFiniteConstraint(_) => "non_finite_constraint",
            Self::NonPositiveConstraint(_) => "non_positive_constraint",
            Self::InvalidPercentage(_) => "invalid_percentage",
            Self::AutoLimit(_) => "auto_limit",
            Self::MixedLimitUnits(_) => "mixed_limit_units",
            Self::MinExceedsMax(_) => "min_exceeds_max",
            Self::AspectRatioOverconstrained => "aspect_ratio_overconstrained",
            Self::NonFiniteFocus => "non_finite_focus",
            Self::ZeroSourceSize => "zero_source_size",
            Self::ZeroContainerSize => "zero_container_size",
            Self::InvalidSourcePath => "invalid_source_path",
            Self::ZeroDeclaredSourceSize => "zero_declared_source_size",
            Self::SourceSizeMismatch => "source_size_mismatch",
            Self::ZeroFrameSize => "zero_frame_size",
            Self::FrameOutOfBounds => "frame_out_of_bounds",
            Self::OriginalSizeSmallerThanFrame => "original_size_smaller_than_frame",
            Self::NonFinitePivot => "non_finite_pivot",
            Self::PivotOutOfBounds => "pivot_out_of_bounds",
            Self::NonFiniteSliceInset => "non_finite_slice_inset",
            Self::NegativeSliceInset => "negative_slice_inset",
            Self::SliceInsetsOutOfBounds(_) => "slice_insets_out_of_bounds",
            Self::NonFiniteSliceScale => "non_finite_slice_scale",
            Self::InvalidSliceScale => "invalid_slice_scale",
            Self::NonFiniteCornerScale => "non_finite_corner_scale",
            Self::InvalidCornerScale => "invalid_corner_scale",
            Self::NonFiniteTileScale => "non_finite_tile_scale",
            Self::InvalidTileScale => "invalid_tile_scale",
            Self::InvalidRepeatBudget => "invalid_repeat_budget",
            Self::RepeatBudgetExceeded => "repeat_budget_exceeded",
            Self::NonFiniteDeviceScale => "non_finite_device_scale",
            Self::NonPositiveDeviceScale => "non_positive_device_scale",
            Self::TargetBelowPhysicalPixel(_) => "target_below_physical_pixel",
            Self::IncompatibleAtlasMode => "incompatible_atlas_mode",
        }
    }
}

impl fmt::Display for UiImageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

#[derive(Clone, Copy, Component, Debug, PartialEq, Eq)]
pub(crate) enum UiImageStatus {
    Loading,
    Ready { source_size: UVec2 },
    Failed,
    Invalid(UiImageError),
}

impl UiImageStatus {
    #[allow(dead_code)]
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Ready { .. } => "ready",
            Self::Failed => "failed",
            Self::Invalid(error) => error.code(),
        }
    }
}

#[derive(Clone, Copy, Component, Debug)]
pub(crate) struct UiImageFrame {
    size: UiImageSize,
    validation_error: Option<UiImageError>,
}

#[derive(Clone, Component, Debug)]
pub(crate) struct UiImageWidget {
    presentation: UiImagePresentation,
    validation_error: Option<UiImageError>,
    ready_tint: Color,
}

#[derive(Clone, Debug)]
enum UiImagePresentation {
    Fit(UiImageFit),
    Advanced(UiAdvancedImageSpec),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiImagePresentationKind {
    Natural,
    Stretch,
    Contain,
    Cover,
    NineSlice,
    Tiled,
    AtlasFrame,
}

impl UiImagePresentationKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Natural => "natural",
            Self::Stretch => "stretch",
            Self::Contain => "contain",
            Self::Cover => "cover",
            Self::NineSlice => "nine_slice",
            Self::Tiled => "tiled",
            Self::AtlasFrame => "atlas_frame",
        }
    }
}

impl UiImageWidget {
    pub(crate) fn presentation_kind(&self) -> UiImagePresentationKind {
        match &self.presentation {
            UiImagePresentation::Fit(UiImageFit::Natural) => UiImagePresentationKind::Natural,
            UiImagePresentation::Fit(UiImageFit::Stretch) => UiImagePresentationKind::Stretch,
            UiImagePresentation::Fit(UiImageFit::Contain) => UiImagePresentationKind::Contain,
            UiImagePresentation::Fit(UiImageFit::Cover { .. }) => UiImagePresentationKind::Cover,
            UiImagePresentation::Advanced(spec)
                if matches!(spec.source, UiAdvancedImageSource::AtlasFrame(_)) =>
            {
                UiImagePresentationKind::AtlasFrame
            }
            UiImagePresentation::Advanced(spec) => match spec.mode {
                UiAdvancedImageMode::Stretch => UiImagePresentationKind::Stretch,
                UiAdvancedImageMode::NineSlice(_) => UiImagePresentationKind::NineSlice,
                UiAdvancedImageMode::Tiled(_) => UiImagePresentationKind::Tiled,
            },
        }
    }
}

#[derive(Bundle)]
pub(crate) struct UiAdvancedImageBundle {
    node: Node,
    image: ImageNode,
    background: BackgroundColor,
    widget: UiImageWidget,
    status: UiImageStatus,
    name: Name,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UiImageFitLayout {
    pub render_size: Vec2,
    pub source_rect: Option<Rect>,
}

#[derive(Clone, Debug)]
struct UiAdvancedImageLayout {
    render_size: Vec2,
    source_rect: Option<Rect>,
    image_mode: NodeImageMode,
}

pub(crate) fn calculate_image_fit(
    fit: UiImageFit,
    source_size: Vec2,
    container_size: Vec2,
) -> Result<UiImageFitLayout, UiImageError> {
    if !source_size.is_finite() || source_size.x <= 0.0 || source_size.y <= 0.0 {
        return Err(UiImageError::ZeroSourceSize);
    }
    if !container_size.is_finite() || container_size.x <= 0.0 || container_size.y <= 0.0 {
        return Err(UiImageError::ZeroContainerSize);
    }
    if let Some(error) = fit.validation_error() {
        return Err(error);
    }

    match fit {
        UiImageFit::Natural => Ok(UiImageFitLayout {
            render_size: source_size,
            source_rect: None,
        }),
        UiImageFit::Stretch => Ok(UiImageFitLayout {
            render_size: container_size,
            source_rect: None,
        }),
        UiImageFit::Contain => {
            let scale = (container_size.x / source_size.x).min(container_size.y / source_size.y);
            Ok(UiImageFitLayout {
                render_size: source_size * scale,
                source_rect: None,
            })
        }
        UiImageFit::Cover { focus } => {
            let focus = focus.clamped()?;
            let source_aspect = source_size.x / source_size.y;
            let container_aspect = container_size.x / container_size.y;
            let crop_size = if source_aspect > container_aspect {
                Vec2::new(source_size.y * container_aspect, source_size.y)
            } else {
                Vec2::new(source_size.x, source_size.x / container_aspect)
            };
            let available_offset = (source_size - crop_size).max(Vec2::ZERO);
            let min = (available_offset * Vec2::new(focus.x, focus.y)).max(Vec2::ZERO);
            let max = (min + crop_size).min(source_size);

            Ok(UiImageFitLayout {
                render_size: container_size,
                source_rect: Some(Rect { min, max }),
            })
        }
    }
}

pub(crate) fn calculate_nine_slice_layout(
    slice: UiNineSlice,
    source_size: Vec2,
    target_size: Vec2,
    device_scale: f32,
) -> Result<UiNineSliceLayout, UiImageError> {
    validate_source_and_target(source_size, target_size)?;
    slice.validate_static()?;
    slice.insets.validate(source_size)?;
    validate_device_scale(device_scale)?;

    let physical_target = target_size * device_scale;
    if physical_target.x < 1.0 {
        return Err(UiImageError::TargetBelowPhysicalPixel(
            UiImageAxis::Horizontal,
        ));
    }
    if physical_target.y < 1.0 {
        return Err(UiImageError::TargetBelowPhysicalPixel(
            UiImageAxis::Vertical,
        ));
    }

    // This matches Bevy 0.18.1's corner rule. Scaling all corners by the smallest
    // source-to-target ratio keeps opposing borders below the target extent.
    let effective_corner_scale = (target_size.x / source_size.x)
        .min(target_size.y / source_size.y)
        .min(slice.max_corner_scale);
    let source_center = Vec2::new(
        source_size.x - slice.insets.left - slice.insets.right,
        source_size.y - slice.insets.top - slice.insets.bottom,
    );
    let target_center = Vec2::new(
        target_size.x - (slice.insets.left + slice.insets.right) * effective_corner_scale,
        target_size.y - (slice.insets.top + slice.insets.bottom) * effective_corner_scale,
    );
    if !target_center.is_finite() || target_center.x < 0.0 || target_center.y < 0.0 {
        return Err(UiImageError::ZeroContainerSize);
    }

    let center_count = match slice.center {
        UiSliceScaleMode::Stretch => 1,
        UiSliceScaleMode::Tile { stretch_value } => {
            repeat_count(target_center.x, source_center.x, stretch_value)?.saturating_mul(
                repeat_count(target_center.y, source_center.y, stretch_value)?,
            )
        }
    };
    let side_count = match slice.sides {
        UiSliceScaleMode::Stretch => 4,
        UiSliceScaleMode::Tile { stretch_value } => {
            let horizontal = repeat_count(target_center.x, source_center.x, stretch_value)?;
            let vertical = repeat_count(target_center.y, source_center.y, stretch_value)?;
            horizontal
                .saturating_mul(2)
                .saturating_add(vertical.saturating_mul(2))
        }
    };
    let estimated_slices = 4_u64
        .saturating_add(center_count)
        .saturating_add(side_count);
    if estimated_slices > u64::from(slice.max_generated_slices) {
        return Err(UiImageError::RepeatBudgetExceeded);
    }

    Ok(UiNineSliceLayout {
        effective_corner_scale,
        estimated_slices: estimated_slices as u32,
        image_mode: NodeImageMode::Sliced(slice.to_texture_slicer()),
    })
}

pub(crate) fn calculate_tiling_layout(
    tiling: UiImageTiling,
    source_size: Vec2,
    target_size: Vec2,
) -> Result<UiTilingLayout, UiImageError> {
    validate_source_and_target(source_size, target_size)?;
    tiling.validate_static()?;
    let (tile_x, tile_y) = tiling.axis.flags();
    let repeat_x = if tile_x {
        repeat_count(target_size.x, source_size.x, tiling.stretch_value)?
    } else {
        1
    };
    let repeat_y = if tile_y {
        repeat_count(target_size.y, source_size.y, tiling.stretch_value)?
    } else {
        1
    };
    let total_repeats = repeat_x.saturating_mul(repeat_y);
    if total_repeats > u64::from(tiling.max_repeats) {
        return Err(UiImageError::RepeatBudgetExceeded);
    }

    Ok(UiTilingLayout {
        repeats: UVec2::new(repeat_x as u32, repeat_y as u32),
        total_repeats: total_repeats as u32,
        image_mode: tiling.to_node_image_mode(),
    })
}

fn validate_source_and_target(source_size: Vec2, target_size: Vec2) -> Result<(), UiImageError> {
    if !source_size.is_finite() || source_size.x <= 0.0 || source_size.y <= 0.0 {
        return Err(UiImageError::ZeroSourceSize);
    }
    if !target_size.is_finite() || target_size.x <= 0.0 || target_size.y <= 0.0 {
        return Err(UiImageError::ZeroContainerSize);
    }
    Ok(())
}

fn validate_device_scale(device_scale: f32) -> Result<(), UiImageError> {
    if !device_scale.is_finite() {
        return Err(UiImageError::NonFiniteDeviceScale);
    }
    if device_scale <= 0.0 {
        return Err(UiImageError::NonPositiveDeviceScale);
    }
    Ok(())
}

fn repeat_count(
    target_extent: f32,
    source_extent: f32,
    stretch_value: f32,
) -> Result<u64, UiImageError> {
    let tile_extent = (source_extent * stretch_value).max(1.0);
    let repeats = (target_extent / tile_extent).ceil();
    if !repeats.is_finite() || repeats > u64::MAX as f32 {
        return Err(UiImageError::RepeatBudgetExceeded);
    }
    Ok(repeats.max(1.0) as u64)
}

fn resolve_advanced_image_layout(
    spec: &UiAdvancedImageSpec,
    actual_source_size: UVec2,
    target_size: Vec2,
    device_scale: f32,
) -> Result<UiAdvancedImageLayout, UiImageError> {
    spec.validate()?;
    if actual_source_size != spec.source.texture().size.as_uvec2() {
        return Err(UiImageError::SourceSizeMismatch);
    }
    let source_size = actual_source_size.as_vec2();
    let image_mode = match spec.mode {
        UiAdvancedImageMode::Stretch => NodeImageMode::Stretch,
        UiAdvancedImageMode::NineSlice(slice) => {
            calculate_nine_slice_layout(slice, source_size, target_size, device_scale)?.image_mode
        }
        UiAdvancedImageMode::Tiled(tiling) => {
            calculate_tiling_layout(tiling, source_size, target_size)?.image_mode
        }
    };

    Ok(UiAdvancedImageLayout {
        render_size: target_size,
        source_rect: spec.source.rect(),
        image_mode,
    })
}

pub(crate) fn ui_image(
    image: Handle<Image>,
    fit: UiImageFit,
    size: UiImageSize,
) -> (
    Node,
    ImageNode,
    BackgroundColor,
    UiImageWidget,
    UiImageStatus,
    Name,
) {
    let (mut node, size_error) = size.to_node_or_fallback();
    node.align_self = fit.to_align_self();
    let validation_error = size_error.or_else(|| fit.validation_error());
    let status = validation_error.map_or(UiImageStatus::Loading, UiImageStatus::Invalid);

    (
        node,
        ImageNode::new(image).with_mode(fit.to_node_image_mode()),
        BackgroundColor(loading_placeholder_color()),
        UiImageWidget {
            presentation: UiImagePresentation::Fit(fit),
            validation_error,
            ready_tint: Color::WHITE,
        },
        status,
        Name::new("UI image"),
    )
}

pub(crate) fn try_ui_advanced_image(
    asset_server: &AssetServer,
    spec: UiAdvancedImageSpec,
    size: UiImageSize,
) -> Result<UiAdvancedImageBundle, UiImageError> {
    spec.validate()?;
    let mut node = size.try_to_node()?;
    node.align_self = AlignSelf::Stretch;
    let source_rect = spec.source.rect();
    let image_mode = spec.mode.initial_node_image_mode();
    let image: Handle<Image> = asset_server.load(spec.source.texture().path.clone());
    let mut image_node = ImageNode::new(image).with_mode(image_mode);
    if let Some(source_rect) = source_rect {
        image_node = image_node.with_rect(source_rect);
    }

    Ok(UiAdvancedImageBundle {
        node,
        image: image_node,
        background: BackgroundColor(loading_placeholder_color()),
        widget: UiImageWidget {
            presentation: UiImagePresentation::Advanced(spec),
            validation_error: None,
            ready_tint: Color::WHITE,
        },
        status: UiImageStatus::Loading,
        name: Name::new("UI advanced image"),
    })
}

pub(crate) fn ui_image_panel_node(size: UiImageSize) -> (Node, UiImageFrame, Name) {
    ui_image_panel_node_with_radius(size, 0.0)
}

pub(crate) fn ui_image_panel_node_with_radius(
    size: UiImageSize,
    radius: f32,
) -> (Node, UiImageFrame, Name) {
    let (mut node, mut validation_error) = size.to_node_or_fallback();
    if !radius.is_finite() || radius < 0.0 {
        validation_error = Some(if radius.is_finite() {
            UiImageError::NonPositiveConstraint(UiImageConstraintField::BorderRadius)
        } else {
            UiImageError::NonFiniteConstraint(UiImageConstraintField::BorderRadius)
        });
    }
    node.overflow = Overflow::clip();
    node.align_items = AlignItems::Center;
    node.justify_content = JustifyContent::Center;
    node.border_radius = BorderRadius::all(px(if validation_error.is_some() {
        0.0
    } else {
        radius
    }));

    (
        node,
        UiImageFrame {
            size,
            validation_error,
        },
        Name::new("UI image frame"),
    )
}

pub(crate) fn ui_thumbnail_grid(columns: u16, gap: f32) -> impl Bundle {
    Node {
        width: percent(100),
        display: Display::Grid,
        grid_template_columns: RepeatedGridTrack::flex(columns.max(1), 1.0),
        grid_auto_rows: vec![GridTrack::auto()],
        column_gap: px(gap),
        row_gap: px(gap),
        align_items: AlignItems::Center,
        justify_items: JustifyItems::Stretch,
        ..default()
    }
}

pub(crate) fn update_ui_images(
    asset_server: Res<AssetServer>,
    images: Res<Assets<Image>>,
    frames: Query<(&UiImageFrame, &ComputedNode)>,
    mut image_nodes: Query<(
        &UiImageWidget,
        &mut UiImageStatus,
        &mut Node,
        &ComputedNode,
        &mut ImageNode,
        &mut BackgroundColor,
        Option<&ChildOf>,
    )>,
) {
    for (widget, mut status, mut node, computed, mut image_node, mut background, parent) in
        &mut image_nodes
    {
        let mut next_node = (*node).clone();
        let mut next_image_node = (*image_node).clone();
        let mut next_background = *background;
        let (container_size, frame_error, inverse_scale_factor) = parent
            .and_then(|parent| frames.get(parent.parent()).ok())
            .map(|(frame, computed)| {
                let logical_size = computed.content_box().size() * computed.inverse_scale_factor;
                (
                    logical_size,
                    frame
                        .validation_error
                        .or_else(|| frame.size.validate().err()),
                    computed.inverse_scale_factor,
                )
            })
            .unwrap_or_else(|| {
                (
                    computed.size() * computed.inverse_scale_factor,
                    widget.validation_error,
                    computed.inverse_scale_factor,
                )
            });

        let next_status = if let Some(error) = frame_error.or(widget.validation_error) {
            apply_image_placeholder(
                &mut next_node,
                &mut next_image_node,
                &mut next_background,
                invalid_placeholder_color(),
                widget.ready_tint,
            );
            UiImageStatus::Invalid(error)
        } else if !container_size.is_finite() || container_size.x <= 0.0 || container_size.y <= 0.0
        {
            apply_image_placeholder(
                &mut next_node,
                &mut next_image_node,
                &mut next_background,
                invalid_placeholder_color(),
                widget.ready_tint,
            );
            UiImageStatus::Invalid(UiImageError::ZeroContainerSize)
        } else {
            let image_id = next_image_node.image.id();
            if let Some(image) = images.get(image_id) {
                let source_size = image.size();
                let resolved = match &widget.presentation {
                    UiImagePresentation::Fit(fit) => {
                        calculate_image_fit(*fit, source_size.as_vec2(), container_size).map(
                            |layout| {
                                apply_ready_image(
                                    &mut next_node,
                                    &mut next_image_node,
                                    &mut next_background,
                                    widget.ready_tint,
                                    layout,
                                );
                            },
                        )
                    }
                    UiImagePresentation::Advanced(spec) => resolve_advanced_image_layout(
                        spec,
                        source_size,
                        container_size,
                        inverse_scale_factor.recip(),
                    )
                    .map(|layout| {
                        apply_ready_advanced_image(
                            &mut next_node,
                            &mut next_image_node,
                            &mut next_background,
                            widget.ready_tint,
                            layout,
                        );
                    }),
                };
                match resolved {
                    Ok(()) => UiImageStatus::Ready { source_size },
                    Err(error) => {
                        apply_image_placeholder(
                            &mut next_node,
                            &mut next_image_node,
                            &mut next_background,
                            invalid_placeholder_color(),
                            widget.ready_tint,
                        );
                        UiImageStatus::Invalid(error)
                    }
                }
            } else {
                let failed = matches!(
                    asset_server.get_load_state(image_id),
                    Some(LoadState::Failed(_))
                );
                apply_image_placeholder(
                    &mut next_node,
                    &mut next_image_node,
                    &mut next_background,
                    if failed {
                        failed_placeholder_color()
                    } else {
                        loading_placeholder_color()
                    },
                    widget.ready_tint,
                );
                if failed {
                    UiImageStatus::Failed
                } else {
                    UiImageStatus::Loading
                }
            }
        };
        commit_image_components(
            &mut node,
            &mut image_node,
            &mut background,
            &mut status,
            next_node,
            next_image_node,
            next_background,
            next_status,
        );
    }
}

fn commit_image_components(
    node: &mut Mut<'_, Node>,
    image_node: &mut Mut<'_, ImageNode>,
    background: &mut Mut<'_, BackgroundColor>,
    status: &mut Mut<'_, UiImageStatus>,
    next_node: Node,
    next_image_node: ImageNode,
    next_background: BackgroundColor,
    next_status: UiImageStatus,
) {
    node.set_if_neq(next_node);
    if image_node.rect != next_image_node.rect
        || image_node.image_mode != next_image_node.image_mode
        || image_node.color != next_image_node.color
    {
        let image_node = &mut **image_node;
        image_node.rect = next_image_node.rect;
        image_node.image_mode = next_image_node.image_mode;
        image_node.color = next_image_node.color;
    }
    background.set_if_neq(next_background);
    status.set_if_neq(next_status);
}

fn apply_ready_image(
    node: &mut Node,
    image_node: &mut ImageNode,
    background: &mut BackgroundColor,
    tint: Color,
    layout: UiImageFitLayout,
) {
    set_child_render_size(node, layout.render_size);
    if image_node.rect != layout.source_rect {
        image_node.rect = layout.source_rect;
    }
    if image_node.image_mode != NodeImageMode::Stretch {
        image_node.image_mode = NodeImageMode::Stretch;
    }
    if image_node.color != tint {
        image_node.color = tint;
    }
    *background = BackgroundColor(Color::NONE);
}

fn apply_ready_advanced_image(
    node: &mut Node,
    image_node: &mut ImageNode,
    background: &mut BackgroundColor,
    tint: Color,
    layout: UiAdvancedImageLayout,
) {
    set_child_render_size(node, layout.render_size);
    image_node.rect = layout.source_rect;
    image_node.image_mode = layout.image_mode;
    image_node.color = tint;
    *background = BackgroundColor(Color::NONE);
}

fn apply_image_placeholder(
    node: &mut Node,
    image_node: &mut ImageNode,
    background: &mut BackgroundColor,
    placeholder_color: Color,
    ready_tint: Color,
) {
    node.width = percent(100.0);
    node.height = percent(100.0);
    clear_child_constraints(node);
    image_node.rect = None;
    image_node.image_mode = NodeImageMode::Stretch;
    image_node.color = ready_tint.with_alpha(0.0);
    *background = BackgroundColor(placeholder_color);
}

fn set_child_render_size(node: &mut Node, render_size: Vec2) {
    node.width = px(render_size.x);
    node.height = px(render_size.y);
    clear_child_constraints(node);
}

fn clear_child_constraints(node: &mut Node) {
    node.min_width = Val::Auto;
    node.max_width = Val::Auto;
    node.min_height = Val::Auto;
    node.max_height = Val::Auto;
    node.aspect_ratio = None;
    node.flex_shrink = 0.0;
}

fn loading_placeholder_color() -> Color {
    Color::srgba(0.28, 0.30, 0.34, 0.72)
}

fn failed_placeholder_color() -> Color {
    Color::srgba(0.55, 0.12, 0.14, 0.88)
}

fn invalid_placeholder_color() -> Color {
    Color::srgba(0.72, 0.34, 0.06, 0.88)
}

fn validate_optional_limit(
    value: Option<UiImageLength>,
    field: UiImageConstraintField,
    allow_zero: bool,
) -> Result<(), UiImageError> {
    if let Some(value) = value {
        validate_image_length(value, field, allow_zero, false)?;
    }
    Ok(())
}

fn validate_image_length(
    length: UiImageLength,
    field: UiImageConstraintField,
    allow_zero: bool,
    allow_auto: bool,
) -> Result<(), UiImageError> {
    let Some(value) = length.value() else {
        return if allow_auto {
            Ok(())
        } else {
            Err(UiImageError::AutoLimit(field))
        };
    };
    if !value.is_finite() {
        return Err(UiImageError::NonFiniteConstraint(field));
    }
    if value < 0.0 || (!allow_zero && value == 0.0) {
        return Err(UiImageError::NonPositiveConstraint(field));
    }
    if matches!(length, UiImageLength::Percent(_)) && value > 100.0 {
        return Err(UiImageError::InvalidPercentage(field));
    }
    Ok(())
}

fn validate_limit_pair(
    min: Option<UiImageLength>,
    max: Option<UiImageLength>,
    axis: UiImageAxis,
) -> Result<(), UiImageError> {
    let (Some(min), Some(max)) = (min, max) else {
        return Ok(());
    };
    if min.unit() != max.unit() {
        return Err(UiImageError::MixedLimitUnits(axis));
    }
    if min.value().expect("validated min has a value")
        > max.value().expect("validated max has a value")
    {
        return Err(UiImageError::MinExceedsMax(axis));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        asset::AssetPlugin,
        render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    };

    fn assert_vec2_close(actual: Vec2, expected: Vec2) {
        assert!((actual - expected).abs().max_element() < 0.001);
    }

    fn assert_rect_close(actual: Rect, expected: Rect) {
        assert_vec2_close(actual.min, expected.min);
        assert_vec2_close(actual.max, expected.max);
    }

    fn image_runtime_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>();
        app.finish();
        app.cleanup();
        app
    }

    fn test_image(width: u32, height: u32) -> Image {
        Image::new_fill(
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[255, 255, 255, 255],
            TextureFormat::Rgba8UnormSrgb,
            default(),
        )
    }

    fn computed_node(size: Vec2) -> ComputedNode {
        ComputedNode {
            size,
            unrounded_size: size,
            ..default()
        }
    }

    fn spawn_runtime_image(app: &mut App, image: Handle<Image>, fit: UiImageFit) -> Entity {
        let frame = app
            .world_mut()
            .spawn(ui_image_panel_node(UiImageSize::FixedBox {
                width: 100.0,
                height: 100.0,
            }))
            .insert(computed_node(Vec2::splat(100.0)))
            .id();
        let image = app
            .world_mut()
            .spawn(ui_image(
                image,
                fit,
                UiImageSize::PercentBox {
                    width: 100.0,
                    height: 100.0,
                },
            ))
            .insert(computed_node(Vec2::splat(100.0)))
            .id();
        app.world_mut().entity_mut(frame).add_child(image);
        image
    }

    fn texture_source(path: &str, width: u32, height: u32) -> UiImageTextureSource {
        UiImageTextureSource::new(path, UiImagePixelSize::new(width, height))
    }

    fn nine_slice_spec(path: &str) -> UiAdvancedImageSpec {
        UiAdvancedImageSpec {
            source: UiAdvancedImageSource::Texture(texture_source(path, 48, 48)),
            mode: UiAdvancedImageMode::NineSlice(UiNineSlice::uniform(12.0)),
        }
    }

    fn spawn_runtime_advanced_image(
        app: &mut App,
        image: Image,
        spec: UiAdvancedImageSpec,
    ) -> Entity {
        let frame = app
            .world_mut()
            .spawn(ui_image_panel_node(UiImageSize::FixedBox {
                width: 120.0,
                height: 72.0,
            }))
            .insert(computed_node(Vec2::new(120.0, 72.0)))
            .id();
        let bundle = try_ui_advanced_image(
            app.world().resource::<AssetServer>(),
            spec,
            UiImageSize::PercentBox {
                width: 100.0,
                height: 100.0,
            },
        )
        .unwrap();
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .insert(bundle.image.image.id(), image)
            .unwrap();
        let image = app
            .world_mut()
            .spawn(bundle)
            .insert(computed_node(Vec2::new(120.0, 72.0)))
            .id();
        app.world_mut().entity_mut(frame).add_child(image);
        image
    }

    fn assert_image_component_changes(world: &World, entity: Entity, expected: bool) {
        let entity = world.entity(entity);
        assert_eq!(entity.get_ref::<Node>().unwrap().is_changed(), expected);
        assert_eq!(
            entity.get_ref::<ImageNode>().unwrap().is_changed(),
            expected
        );
        assert_eq!(
            entity.get_ref::<BackgroundColor>().unwrap().is_changed(),
            expected
        );
        assert_eq!(
            entity.get_ref::<UiImageStatus>().unwrap().is_changed(),
            expected
        );
    }

    #[test]
    fn image_fit_maps_to_initial_node_image_mode() {
        assert_eq!(
            UiImageFit::Natural.to_node_image_mode(),
            NodeImageMode::Auto
        );
        assert_eq!(
            UiImageFit::Stretch.to_node_image_mode(),
            NodeImageMode::Stretch
        );
        assert_eq!(
            UiImageFit::Contain.to_node_image_mode(),
            NodeImageMode::Stretch
        );
        assert_eq!(
            UiImageFit::cover(UiImageFocus::CENTER).to_node_image_mode(),
            NodeImageMode::Stretch
        );
    }

    #[test]
    fn image_size_maps_fixed_percent_and_aspect_dimensions() {
        let fixed = UiImageSize::FixedBox {
            width: 64.0,
            height: 48.0,
        }
        .try_to_node()
        .unwrap();
        assert_eq!(fixed.width, px(64));
        assert_eq!(fixed.height, px(48));

        let percent_box = UiImageSize::PercentBox {
            width: 50.0,
            height: 100.0,
        }
        .try_to_node()
        .unwrap();
        assert_eq!(percent_box.width, percent(50));
        assert_eq!(percent_box.height, percent(100));
        assert_eq!(percent_box.max_width, percent(100));

        let aspect = UiImageSize::FullWidthAspect { aspect_ratio: 1.5 }
            .try_to_node()
            .unwrap();
        assert_eq!(aspect.width, percent(100));
        assert_eq!(aspect.height, Val::Auto);
        assert_eq!(aspect.max_width, percent(100));
        assert_eq!(aspect.aspect_ratio, Some(1.5));
    }

    #[test]
    fn constrained_size_maps_min_max_and_aspect() {
        let node = UiImageSize::constrained(
            UiImageConstraints::new(UiImageLength::Percent(75.0), UiImageLength::Auto)
                .with_aspect_ratio(16.0 / 9.0)
                .with_min_width(UiImageLength::Px(120.0))
                .with_max_width(UiImageLength::Px(640.0))
                .with_min_height(UiImageLength::Percent(10.0))
                .with_max_height(UiImageLength::Percent(90.0)),
        )
        .try_to_node()
        .unwrap();

        assert_eq!(node.width, percent(75));
        assert_eq!(node.height, Val::Auto);
        assert_eq!(node.aspect_ratio, Some(16.0 / 9.0));
        assert_eq!(node.min_width, px(120));
        assert_eq!(node.max_width, px(640));
        assert_eq!(node.min_height, percent(10));
        assert_eq!(node.max_height, percent(90));
    }

    #[test]
    fn size_validation_rejects_non_finite_zero_and_invalid_percentages() {
        assert_eq!(
            UiImageSize::FixedBox {
                width: f32::NAN,
                height: 20.0
            }
            .validate(),
            Err(UiImageError::NonFiniteConstraint(
                UiImageConstraintField::Width
            ))
        );
        assert_eq!(
            UiImageSize::FixedBox {
                width: 20.0,
                height: 0.0
            }
            .validate(),
            Err(UiImageError::NonPositiveConstraint(
                UiImageConstraintField::Height
            ))
        );
        assert_eq!(
            UiImageSize::PercentBox {
                width: 101.0,
                height: 50.0
            }
            .validate(),
            Err(UiImageError::InvalidPercentage(
                UiImageConstraintField::Width
            ))
        );
        assert_eq!(
            UiImageSize::FullWidthAspect {
                aspect_ratio: f32::INFINITY
            }
            .validate(),
            Err(UiImageError::NonFiniteConstraint(
                UiImageConstraintField::AspectRatio
            ))
        );
        assert_eq!(
            UiImageSize::FullWidthAspect { aspect_ratio: 0.0 }.validate(),
            Err(UiImageError::NonPositiveConstraint(
                UiImageConstraintField::AspectRatio
            ))
        );
    }

    #[test]
    fn size_validation_rejects_contradictory_and_ambiguous_constraints() {
        let min_over_max = UiImageSize::constrained(
            UiImageConstraints::new(UiImageLength::Percent(100.0), UiImageLength::Auto)
                .with_min_width(UiImageLength::Px(300.0))
                .with_max_width(UiImageLength::Px(200.0)),
        );
        assert_eq!(
            min_over_max.validate(),
            Err(UiImageError::MinExceedsMax(UiImageAxis::Horizontal))
        );

        let mixed_limits = UiImageSize::constrained(
            UiImageConstraints::new(UiImageLength::Percent(100.0), UiImageLength::Auto)
                .with_min_width(UiImageLength::Px(100.0))
                .with_max_width(UiImageLength::Percent(80.0)),
        );
        assert_eq!(
            mixed_limits.validate(),
            Err(UiImageError::MixedLimitUnits(UiImageAxis::Horizontal))
        );

        let overconstrained = UiImageSize::constrained(
            UiImageConstraints::new(UiImageLength::Px(100.0), UiImageLength::Px(80.0))
                .with_aspect_ratio(1.0),
        );
        assert_eq!(
            overconstrained.validate(),
            Err(UiImageError::AspectRatioOverconstrained)
        );
    }

    #[test]
    fn natural_stretch_and_contain_follow_declared_size_rules() {
        let source = Vec2::new(200.0, 100.0);
        let container = Vec2::splat(100.0);

        let natural = calculate_image_fit(UiImageFit::Natural, source, container).unwrap();
        assert_vec2_close(natural.render_size, source);
        assert_eq!(natural.source_rect, None);

        let stretch = calculate_image_fit(UiImageFit::Stretch, source, container).unwrap();
        assert_vec2_close(stretch.render_size, container);
        assert_eq!(stretch.source_rect, None);

        let contain = calculate_image_fit(UiImageFit::Contain, source, container).unwrap();
        assert_vec2_close(contain.render_size, Vec2::new(100.0, 50.0));
        assert_eq!(contain.source_rect, None);
    }

    #[test]
    fn cover_crops_landscape_source_without_exceeding_source_bounds() {
        let layout = calculate_image_fit(
            UiImageFit::cover(UiImageFocus::CENTER),
            Vec2::new(200.0, 100.0),
            Vec2::splat(100.0),
        )
        .unwrap();

        assert_vec2_close(layout.render_size, Vec2::splat(100.0));
        assert_rect_close(
            layout.source_rect.unwrap(),
            Rect::from_corners(Vec2::new(50.0, 0.0), Vec2::new(150.0, 100.0)),
        );
    }

    #[test]
    fn cover_crops_portrait_source_without_exceeding_source_bounds() {
        let layout = calculate_image_fit(
            UiImageFit::cover(UiImageFocus::CENTER),
            Vec2::new(100.0, 200.0),
            Vec2::new(200.0, 100.0),
        )
        .unwrap();

        assert_rect_close(
            layout.source_rect.unwrap(),
            Rect::from_corners(Vec2::new(0.0, 75.0), Vec2::new(100.0, 125.0)),
        );
    }

    #[test]
    fn cover_focus_is_clamped_in_top_left_source_coordinates() {
        let left = calculate_image_fit(
            UiImageFit::cover(UiImageFocus::new(-2.0, 0.5)),
            Vec2::new(200.0, 100.0),
            Vec2::splat(100.0),
        )
        .unwrap();
        let right = calculate_image_fit(
            UiImageFit::cover(UiImageFocus::new(2.0, 0.5)),
            Vec2::new(200.0, 100.0),
            Vec2::splat(100.0),
        )
        .unwrap();

        assert_rect_close(
            left.source_rect.unwrap(),
            Rect::from_corners(Vec2::ZERO, Vec2::splat(100.0)),
        );
        assert_rect_close(
            right.source_rect.unwrap(),
            Rect::from_corners(Vec2::new(100.0, 0.0), Vec2::new(200.0, 100.0)),
        );
    }

    #[test]
    fn cover_vertical_focus_is_clamped_for_portrait_sources() {
        let top = calculate_image_fit(
            UiImageFit::cover(UiImageFocus::new(0.5, -2.0)),
            Vec2::new(100.0, 200.0),
            Vec2::new(200.0, 100.0),
        )
        .unwrap();
        let bottom = calculate_image_fit(
            UiImageFit::cover(UiImageFocus::new(0.5, 2.0)),
            Vec2::new(100.0, 200.0),
            Vec2::new(200.0, 100.0),
        )
        .unwrap();

        assert_rect_close(
            top.source_rect.unwrap(),
            Rect::from_corners(Vec2::ZERO, Vec2::new(100.0, 50.0)),
        );
        assert_rect_close(
            bottom.source_rect.unwrap(),
            Rect::from_corners(Vec2::new(0.0, 150.0), Vec2::new(100.0, 200.0)),
        );
    }

    #[test]
    fn fit_rejects_non_finite_focus_and_zero_sizes() {
        assert_eq!(
            calculate_image_fit(
                UiImageFit::cover(UiImageFocus::new(f32::NAN, 0.5)),
                Vec2::splat(100.0),
                Vec2::splat(100.0),
            ),
            Err(UiImageError::NonFiniteFocus)
        );
        assert_eq!(
            calculate_image_fit(UiImageFit::Contain, Vec2::ZERO, Vec2::splat(100.0)),
            Err(UiImageError::ZeroSourceSize)
        );
        assert_eq!(
            calculate_image_fit(UiImageFit::Contain, Vec2::splat(100.0), Vec2::ZERO),
            Err(UiImageError::ZeroContainerSize)
        );
    }

    #[test]
    fn advanced_image_models_round_trip_through_ron() {
        let specs = vec![
            UiAdvancedImageSpec {
                source: UiAdvancedImageSource::AtlasFrame(UiAtlasFrame {
                    source: texture_source("ui/fixtures/atlas.png", 128, 32),
                    rect: UiImagePixelRect::new(32, 0, 32, 32),
                    original_size: UiImagePixelSize::new(40, 36),
                    pivot: Some(UiImagePivot::new(0.5, 0.75)),
                }),
                mode: UiAdvancedImageMode::Stretch,
            },
            nine_slice_spec("ui/fixtures/nine-slice.png"),
            UiAdvancedImageSpec {
                source: UiAdvancedImageSource::Texture(texture_source(
                    "ui/fixtures/tile.png",
                    32,
                    24,
                )),
                mode: UiAdvancedImageMode::Tiled(UiImageTiling::new(UiTileAxis::Both)),
            },
        ];

        let encoded = ron::ser::to_string(&specs).unwrap();
        let decoded: Vec<UiAdvancedImageSpec> = ron::de::from_str(&encoded).unwrap();

        assert_eq!(decoded, specs);
        assert!(decoded.iter().all(|spec| spec.validate().is_ok()));
    }

    #[test]
    fn nine_slice_maps_to_bevy_and_scales_down_without_flipping() {
        let slice = UiNineSlice::uniform(12.0);
        let layout =
            calculate_nine_slice_layout(slice, Vec2::splat(48.0), Vec2::new(10.5, 7.25), 3.25)
                .unwrap();

        assert!((layout.effective_corner_scale - (7.25 / 48.0)).abs() < 0.001);
        assert_eq!(layout.estimated_slices, 9);
        let NodeImageMode::Sliced(slicer) = layout.image_mode else {
            panic!("nine-slice should resolve to NodeImageMode::Sliced");
        };
        assert_eq!(slicer.border.min_inset, Vec2::splat(12.0));
        assert_eq!(slicer.border.max_inset, Vec2::splat(12.0));
        assert_eq!(slicer.center_scale_mode, SliceScaleMode::Stretch);
        assert_eq!(slicer.sides_scale_mode, SliceScaleMode::Stretch);
    }

    #[test]
    fn nine_slice_enforces_physical_pixel_and_source_bounds() {
        let slice = UiNineSlice::uniform(12.0);
        assert_eq!(
            calculate_nine_slice_layout(slice, Vec2::splat(48.0), Vec2::new(0.30, 20.0), 3.25,),
            Err(UiImageError::TargetBelowPhysicalPixel(
                UiImageAxis::Horizontal
            ))
        );

        let mut out_of_bounds = slice;
        out_of_bounds.insets.right = 36.0;
        assert_eq!(
            calculate_nine_slice_layout(out_of_bounds, Vec2::splat(48.0), Vec2::splat(80.0), 1.0,),
            Err(UiImageError::SliceInsetsOutOfBounds(
                UiImageAxis::Horizontal
            ))
        );

        let mut non_finite = slice;
        non_finite.insets.top = f32::NAN;
        assert_eq!(
            calculate_nine_slice_layout(non_finite, Vec2::splat(48.0), Vec2::splat(80.0), 1.0,),
            Err(UiImageError::NonFiniteSliceInset)
        );

        let mut negative = slice;
        negative.insets.left = -1.0;
        assert_eq!(
            calculate_nine_slice_layout(negative, Vec2::splat(48.0), Vec2::splat(80.0), 1.0,),
            Err(UiImageError::NegativeSliceInset)
        );
    }

    #[test]
    fn nine_slice_tiling_rejects_excessive_generated_slices() {
        let mut slice = UiNineSlice::uniform(4.0);
        slice.center = UiSliceScaleMode::Tile {
            stretch_value: 0.25,
        };
        slice.sides = UiSliceScaleMode::Tile {
            stretch_value: 0.25,
        };
        slice.max_generated_slices = 16;

        assert_eq!(
            calculate_nine_slice_layout(slice, Vec2::splat(48.0), Vec2::splat(500.0), 1.0,),
            Err(UiImageError::RepeatBudgetExceeded)
        );
    }

    #[test]
    fn tiled_modes_map_axes_and_enforce_repeat_budget() {
        let cases = [
            (UiTileAxis::X, UVec2::new(4, 1)),
            (UiTileAxis::Y, UVec2::new(1, 3)),
            (UiTileAxis::Both, UVec2::new(4, 3)),
        ];
        for (axis, repeats) in cases {
            let layout = calculate_tiling_layout(
                UiImageTiling {
                    axis,
                    stretch_value: 1.0,
                    max_repeats: 16,
                },
                Vec2::new(32.0, 24.0),
                Vec2::new(100.0, 50.0),
            )
            .unwrap();
            assert_eq!(layout.repeats, repeats);
            let NodeImageMode::Tiled { tile_x, tile_y, .. } = layout.image_mode else {
                panic!("tiling should resolve to NodeImageMode::Tiled");
            };
            assert_eq!((tile_x, tile_y), axis.flags());
        }

        assert_eq!(
            calculate_tiling_layout(
                UiImageTiling {
                    axis: UiTileAxis::Both,
                    stretch_value: 0.5,
                    max_repeats: 8,
                },
                Vec2::splat(8.0),
                Vec2::splat(100.0),
            ),
            Err(UiImageError::RepeatBudgetExceeded)
        );
    }

    #[test]
    fn tile_and_slice_scale_validation_rejects_bevy_clamp_inputs() {
        assert_eq!(
            UiImageTiling {
                axis: UiTileAxis::X,
                stretch_value: 0.0001,
                max_repeats: 10,
            }
            .validate_static(),
            Err(UiImageError::InvalidTileScale)
        );
        assert_eq!(
            UiSliceScaleMode::Tile {
                stretch_value: f32::INFINITY,
            }
            .validate(),
            Err(UiImageError::NonFiniteSliceScale)
        );
        assert_eq!(
            UiImageTiling {
                axis: UiTileAxis::Both,
                stretch_value: 1.0,
                max_repeats: 0,
            }
            .validate_static(),
            Err(UiImageError::InvalidRepeatBudget)
        );
    }

    #[test]
    fn atlas_frame_validates_bounds_original_size_pivot_and_path() {
        let valid = UiAtlasFrame {
            source: texture_source("ui/fixtures/atlas.png", 128, 32),
            rect: UiImagePixelRect::new(96, 0, 32, 32),
            original_size: UiImagePixelSize::new(32, 32),
            pivot: Some(UiImagePivot::new(0.5, 0.5)),
        };
        assert_eq!(valid.validate(), Ok(()));

        let mut frame = valid.clone();
        frame.rect.x = 97;
        assert_eq!(frame.validate(), Err(UiImageError::FrameOutOfBounds));

        let mut frame = valid.clone();
        frame.original_size.width = 31;
        assert_eq!(
            frame.validate(),
            Err(UiImageError::OriginalSizeSmallerThanFrame)
        );

        let mut frame = valid.clone();
        frame.pivot = Some(UiImagePivot::new(f32::NAN, 0.5));
        assert_eq!(frame.validate(), Err(UiImageError::NonFinitePivot));

        let mut frame = valid;
        frame.source.path = "../atlas.png".to_owned();
        assert_eq!(frame.validate(), Err(UiImageError::InvalidSourcePath));
    }

    #[test]
    fn texture_source_rejects_programmatic_and_unsafe_windows_paths() {
        for path in [
            "",
            "C:foo.png",
            "C:/foo.png",
            " ui/foo.png",
            "/ui/foo.png",
            "ui/./foo.png",
            "../ui/foo.png",
            r"ui\foo.png",
            r"\\server\share\foo.png",
        ] {
            assert_eq!(
                texture_source(path, 32, 32).validate(),
                Err(UiImageError::InvalidSourcePath),
                "path should be rejected: {path:?}"
            );
        }
        assert_eq!(texture_source("ui/foo.png", 32, 32).validate(), Ok(()));
    }

    #[test]
    fn advanced_builder_rejects_atlas_slice_before_loading_or_bundle_creation() {
        let app = image_runtime_test_app();
        let asset_server = app.world().resource::<AssetServer>();
        let path = "ui/fixtures/atlas-invalid-combination.png";
        let spec = UiAdvancedImageSpec {
            source: UiAdvancedImageSource::AtlasFrame(UiAtlasFrame {
                source: texture_source(path, 128, 32),
                rect: UiImagePixelRect::new(0, 0, 32, 32),
                original_size: UiImagePixelSize::new(32, 32),
                pivot: None,
            }),
            mode: UiAdvancedImageMode::NineSlice(UiNineSlice::uniform(4.0)),
        };

        assert!(asset_server.get_handle::<Image>(path).is_none());
        let result = try_ui_advanced_image(
            asset_server,
            spec,
            UiImageSize::FixedBox {
                width: 64.0,
                height: 64.0,
            },
        );
        assert!(matches!(result, Err(UiImageError::IncompatibleAtlasMode)));
        assert!(asset_server.get_handle::<Image>(path).is_none());
    }

    #[test]
    fn advanced_builder_uses_spec_path_and_cannot_accept_same_sized_wrong_texture() {
        let mut app = image_runtime_test_app();
        let declared_path = "ui/fixtures/atlas-declared.png";
        let wrong_path = "ui/fixtures/atlas-wrong-same-size.png";
        let wrong_handle: Handle<Image> = app.world().resource::<AssetServer>().load(wrong_path);
        let spec = UiAdvancedImageSpec {
            source: UiAdvancedImageSource::AtlasFrame(UiAtlasFrame {
                source: texture_source(declared_path, 128, 32),
                rect: UiImagePixelRect::new(32, 0, 32, 32),
                original_size: UiImagePixelSize::new(32, 32),
                pivot: None,
            }),
            mode: UiAdvancedImageMode::Stretch,
        };

        let bundle = try_ui_advanced_image(
            app.world().resource::<AssetServer>(),
            spec,
            UiImageSize::FixedBox {
                width: 32.0,
                height: 32.0,
            },
        )
        .unwrap();
        let selected_handle = bundle.image.image.clone();

        assert_ne!(selected_handle.id(), wrong_handle.id());
        assert_eq!(
            app.world()
                .resource::<AssetServer>()
                .get_path(selected_handle.id())
                .unwrap()
                .to_string(),
            declared_path
        );

        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        images
            .insert(wrong_handle.id(), test_image(128, 32))
            .unwrap();
        images
            .insert(selected_handle.id(), test_image(128, 32))
            .unwrap();
        assert_eq!(
            images.get(wrong_handle.id()).unwrap().size(),
            images.get(selected_handle.id()).unwrap().size()
        );
    }

    #[test]
    fn advanced_runtime_applies_slice_and_stays_change_stable() {
        let mut app = image_runtime_test_app();
        let entity = spawn_runtime_advanced_image(
            &mut app,
            test_image(48, 48),
            nine_slice_spec("ui/fixtures/nine-slice.png"),
        );
        let mut schedule = Schedule::default();
        schedule.add_systems(update_ui_images);

        app.world_mut().clear_trackers();
        schedule.run(app.world_mut());
        assert_eq!(
            app.world().entity(entity).get::<UiImageStatus>(),
            Some(&UiImageStatus::Ready {
                source_size: UVec2::splat(48)
            })
        );
        assert!(matches!(
            app.world()
                .entity(entity)
                .get::<ImageNode>()
                .unwrap()
                .image_mode,
            NodeImageMode::Sliced(_)
        ));

        app.world_mut().clear_trackers();
        schedule.run(app.world_mut());
        assert_image_component_changes(app.world(), entity, false);
    }

    #[test]
    fn advanced_runtime_exposes_declared_source_size_mismatch() {
        let mut app = image_runtime_test_app();
        let entity = spawn_runtime_advanced_image(
            &mut app,
            test_image(64, 48),
            nine_slice_spec("ui/fixtures/nine-slice.png"),
        );
        let mut schedule = Schedule::default();
        schedule.add_systems(update_ui_images);
        schedule.run(app.world_mut());

        assert_eq!(
            app.world().entity(entity).get::<UiImageStatus>(),
            Some(&UiImageStatus::Invalid(UiImageError::SourceSizeMismatch))
        );
    }

    #[test]
    fn rounded_image_panel_owns_clip_without_changing_child_constraints() {
        let (panel, frame, _) = ui_image_panel_node_with_radius(
            UiImageSize::FixedBox {
                width: 120.0,
                height: 80.0,
            },
            12.0,
        );

        assert_eq!(panel.width, px(120));
        assert_eq!(panel.height, px(80));
        assert_eq!(panel.overflow, Overflow::clip());
        assert_eq!(panel.border_radius, BorderRadius::all(px(12)));
        assert_eq!(frame.size.validate(), Ok(()));
        assert_eq!(frame.validation_error, None);
    }

    #[test]
    fn invalid_bundle_configuration_is_observable_and_uses_stable_fallback() {
        let (node, _, _, _, status, _) = ui_image(
            Handle::default(),
            UiImageFit::Stretch,
            UiImageSize::FixedBox {
                width: 0.0,
                height: 40.0,
            },
        );

        assert_eq!(node.width, px(INVALID_IMAGE_FALLBACK_WIDTH));
        assert_eq!(node.height, px(INVALID_IMAGE_FALLBACK_HEIGHT));
        assert_eq!(
            status,
            UiImageStatus::Invalid(UiImageError::NonPositiveConstraint(
                UiImageConstraintField::Width
            ))
        );
        assert_eq!(status.code(), "non_positive_constraint");
    }

    #[test]
    fn ready_runtime_does_not_mark_stable_components_changed_again() {
        let mut app = image_runtime_test_app();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(test_image(200, 100));
        let entity = spawn_runtime_image(&mut app, handle, UiImageFit::cover(UiImageFocus::CENTER));
        let mut schedule = Schedule::default();
        schedule.add_systems(update_ui_images);

        app.world_mut().clear_trackers();
        schedule.run(app.world_mut());
        assert_image_component_changes(app.world(), entity, true);

        app.world_mut().clear_trackers();
        schedule.run(app.world_mut());
        assert_image_component_changes(app.world(), entity, false);
    }

    #[test]
    fn loading_placeholder_does_not_mark_stable_components_changed_again() {
        let mut app = image_runtime_test_app();
        let entity = spawn_runtime_image(&mut app, Handle::default(), UiImageFit::Natural);
        let mut schedule = Schedule::default();
        schedule.add_systems(update_ui_images);

        app.world_mut().clear_trackers();
        schedule.run(app.world_mut());
        assert!(
            app.world()
                .entity(entity)
                .get_ref::<Node>()
                .unwrap()
                .is_changed()
        );
        assert!(
            app.world()
                .entity(entity)
                .get_ref::<ImageNode>()
                .unwrap()
                .is_changed()
        );

        app.world_mut().clear_trackers();
        schedule.run(app.world_mut());
        assert_image_component_changes(app.world(), entity, false);
    }
}
