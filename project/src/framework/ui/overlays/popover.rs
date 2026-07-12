use bevy::{input::keyboard::Key, picking::Pickable, prelude::*};

use crate::framework::ui::widgets::controls::SecondaryButton;
use crate::framework::ui::{
    core::{
        UI_PANEL_DROPDOWN, UI_PANEL_TOOLTIP, UiLayer, UiLayerRoot, UiMetrics, UiOwnerId,
        UiPanelCommand, UiPanelId, UiPanelKind, UiPanelRoot, UiPanelStack, UiViewport,
        close_top_target_id, focus::UiFocusState,
    },
    style::{
        UiFontAssets, UiTheme,
        theme::{UiThemeBackgroundRole, UiThemeBorderRole, UiThemeTextStyleRole},
    },
    widgets::{
        DisabledButton, FocusableButton, SelectedButton, UiButtonEvent, UiButtonEventKind,
        UiControlEvent, UiControlEventKind, UiControlEventReason, UiControlFlags, UiControlKind,
        UiControlMeta, UiControlValue, UiDropdown, UiTooltip, UiTooltipTone,
        ui_scroll_column_with_max_height,
    },
};

const POPOVER_Z_INDEX: i32 = 120;
const POPOVER_GAP: f32 = 8.0;
const POPOVER_EDGE_GAP: f32 = 8.0;
const TOOLTIP_WIDTH: f32 = 280.0;
const DROPDOWN_MIN_WIDTH: f32 = 220.0;
const DROPDOWN_MAX_WIDTH: f32 = 420.0;
const DROPDOWN_MAX_HEIGHT: f32 = 260.0;

#[derive(Clone, Debug)]
pub(crate) struct UiTooltipPanel {
    pub anchor: Entity,
    pub meta: UiControlMeta,
    pub owner: Option<UiOwnerId>,
    pub tooltip: UiTooltip,
}

#[derive(Clone, Debug)]
pub(crate) struct UiDropdownPanel {
    pub anchor: Entity,
    pub meta: UiControlMeta,
    pub owner: Option<UiOwnerId>,
    pub dropdown: UiDropdown,
}

#[derive(Clone, Copy, Debug, Component)]
pub(crate) struct UiPopoverAnchor {
    pub anchor: Entity,
    pub panel_id: UiPanelId,
    pub owner: Option<UiOwnerId>,
    pub meta: UiControlMeta,
    pub kind: UiControlKind,
}

#[derive(Component)]
pub(crate) struct UiTooltipOverlay;

#[derive(Component)]
pub(crate) struct UiDropdownOverlay;

#[derive(Component)]
pub(crate) struct UiPopoverDismissSurface;

#[derive(Component)]
pub(crate) struct UiPopoverBody;

#[derive(Default, Resource)]
pub(crate) struct UiPopoverFocusReturn(Option<Entity>);

#[derive(Clone, Debug, Component)]
pub(crate) struct UiDropdownOptionButton {
    pub control: Entity,
    pub index: usize,
    pub value: String,
    pub selected: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiPopoverPlacement {
    Above,
    Below,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UiPopoverLayout {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub placement: UiPopoverPlacement,
}

pub(crate) fn spawn_tooltip_panel(
    commands: &mut Commands,
    theme: &UiTheme,
    viewport: &UiViewport,
    fonts: &UiFontAssets,
    panel: &UiTooltipPanel,
    fallback_owner: Option<UiOwnerId>,
) {
    let owner = panel.owner.or(fallback_owner);
    let marker = UiPopoverAnchor {
        anchor: panel.anchor,
        panel_id: UI_PANEL_TOOLTIP,
        owner,
        meta: panel.meta,
        kind: UiControlKind::Tooltip,
    };
    let text_color = if panel.tooltip.tone == UiTooltipTone::Error {
        theme.colors.text_error
    } else {
        theme.colors.text_primary
    };
    let border_color = if panel.tooltip.tone == UiTooltipTone::Error {
        theme.colors.error
    } else {
        theme.colors.panel_border
    };

    commands
        .spawn((
            UiPanelRoot {
                id: UI_PANEL_TOOLTIP,
                kind: UiPanelKind::Floating,
                owner,
            },
            UiLayerRoot {
                layer: UiLayer::Floating,
            },
            marker,
            UiTooltipOverlay,
            Pickable::IGNORE,
            popover_root_node(),
            ZIndex(POPOVER_Z_INDEX),
        ))
        .with_children(|root| {
            root.spawn((
                UiPopoverBody,
                Pickable::IGNORE,
                tooltip_body_node(theme, viewport),
                BackgroundColor(theme.colors.panel_background.with_alpha(1.0)),
                BorderColor::all(border_color),
                UiThemeBackgroundRole::Popover,
            ))
            .with_children(|body| {
                body.spawn((
                    Text::new(panel.tooltip.text.clone()),
                    TextFont {
                        font: fonts.regular.clone(),
                        font_size: theme.text.caption,
                        ..default()
                    },
                    TextColor(text_color),
                    UiThemeTextStyleRole::Caption,
                ));
            });
        });
}

pub(crate) fn spawn_dropdown_panel(
    commands: &mut Commands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    viewport: &UiViewport,
    fonts: &UiFontAssets,
    panel: &UiDropdownPanel,
    fallback_owner: Option<UiOwnerId>,
) {
    let owner = panel.owner.or(fallback_owner);
    let marker = UiPopoverAnchor {
        anchor: panel.anchor,
        panel_id: UI_PANEL_DROPDOWN,
        owner,
        meta: panel.meta,
        kind: UiControlKind::Dropdown,
    };

    commands
        .spawn((
            UiPanelRoot {
                id: UI_PANEL_DROPDOWN,
                kind: UiPanelKind::Floating,
                owner,
            },
            UiLayerRoot {
                layer: UiLayer::Floating,
            },
            marker,
            UiDropdownOverlay,
            UiPopoverDismissSurface,
            Button,
            popover_root_node(),
            ZIndex(POPOVER_Z_INDEX),
            BackgroundColor(Color::NONE),
        ))
        .with_children(|root| {
            root.spawn((
                UiPopoverBody,
                Button,
                dropdown_body_node(theme, viewport),
                BackgroundColor(theme.colors.panel_background.with_alpha(1.0)),
                BorderColor::all(theme.colors.panel_border),
                UiThemeBackgroundRole::Popover,
                UiThemeBorderRole::Panel,
            ))
            .with_children(|body| {
                body.spawn(ui_scroll_column_with_max_height(
                    theme.layout.row_gap.max(4.0),
                    DROPDOWN_MAX_HEIGHT,
                ))
                .with_children(|options| {
                    for (index, option) in panel.dropdown.options.iter().enumerate() {
                        let selected = panel.dropdown.selected == Some(index);
                        let mut option_entity = options.spawn((
                            Button,
                            FocusableButton,
                            SecondaryButton,
                            UiDropdownOptionButton {
                                control: panel.anchor,
                                index,
                                value: option.value.clone(),
                                selected,
                            },
                            Node {
                                width: percent(100),
                                min_height: px(metrics.touch_target_min),
                                align_items: AlignItems::Center,
                                padding: UiRect::axes(px(theme.button.padding_x), px(4)),
                                border_radius: BorderRadius::all(px(
                                    (theme.button.radius - 2.0).max(0.0)
                                )),
                                ..default()
                            },
                            BackgroundColor(if selected {
                                theme.colors.secondary_button.selected
                            } else {
                                theme.colors.secondary_button.idle
                            }),
                            children![(
                                Text::new(option.label.clone()),
                                TextFont {
                                    font: fonts.regular.clone(),
                                    font_size: theme.text.button,
                                    ..default()
                                },
                                TextColor(if option.disabled {
                                    theme.colors.text_muted
                                } else {
                                    theme.colors.text_primary
                                }),
                                UiThemeTextStyleRole::Button,
                            )],
                        ));
                        if option.disabled {
                            option_entity.insert(DisabledButton);
                        }
                        if selected {
                            option_entity.insert(SelectedButton);
                        }
                    }
                });
            });
        });
}

fn popover_root_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(0),
        right: px(0),
        top: px(0),
        bottom: px(0),
        ..default()
    }
}

fn tooltip_body_node(theme: &UiTheme, viewport: &UiViewport) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(POPOVER_EDGE_GAP),
        top: px(POPOVER_EDGE_GAP),
        width: px(TOOLTIP_WIDTH.min(popover_available_width(viewport))),
        max_width: px(popover_available_width(viewport)),
        padding: UiRect::axes(px(12), px(8)),
        border: UiRect::all(px(theme.panel.border)),
        border_radius: BorderRadius::all(px(theme.button.radius)),
        ..default()
    }
}

fn dropdown_body_node(theme: &UiTheme, viewport: &UiViewport) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(POPOVER_EDGE_GAP),
        top: px(POPOVER_EDGE_GAP),
        width: px(DROPDOWN_MIN_WIDTH.min(popover_available_width(viewport))),
        max_width: px(popover_available_width(viewport).min(DROPDOWN_MAX_WIDTH)),
        max_height: px(DROPDOWN_MAX_HEIGHT),
        flex_direction: FlexDirection::Column,
        padding: UiRect::all(px(4)),
        border: UiRect::all(px(theme.panel.border)),
        border_radius: BorderRadius::all(px(theme.button.radius)),
        overflow: Overflow::clip(),
        ..default()
    }
}

fn popover_available_width(viewport: &UiViewport) -> f32 {
    (viewport.logical_width
        - viewport.safe_area.left
        - viewport.safe_area.right
        - POPOVER_EDGE_GAP * 2.0)
        .max(1.0)
}

pub(crate) fn update_popover_positions(
    viewport: Res<UiViewport>,
    popovers: Query<&UiPopoverAnchor>,
    anchors: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut bodies: Query<(&ChildOf, &ComputedNode, &mut Node), With<UiPopoverBody>>,
) {
    for (parent, body_computed, mut node) in &mut bodies {
        let Ok(popover) = popovers.get(parent.parent()) else {
            continue;
        };
        let Ok((anchor_computed, anchor_transform)) = anchors.get(popover.anchor) else {
            continue;
        };
        let anchor_rect = logical_node_rect(anchor_computed, anchor_transform);
        let popup_size = logical_node_size(body_computed);
        let preferred_width = match popover.kind {
            UiControlKind::Dropdown => anchor_rect
                .width()
                .max(DROPDOWN_MIN_WIDTH)
                .min(DROPDOWN_MAX_WIDTH),
            UiControlKind::Tooltip => TOOLTIP_WIDTH,
            _ => popup_size.x,
        }
        .min(popover_available_width(&viewport));
        let layout = resolve_popover_layout(
            anchor_rect,
            Vec2::new(preferred_width, popup_size.y.max(32.0)),
            &viewport,
        );
        let next_left = px(layout.left);
        if node.left != next_left {
            node.left = next_left;
        }
        let next_top = px(layout.top);
        if node.top != next_top {
            node.top = next_top;
        }
        let next_width = px(layout.width);
        if node.width != next_width {
            node.width = next_width;
        }
    }
}

fn logical_node_size(computed: &ComputedNode) -> Vec2 {
    computed.size() * computed.inverse_scale_factor()
}

fn logical_node_rect(computed: &ComputedNode, transform: &UiGlobalTransform) -> Rect {
    let inverse_scale = computed.inverse_scale_factor();
    let center = transform.affine().translation * inverse_scale;
    Rect::from_center_size(center, logical_node_size(computed))
}

pub(crate) fn resolve_popover_layout(
    anchor: Rect,
    popup_size: Vec2,
    viewport: &UiViewport,
) -> UiPopoverLayout {
    let safe_left = viewport.safe_area.left + POPOVER_EDGE_GAP;
    let safe_top = viewport.safe_area.top + POPOVER_EDGE_GAP;
    let safe_right = viewport.logical_width - viewport.safe_area.right - POPOVER_EDGE_GAP;
    let safe_bottom = viewport.logical_height - viewport.safe_area.bottom - POPOVER_EDGE_GAP;
    let available_width = (safe_right - safe_left).max(1.0);
    let width = popup_size.x.clamp(1.0, available_width);
    let left = anchor
        .min
        .x
        .clamp(safe_left, (safe_right - width).max(safe_left));
    let below_top = anchor.max.y + POPOVER_GAP;
    let above_top = anchor.min.y - POPOVER_GAP - popup_size.y;
    let (placement, desired_top) = if below_top + popup_size.y <= safe_bottom
        || safe_bottom - anchor.max.y >= anchor.min.y - safe_top
    {
        (UiPopoverPlacement::Below, below_top)
    } else {
        (UiPopoverPlacement::Above, above_top)
    };
    let top = desired_top.clamp(safe_top, (safe_bottom - popup_size.y).max(safe_top));

    UiPopoverLayout {
        left,
        top,
        width,
        placement,
    }
}

pub(crate) fn handle_popover_button_events(
    mut commands: Commands,
    dismiss_surfaces: Query<&UiPopoverAnchor, With<UiPopoverDismissSurface>>,
    option_buttons: Query<&UiDropdownOptionButton>,
    mut dropdowns: Query<(&mut UiDropdown, &mut UiControlFlags, &UiControlMeta)>,
    mut button_events: MessageReader<UiButtonEvent>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
    mut control_events: MessageWriter<UiControlEvent>,
    mut focus_return: ResMut<UiPopoverFocusReturn>,
) {
    for event in button_events.read() {
        if event.kind != UiButtonEventKind::Click {
            continue;
        }

        if let Ok(popover) = dismiss_surfaces.get(event.entity) {
            panel_commands.write(UiPanelCommand::Close(popover.panel_id));
            focus_return.0 = Some(popover.anchor);
            control_events.write(UiControlEvent {
                entity: popover.anchor,
                owner: popover.owner,
                control_id: popover.meta.id,
                control_kind: popover.meta.kind,
                kind: UiControlEventKind::Closed,
                value: UiControlValue::None,
                reason: UiControlEventReason::ClickAway,
            });
            continue;
        }

        let Ok(option) = option_buttons.get(event.entity) else {
            continue;
        };
        let Ok((mut dropdown, mut flags, meta)) = dropdowns.get_mut(option.control) else {
            panel_commands.write(UiPanelCommand::Close(UI_PANEL_DROPDOWN));
            continue;
        };
        if dropdown.selected != Some(option.index) {
            dropdown.selected = Some(option.index);
        }
        if !flags.selected {
            flags.selected = true;
        }
        commands.entity(option.control).insert(SelectedButton);
        focus_return.0 = Some(option.control);
        let popover = dismiss_surfaces
            .iter()
            .find(|popover| popover.anchor == option.control);
        let reason = if event.button.is_some() {
            UiControlEventReason::Pointer
        } else {
            UiControlEventReason::Keyboard
        };
        let owner = popover.and_then(|popover| popover.owner);
        control_events.write(UiControlEvent {
            entity: option.control,
            owner,
            control_id: meta.id,
            control_kind: meta.kind,
            kind: UiControlEventKind::ValueChanged,
            value: UiControlValue::Text(option.value.clone()),
            reason,
        });
        control_events.write(UiControlEvent {
            entity: option.control,
            owner,
            control_id: meta.id,
            control_kind: meta.kind,
            kind: UiControlEventKind::Closed,
            value: UiControlValue::Text(option.value.clone()),
            reason,
        });
        panel_commands.write(UiPanelCommand::Close(UI_PANEL_DROPDOWN));
    }
}

pub(crate) fn report_dropdown_escape(
    key_codes: Res<ButtonInput<KeyCode>>,
    keys: Res<ButtonInput<Key>>,
    popovers: Query<&UiPopoverAnchor, With<UiDropdownOverlay>>,
    panel_roots: Query<(
        Entity,
        &UiPanelRoot,
        Option<&crate::framework::ui::core::UiBlockingOverlay>,
    )>,
    panel_stack: Res<UiPanelStack>,
    mut control_events: MessageWriter<UiControlEvent>,
    mut focus_return: ResMut<UiPopoverFocusReturn>,
) {
    if !key_codes.just_pressed(KeyCode::Escape) && !keys.just_pressed(Key::BrowserBack) {
        return;
    }
    let Ok(popover) = popovers.single() else {
        return;
    };
    if close_top_target_id(&panel_roots, &panel_stack) != Some(UI_PANEL_DROPDOWN) {
        return;
    }
    focus_return.0 = Some(popover.anchor);
    control_events.write(UiControlEvent {
        entity: popover.anchor,
        owner: popover.owner,
        control_id: popover.meta.id,
        control_kind: popover.meta.kind,
        kind: UiControlEventKind::Closed,
        value: UiControlValue::None,
        reason: UiControlEventReason::Escape,
    });
}

pub(crate) fn restore_popover_focus(
    mut focus_return: ResMut<UiPopoverFocusReturn>,
    mut focus_state: ResMut<UiFocusState>,
    focusable_anchors: Query<
        (),
        (
            With<Button>,
            With<FocusableButton>,
            Without<DisabledButton>,
            Without<crate::framework::ui::widgets::LoadingButton>,
        ),
    >,
) {
    let Some(anchor) = focus_return.0.take() else {
        return;
    };
    if focusable_anchors.contains(anchor) {
        focus_state.focused_entity = Some(anchor);
    }
}

pub(crate) fn close_orphaned_popovers(
    entities: Query<Entity>,
    popovers: Query<&UiPopoverAnchor>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
    mut control_events: MessageWriter<UiControlEvent>,
) {
    for popover in &popovers {
        if entities.contains(popover.anchor) {
            continue;
        }
        panel_commands.write(UiPanelCommand::Close(popover.panel_id));
        control_events.write(UiControlEvent {
            entity: popover.anchor,
            owner: popover.owner,
            control_id: popover.meta.id,
            control_kind: popover.meta.kind,
            kind: UiControlEventKind::Closed,
            value: UiControlValue::None,
            reason: UiControlEventReason::OwnerRemoved,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::ui::core::{
        UiPanelCommand, UiSafeArea, focus::UiFocusPlugin, panel::UiPanelPlugin,
    };
    use crate::framework::ui::overlays::UiOverlayPlugin;
    use crate::framework::ui::widgets::{UiControlId, UiDropdownOption};

    fn popover_integration_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            UiOverlayPlugin,
            UiPanelPlugin,
            UiFocusPlugin,
        ))
        .insert_resource(UiTheme::default())
        .insert_resource(UiMetrics::default())
        .insert_resource(UiViewport::default())
        .insert_resource(UiFontAssets::test_registry())
        .insert_resource(ButtonInput::<KeyCode>::default())
        .insert_resource(ButtonInput::<Key>::default())
        .add_message::<UiButtonEvent>()
        .add_message::<UiControlEvent>()
        .add_systems(
            PostUpdate,
            crate::framework::ui::widgets::controls::focus_opened_dropdown,
        );
        app
    }

    fn spawn_modal_dropdown(app: &mut App) -> (Entity, Entity, UiControlMeta, UiDropdown) {
        let modal = app
            .world_mut()
            .spawn((
                UiPanelRoot {
                    id: crate::framework::ui::core::UI_PANEL_CONFIRM_MODAL,
                    kind: UiPanelKind::Modal,
                    owner: None,
                },
                ZIndex(100),
            ))
            .id();
        let meta = UiControlMeta::new(
            UiControlId::new("test.modal.dropdown"),
            UiControlKind::Dropdown,
        );
        let dropdown = UiDropdown::new(
            "Choose",
            vec![
                UiDropdownOption::new("north", "North"),
                UiDropdownOption::new("south", "South"),
            ],
            None,
        );
        let trigger = app
            .world_mut()
            .spawn((
                Button,
                FocusableButton,
                Interaction::None,
                InheritedVisibility::VISIBLE,
                meta,
                dropdown.clone(),
                UiControlFlags::default(),
            ))
            .id();
        let other_button = app
            .world_mut()
            .spawn((
                Button,
                FocusableButton,
                Interaction::None,
                InheritedVisibility::VISIBLE,
            ))
            .id();
        app.world_mut()
            .entity_mut(modal)
            .add_children(&[trigger, other_button]);
        (trigger, other_button, meta, dropdown)
    }

    fn open_dropdown(app: &mut App, trigger: Entity, meta: UiControlMeta, dropdown: &UiDropdown) {
        app.world_mut().write_message(UiPanelCommand::Open(
            crate::framework::ui::core::UiPanelRequest::Dropdown(UiDropdownPanel {
                anchor: trigger,
                meta,
                owner: None,
                dropdown: dropdown.clone(),
            }),
        ));
        app.update();
        app.update();
    }

    #[test]
    fn layout_uses_below_when_space_is_available() {
        let viewport = UiViewport::from_device_logical_size(
            400.0,
            800.0,
            crate::framework::ui::core::UiInputMode::MouseTouch,
            UiSafeArea::default(),
        );
        let layout = resolve_popover_layout(
            Rect::from_corners(Vec2::new(40.0, 100.0), Vec2::new(180.0, 146.0)),
            Vec2::new(220.0, 180.0),
            &viewport,
        );
        assert_eq!(layout.placement, UiPopoverPlacement::Below);
        assert_eq!(layout.top, 154.0);
    }

    #[test]
    fn layout_flips_above_and_avoids_right_edge() {
        let viewport = UiViewport::from_device_logical_size(
            400.0,
            800.0,
            crate::framework::ui::core::UiInputMode::MouseTouch,
            UiSafeArea::default(),
        );
        let layout = resolve_popover_layout(
            Rect::from_corners(Vec2::new(330.0, 700.0), Vec2::new(390.0, 746.0)),
            Vec2::new(220.0, 180.0),
            &viewport,
        );
        assert_eq!(layout.placement, UiPopoverPlacement::Above);
        assert_eq!(layout.left, 172.0);
        assert_eq!(layout.top, 512.0);
    }

    #[test]
    fn layout_respects_safe_area_and_narrow_viewport() {
        let viewport = UiViewport::from_device_logical_size(
            200.0,
            400.0,
            crate::framework::ui::core::UiInputMode::MouseTouch,
            UiSafeArea {
                left: 12.0,
                right: 16.0,
                top: 20.0,
                bottom: 10.0,
            },
        );
        let layout = resolve_popover_layout(
            Rect::from_corners(Vec2::new(0.0, 0.0), Vec2::new(30.0, 30.0)),
            Vec2::new(400.0, 80.0),
            &viewport,
        );
        assert_eq!(layout.left, 20.0);
        assert_eq!(layout.width, 156.0);
        assert!(layout.top >= 28.0);
    }

    #[test]
    fn orphaned_anchor_closes_panel_and_reports_stable_reason() {
        let mut app = App::new();
        app.add_message::<UiPanelCommand>()
            .add_message::<UiControlEvent>()
            .add_systems(Update, close_orphaned_popovers);
        let control_id = UiControlId::new("test.dropdown");
        app.world_mut().spawn(UiPopoverAnchor {
            anchor: Entity::PLACEHOLDER,
            panel_id: UI_PANEL_DROPDOWN,
            owner: None,
            meta: UiControlMeta::new(control_id, UiControlKind::Dropdown),
            kind: UiControlKind::Dropdown,
        });
        app.update();

        let panel_messages = app.world().resource::<Messages<UiPanelCommand>>();
        let mut panel_cursor = bevy::ecs::message::MessageCursor::default();
        assert!(
            panel_cursor
                .read(panel_messages)
                .any(|command| matches!(command, UiPanelCommand::Close(UI_PANEL_DROPDOWN)))
        );
        let control_messages = app.world().resource::<Messages<UiControlEvent>>();
        let mut control_cursor = bevy::ecs::message::MessageCursor::default();
        let event = control_cursor.read(control_messages).next().unwrap();
        assert_eq!(event.control_id, control_id);
        assert_eq!(event.kind, UiControlEventKind::Closed);
        assert_eq!(event.reason, UiControlEventReason::OwnerRemoved);
    }

    #[test]
    fn selecting_option_synchronizes_dropdown_flags_marker_value_and_focus_return() {
        let mut app = App::new();
        app.add_message::<UiButtonEvent>()
            .add_message::<UiPanelCommand>()
            .add_message::<UiControlEvent>()
            .init_resource::<UiPopoverFocusReturn>()
            .add_systems(Update, handle_popover_button_events);
        let control_id = UiControlId::new("test.dropdown");
        let dropdown = app
            .world_mut()
            .spawn((
                Button,
                FocusableButton,
                UiDropdown::new(
                    "Choose",
                    vec![UiDropdownOption::new("north", "North")],
                    None,
                ),
                UiControlFlags::default(),
                UiControlMeta::new(control_id, UiControlKind::Dropdown),
            ))
            .id();
        app.world_mut().spawn((
            UiPopoverDismissSurface,
            UiPopoverAnchor {
                anchor: dropdown,
                panel_id: UI_PANEL_DROPDOWN,
                owner: None,
                meta: UiControlMeta::new(control_id, UiControlKind::Dropdown),
                kind: UiControlKind::Dropdown,
            },
        ));
        let option = app
            .world_mut()
            .spawn(UiDropdownOptionButton {
                control: dropdown,
                index: 0,
                value: "north".to_owned(),
                selected: false,
            })
            .id();
        app.world_mut().write_message(UiButtonEvent {
            entity: option,
            kind: UiButtonEventKind::Click,
            button: None,
        });

        app.update();

        assert_eq!(
            app.world().get::<UiDropdown>(dropdown).unwrap().selected,
            Some(0)
        );
        assert!(
            app.world()
                .get::<UiControlFlags>(dropdown)
                .unwrap()
                .selected
        );
        assert!(app.world().entity(dropdown).contains::<SelectedButton>());
        assert_eq!(
            app.world().resource::<UiPopoverFocusReturn>().0,
            Some(dropdown)
        );
        let messages = app.world().resource::<Messages<UiControlEvent>>();
        let mut cursor = bevy::ecs::message::MessageCursor::default();
        let events = cursor.read(messages).collect::<Vec<_>>();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, UiControlEventKind::ValueChanged);
        assert_eq!(events[0].value, UiControlValue::Text("north".to_owned()));
        assert_eq!(events[1].kind, UiControlEventKind::Closed);
    }

    #[test]
    fn stable_popover_layout_does_not_mark_node_changed_again() {
        let mut app = App::new();
        app.insert_resource(UiViewport::from_device_logical_size(
            400.0,
            800.0,
            crate::framework::ui::core::UiInputMode::MouseTouch,
            UiSafeArea::default(),
        ))
        .add_systems(Update, update_popover_positions);
        let anchor = app
            .world_mut()
            .spawn((ComputedNode::default(), UiGlobalTransform::default()))
            .id();
        let root = app
            .world_mut()
            .spawn(UiPopoverAnchor {
                anchor,
                panel_id: UI_PANEL_DROPDOWN,
                owner: None,
                meta: UiControlMeta::new(
                    UiControlId::new("test.dropdown"),
                    UiControlKind::Dropdown,
                ),
                kind: UiControlKind::Dropdown,
            })
            .id();
        let body = app
            .world_mut()
            .spawn((UiPopoverBody, ComputedNode::default(), Node::default()))
            .id();
        app.world_mut().entity_mut(root).add_child(body);

        app.update();
        app.world_mut().clear_trackers();
        app.update();

        assert!(
            !app.world()
                .entity(body)
                .get_ref::<Node>()
                .unwrap()
                .is_changed()
        );
    }

    #[test]
    fn modal_dropdown_closures_restore_focus_to_trigger_in_production_order() {
        let mut app = popover_integration_app();
        let (trigger, other_button, meta, dropdown) = spawn_modal_dropdown(&mut app);

        open_dropdown(&mut app, trigger, meta, &dropdown);
        let option = app
            .world_mut()
            .query_filtered::<Entity, With<UiDropdownOptionButton>>()
            .iter(app.world())
            .next()
            .unwrap();
        assert_eq!(
            app.world().resource::<UiFocusState>().focused_entity,
            Some(option)
        );
        app.world_mut().write_message(UiButtonEvent {
            entity: option,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.update();
        assert_eq!(
            app.world().resource::<UiFocusState>().focused_entity,
            Some(trigger)
        );
        assert_ne!(
            app.world().resource::<UiFocusState>().focused_entity,
            Some(other_button)
        );

        open_dropdown(&mut app, trigger, meta, &dropdown);
        let dismiss = app
            .world_mut()
            .query_filtered::<Entity, With<UiPopoverDismissSurface>>()
            .iter(app.world())
            .next()
            .unwrap();
        app.world_mut().write_message(UiButtonEvent {
            entity: dismiss,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.update();
        assert_eq!(
            app.world().resource::<UiFocusState>().focused_entity,
            Some(trigger)
        );

        open_dropdown(&mut app, trigger, meta, &dropdown);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert_eq!(
            app.world().resource::<UiFocusState>().focused_entity,
            Some(trigger)
        );
        let mut dropdown_overlays = app
            .world_mut()
            .query_filtered::<Entity, With<UiDropdownOverlay>>();
        assert!(dropdown_overlays.iter(app.world()).next().is_none());
        let control_messages = app.world().resource::<Messages<UiControlEvent>>();
        let mut cursor = bevy::ecs::message::MessageCursor::default();
        assert!(cursor.read(control_messages).any(|event| {
            event.kind == UiControlEventKind::Closed && event.reason == UiControlEventReason::Escape
        }));
    }

    #[test]
    fn escape_does_not_report_dropdown_closed_when_newer_transient_is_on_top() {
        let mut app = popover_integration_app();
        let (trigger, _, meta, dropdown) = spawn_modal_dropdown(&mut app);
        open_dropdown(&mut app, trigger, meta, &dropdown);
        let floating_id = UiPanelId::new("test.newer.floating");
        app.world_mut().write_message(UiPanelCommand::Open(
            crate::framework::ui::core::UiPanelRequest::Floating(
                crate::framework::ui::core::UiFloatingPanel {
                    id: floating_id,
                    title: "Newer".to_owned(),
                    body: "Top panel".to_owned(),
                    detail: None,
                },
            ),
        ));
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();

        let mut panels = app.world_mut().query::<&UiPanelRoot>();
        assert!(
            panels
                .iter(app.world())
                .any(|panel| panel.id == UI_PANEL_DROPDOWN)
        );
        assert!(
            !panels
                .iter(app.world())
                .any(|panel| panel.id == floating_id)
        );
        let control_messages = app.world().resource::<Messages<UiControlEvent>>();
        let mut cursor = bevy::ecs::message::MessageCursor::default();
        assert!(!cursor.read(control_messages).any(|event| {
            event.kind == UiControlEventKind::Closed && event.reason == UiControlEventReason::Escape
        }));
    }
}
