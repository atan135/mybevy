use std::fmt;

use bevy::{
    input::mouse::{MouseScrollUnit, MouseWheel},
    picking::hover::HoverMap,
    prelude::*,
};

use crate::framework::ui::style::UiTheme;

const UI_SCROLL_LINE_HEIGHT: f32 = 24.0;

pub(crate) struct UiScrollPlugin;

impl Plugin for UiScrollPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, send_scroll_events)
            .add_observer(on_scroll_handler)
            .add_observer(on_scroll_drag_start)
            .add_observer(on_scroll_drag);
    }
}

#[derive(Component)]
pub(crate) struct UiScrollView;

#[derive(Clone, Copy, Component, Debug, Eq, Hash, PartialEq)]
pub(crate) struct UiScrollAuditId(&'static str);

impl UiScrollAuditId {
    pub(crate) const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub(crate) const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for UiScrollAuditId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) enum UiScrollAuditPosition {
    Top,
    Middle,
    Bottom,
}

impl UiScrollAuditPosition {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Middle => "middle",
            Self::Bottom => "bottom",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UiScrollAuditMetrics {
    pub offset: f32,
    pub max_offset: f32,
    pub viewport_height: f32,
    pub content_height: f32,
    pub position: UiScrollAuditPosition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiScrollAuditSetError {
    Unreachable,
}

#[derive(Component, Default)]
struct UiScrollDragStart(Vec2);

#[derive(EntityEvent, Debug)]
#[entity_event(propagate, auto_propagate)]
struct UiScroll {
    entity: Entity,
    delta: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UiScrollViewConfig {
    pub row_gap: f32,
    pub max_height: Val,
    pub should_block_lower: bool,
}

impl UiScrollViewConfig {
    pub(crate) const fn new(row_gap: f32) -> Self {
        Self {
            row_gap,
            max_height: Val::Auto,
            should_block_lower: true,
        }
    }

    pub(crate) fn with_max_height(mut self, max_height: f32) -> Self {
        self.max_height = px(max_height);
        self
    }

    #[allow(dead_code)]
    pub(crate) const fn with_block_lower(mut self, should_block_lower: bool) -> Self {
        self.should_block_lower = should_block_lower;
        self
    }
}

pub(crate) fn ui_scroll_column(theme: &UiTheme) -> impl Bundle {
    ui_scroll_column_bundle(UiScrollViewConfig::new(theme.layout.page_gap))
}

pub(crate) fn ui_scroll_column_with_max_height(gap: f32, max_height: f32) -> impl Bundle {
    ui_scroll_column_bundle(UiScrollViewConfig::new(gap).with_max_height(max_height))
}

pub(crate) fn ui_scroll_column_bundle(config: UiScrollViewConfig) -> impl Bundle {
    (
        UiScrollView,
        UiScrollDragStart::default(),
        ScrollPosition(Vec2::ZERO),
        ui_scroll_column_node(config),
        ui_scroll_pickable(config),
    )
}

pub(crate) fn ui_scroll_column_node(config: UiScrollViewConfig) -> Node {
    Node {
        width: percent(100),
        flex_grow: 1.0,
        flex_direction: FlexDirection::Column,
        row_gap: px(config.row_gap),
        max_height: config.max_height,
        overflow: Overflow::scroll_y(),
        ..default()
    }
}

pub(crate) fn ui_scroll_pickable(config: UiScrollViewConfig) -> Pickable {
    Pickable {
        is_hoverable: true,
        should_block_lower: config.should_block_lower,
    }
}

fn send_scroll_events(
    mut mouse_wheel_reader: MessageReader<MouseWheel>,
    hover_map: Res<HoverMap>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
) {
    for mouse_wheel in mouse_wheel_reader.read() {
        let mut delta = -Vec2::new(mouse_wheel.x, mouse_wheel.y);

        if mouse_wheel.unit == MouseScrollUnit::Line {
            delta *= UI_SCROLL_LINE_HEIGHT;
        }

        if keyboard_input.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]) {
            std::mem::swap(&mut delta.x, &mut delta.y);
        }

        for pointer_map in hover_map.values() {
            for entity in pointer_map.keys().copied() {
                commands.trigger(UiScroll { entity, delta });
            }
        }
    }
}

fn on_scroll_handler(
    mut scroll: On<UiScroll>,
    mut scroll_views: Query<(&mut ScrollPosition, &Node, &ComputedNode), With<UiScrollView>>,
) {
    let Ok((mut scroll_position, node, computed)) = scroll_views.get_mut(scroll.entity) else {
        return;
    };

    let max_offset = max_scroll_offset(computed);
    let delta = &mut scroll.delta;

    if node.overflow.x == OverflowAxis::Scroll && delta.x != 0.0 {
        let next_x = (scroll_position.x + delta.x).clamp(0.0, max_offset.x);
        if next_x != scroll_position.x {
            scroll_position.x = next_x;
            delta.x = 0.0;
        }
    }

    if node.overflow.y == OverflowAxis::Scroll && delta.y != 0.0 {
        let next_y = (scroll_position.y + delta.y).clamp(0.0, max_offset.y);
        if next_y != scroll_position.y {
            scroll_position.y = next_y;
            delta.y = 0.0;
        }
    }

    if *delta == Vec2::ZERO {
        scroll.propagate(false);
    }
}

fn on_scroll_drag_start(
    drag_start: On<Pointer<DragStart>>,
    mut scroll_views: Query<(&ComputedNode, &mut UiScrollDragStart), With<UiScrollView>>,
) {
    let Ok((computed, mut start)) = scroll_views.get_mut(drag_start.entity) else {
        return;
    };

    start.0 = computed.scroll_position * computed.inverse_scale_factor;
}

fn on_scroll_drag(
    drag: On<Pointer<Drag>>,
    ui_scale: Res<UiScale>,
    mut scroll_views: Query<
        (&mut ScrollPosition, &UiScrollDragStart, &ComputedNode),
        With<UiScrollView>,
    >,
) {
    let Ok((mut scroll_position, start, computed)) = scroll_views.get_mut(drag.entity) else {
        return;
    };

    let max_offset = max_scroll_offset(computed);
    let next = start.0 - drag.distance / ui_scale.0;
    scroll_position.0 = next.clamp(Vec2::ZERO, max_offset);
}

pub(crate) fn set_scroll_audit_position(
    scroll_position: &mut ScrollPosition,
    computed: &ComputedNode,
    position: UiScrollAuditPosition,
) -> Result<UiScrollAuditMetrics, UiScrollAuditSetError> {
    let metrics = scroll_audit_metrics(scroll_position, computed, position);
    let target = target_scroll_offset(position, metrics.max_offset)?;
    scroll_position.y = target;
    scroll_position.x = scroll_position.x.clamp(0.0, max_scroll_offset(computed).x);

    Ok(UiScrollAuditMetrics {
        offset: target,
        ..metrics
    })
}

pub(crate) fn scroll_audit_metrics(
    scroll_position: &ScrollPosition,
    computed: &ComputedNode,
    position: UiScrollAuditPosition,
) -> UiScrollAuditMetrics {
    let max_offset = max_scroll_offset(computed);
    let scale = computed.inverse_scale_factor();
    UiScrollAuditMetrics {
        offset: scroll_position.y.clamp(0.0, max_offset.y),
        max_offset: max_offset.y,
        viewport_height: computed.size().y * scale,
        content_height: computed.content_size().y * scale,
        position,
    }
}

pub(crate) fn target_scroll_offset(
    position: UiScrollAuditPosition,
    max_offset: f32,
) -> Result<f32, UiScrollAuditSetError> {
    if !max_offset.is_finite() || max_offset < 0.0 {
        return Err(UiScrollAuditSetError::Unreachable);
    }

    match position {
        UiScrollAuditPosition::Top => Ok(0.0),
        UiScrollAuditPosition::Middle if max_offset > f32::EPSILON => Ok(max_offset * 0.5),
        UiScrollAuditPosition::Bottom if max_offset > f32::EPSILON => Ok(max_offset),
        UiScrollAuditPosition::Middle | UiScrollAuditPosition::Bottom => {
            Err(UiScrollAuditSetError::Unreachable)
        }
    }
}

pub(crate) fn scroll_audit_position_reached(
    scroll_position: &ScrollPosition,
    computed: &ComputedNode,
    position: UiScrollAuditPosition,
) -> bool {
    let max_offset = max_scroll_offset(computed).y;
    let Ok(target) = target_scroll_offset(position, max_offset) else {
        return false;
    };
    (scroll_position.y.clamp(0.0, max_offset) - target).abs() <= 0.5
}

pub(crate) fn max_scroll_offset(computed: &ComputedNode) -> Vec2 {
    ((computed.content_size() - computed.size()) * computed.inverse_scale_factor()).max(Vec2::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_column_node_uses_vertical_overflow_and_max_height() {
        let config = UiScrollViewConfig::new(8.0).with_max_height(240.0);
        let node = ui_scroll_column_node(config);

        assert_eq!(node.width, percent(100));
        assert_eq!(node.flex_direction, FlexDirection::Column);
        assert_eq!(node.row_gap, px(8.0));
        assert_eq!(node.max_height, px(240.0));
        assert_eq!(node.overflow, Overflow::scroll_y());
    }

    #[test]
    fn scroll_pickable_blocks_lower_by_default() {
        let pickable = ui_scroll_pickable(UiScrollViewConfig::new(8.0));

        assert!(pickable.is_hoverable);
        assert!(pickable.should_block_lower);
    }

    #[test]
    fn scroll_pickable_can_disable_lower_blocking() {
        let pickable = ui_scroll_pickable(UiScrollViewConfig::new(8.0).with_block_lower(false));

        assert!(pickable.is_hoverable);
        assert!(!pickable.should_block_lower);
    }

    #[test]
    fn scroll_column_bundle_includes_scroll_state_and_drag_start() {
        let mut app = App::new();
        let entity = app
            .world_mut()
            .spawn(ui_scroll_column_bundle(UiScrollViewConfig::new(8.0)))
            .id();

        let entity_ref = app.world().entity(entity);
        assert!(entity_ref.contains::<UiScrollView>());
        assert!(entity_ref.contains::<UiScrollDragStart>());
        assert_eq!(
            entity_ref
                .get::<ScrollPosition>()
                .map(|position| position.0),
            Some(Vec2::ZERO)
        );
    }

    fn computed_node(size: Vec2, content_size: Vec2) -> ComputedNode {
        ComputedNode {
            size,
            content_size,
            inverse_scale_factor: 1.0,
            ..default()
        }
    }

    #[test]
    fn scroll_audit_id_exposes_stable_string() {
        let id = UiScrollAuditId::new("ui_gallery.main");

        assert_eq!(id.as_str(), "ui_gallery.main");
        assert_eq!(id.to_string(), "ui_gallery.main");
    }

    #[test]
    fn target_scroll_offsets_cover_top_middle_and_bottom() {
        assert_eq!(
            target_scroll_offset(UiScrollAuditPosition::Top, 120.0),
            Ok(0.0)
        );
        assert_eq!(
            target_scroll_offset(UiScrollAuditPosition::Middle, 120.0),
            Ok(60.0)
        );
        assert_eq!(
            target_scroll_offset(UiScrollAuditPosition::Bottom, 120.0),
            Ok(120.0)
        );
    }

    #[test]
    fn middle_and_bottom_are_unreachable_without_scroll_space() {
        assert_eq!(
            target_scroll_offset(UiScrollAuditPosition::Middle, 0.0),
            Err(UiScrollAuditSetError::Unreachable)
        );
        assert_eq!(
            target_scroll_offset(UiScrollAuditPosition::Bottom, 0.0),
            Err(UiScrollAuditSetError::Unreachable)
        );
        assert_eq!(
            target_scroll_offset(UiScrollAuditPosition::Top, 0.0),
            Ok(0.0)
        );
    }

    #[test]
    fn set_scroll_audit_position_updates_scroll_position_from_computed_node() {
        let computed = computed_node(Vec2::new(320.0, 200.0), Vec2::new(320.0, 500.0));
        let mut scroll_position = ScrollPosition(Vec2::ZERO);

        let metrics = set_scroll_audit_position(
            &mut scroll_position,
            &computed,
            UiScrollAuditPosition::Bottom,
        )
        .expect("bottom should be reachable when content is taller than viewport");

        assert_eq!(scroll_position.y, 300.0);
        assert_eq!(metrics.offset, 300.0);
        assert_eq!(metrics.max_offset, 300.0);
        assert_eq!(metrics.viewport_height, 200.0);
        assert_eq!(metrics.content_height, 500.0);
        assert!(scroll_audit_position_reached(
            &scroll_position,
            &computed,
            UiScrollAuditPosition::Bottom
        ));
    }
}
