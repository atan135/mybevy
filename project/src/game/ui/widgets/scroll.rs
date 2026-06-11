use bevy::{
    input::mouse::{MouseScrollUnit, MouseWheel},
    picking::hover::HoverMap,
    prelude::*,
};

use crate::game::ui::style::UiTheme;

const UI_SCROLL_LINE_HEIGHT: f32 = 24.0;

pub(in crate::game) struct UiScrollPlugin;

impl Plugin for UiScrollPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, send_scroll_events)
            .add_observer(on_scroll_handler)
            .add_observer(on_scroll_drag_start)
            .add_observer(on_scroll_drag);
    }
}

#[derive(Component)]
pub(in crate::game) struct UiScrollView;

#[derive(Component, Default)]
struct UiScrollDragStart(Vec2);

#[derive(EntityEvent, Debug)]
#[entity_event(propagate, auto_propagate)]
struct UiScroll {
    entity: Entity,
    delta: Vec2,
}

pub(in crate::game) fn ui_scroll_column(theme: &UiTheme) -> impl Bundle {
    ui_scroll_column_node(theme.layout.page_gap, Val::Auto)
}

pub(in crate::game) fn ui_scroll_column_with_max_height(gap: f32, max_height: f32) -> impl Bundle {
    ui_scroll_column_node(gap, px(max_height))
}

fn ui_scroll_column_node(row_gap: f32, max_height: Val) -> impl Bundle {
    (
        UiScrollView,
        UiScrollDragStart::default(),
        ScrollPosition(Vec2::ZERO),
        Node {
            width: percent(100),
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            row_gap: px(row_gap),
            max_height,
            overflow: Overflow::scroll_y(),
            ..default()
        },
        Pickable {
            is_hoverable: true,
            should_block_lower: true,
        },
    )
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

fn max_scroll_offset(computed: &ComputedNode) -> Vec2 {
    ((computed.content_size() - computed.size()) * computed.inverse_scale_factor()).max(Vec2::ZERO)
}
