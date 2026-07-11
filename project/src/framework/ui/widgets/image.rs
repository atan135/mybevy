use std::fmt;

use bevy::{asset::LoadState, prelude::*};

const INVALID_IMAGE_FALLBACK_WIDTH: f32 = 96.0;
const INVALID_IMAGE_FALLBACK_HEIGHT: f32 = 64.0;

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

#[derive(Clone, Copy, Component, Debug)]
pub(crate) struct UiImageWidget {
    fit: UiImageFit,
    validation_error: Option<UiImageError>,
    ready_tint: Color,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UiImageFitLayout {
    pub render_size: Vec2,
    pub source_rect: Option<Rect>,
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
            fit,
            validation_error,
            ready_tint: Color::WHITE,
        },
        status,
        Name::new("UI image"),
    )
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
        let (container_size, frame_error) = parent
            .and_then(|parent| frames.get(parent.parent()).ok())
            .map(|(frame, computed)| {
                let logical_size = computed.content_box().size() * computed.inverse_scale_factor;
                (
                    logical_size,
                    frame
                        .validation_error
                        .or_else(|| frame.size.validate().err()),
                )
            })
            .unwrap_or_else(|| {
                (
                    computed.size() * computed.inverse_scale_factor,
                    widget.validation_error,
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
                match calculate_image_fit(widget.fit, source_size.as_vec2(), container_size) {
                    Ok(layout) => {
                        apply_ready_image(
                            &mut next_node,
                            &mut next_image_node,
                            &mut next_background,
                            widget.ready_tint,
                            layout,
                        );
                        UiImageStatus::Ready { source_size }
                    }
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
