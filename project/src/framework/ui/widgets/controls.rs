use std::collections::HashSet;

use bevy::{
    input::keyboard::{Key, KeyCode, KeyboardInput},
    picking::{
        PickingSystems,
        events::{Cancel, Click, Pointer, Press, Release},
        pointer::PointerButton,
    },
    prelude::*,
    text::TextLayoutInfo,
    ui::{FocusPolicy, RelativeCursorPosition, UiSystems},
};

use crate::framework::ui::{
    core::{UiFocusSystems, UiMetrics, focus::UiFocusState},
    i18n::{UiI18n, UiI18nText},
    style::{
        UiFontAssets,
        theme::{
            ButtonColors, UiTheme, UiThemeButtonNodeRole, UiThemeTextColorRole,
            UiThemeTextStyleRole,
        },
    },
    widgets::scroll::UiScrollPlugin,
};

const NUMERIC_CONTROL_LABEL_WIDTH: f32 = 132.0;
const TEXT_INPUT_FOCUS_SWITCH_LOG_TICKS: u64 = 12;
const TEXT_INPUT_CARET_WIDTH: f32 = 1.5;

mod button;
mod numeric;
mod plugin;
mod selection;
mod text_input;

pub(crate) use button::*;
pub(crate) use numeric::*;
pub(crate) use plugin::*;
pub(crate) use selection::*;
pub(crate) use text_input::*;

#[cfg(test)]
mod tests {
    use super::*;

    fn editable(max_chars: Option<usize>) -> UiTextInputEditMode {
        UiTextInputEditMode {
            readonly: false,
            disabled: false,
            max_chars,
        }
    }

    fn readonly() -> UiTextInputEditMode {
        UiTextInputEditMode {
            readonly: true,
            disabled: false,
            max_chars: None,
        }
    }

    fn disabled() -> UiTextInputEditMode {
        UiTextInputEditMode {
            readonly: false,
            disabled: true,
            max_chars: None,
        }
    }

    fn cursor(position: usize) -> UiTextInputCursor {
        UiTextInputCursor {
            position,
            selection: None,
        }
    }

    fn required(message: &str) -> UiTextInputRequired {
        UiTextInputRequired::new(message)
    }

    #[test]
    fn insert_adds_text_at_cursor() {
        let mut value = "ab".to_string();
        let mut cursor = cursor(1);

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::Insert("X"),
            editable(None),
        );

        assert_eq!(value, "aXb");
        assert_eq!(cursor.position, 2);
    }

    #[test]
    fn cursor_moves_left_right_and_home_end() {
        let mut value = "abc".to_string();
        let mut cursor = cursor(value.len());

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::MoveLeft,
            editable(None),
        );
        assert_eq!(cursor.position, 2);

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::MoveRight,
            editable(None),
        );
        assert_eq!(cursor.position, 3);

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::MoveHome,
            editable(None),
        );
        assert_eq!(cursor.position, 0);

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::MoveEnd,
            editable(None),
        );
        assert_eq!(cursor.position, value.len());
    }

    #[test]
    fn backspace_deletes_before_cursor() {
        let mut value = "abc".to_string();
        let mut cursor = cursor(2);

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::Backspace,
            editable(None),
        );

        assert_eq!(value, "ac");
        assert_eq!(cursor.position, 1);
    }

    #[test]
    fn delete_removes_after_cursor() {
        let mut value = "abc".to_string();
        let mut cursor = cursor(1);

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::Delete,
            editable(None),
        );

        assert_eq!(value, "ac");
        assert_eq!(cursor.position, 1);
    }

    #[test]
    fn max_chars_limits_inserted_text() {
        let mut value = "ab".to_string();
        let mut cursor = cursor(value.len());

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::Insert("cde"),
            editable(Some(4)),
        );

        assert_eq!(value, "abcd");
        assert_eq!(cursor.position, value.len());
    }

    #[test]
    fn selected_text_is_replaced_and_counts_against_max_chars() {
        let mut value = "abcd".to_string();
        let mut cursor = UiTextInputCursor {
            position: 3,
            selection: Some(UiTextInputSelection { start: 1, end: 3 }),
        };

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::Insert("XYZ"),
            editable(Some(5)),
        );

        assert_eq!(value, "aXYZd");
        assert_eq!(cursor.position, 4);
    }

    #[test]
    fn text_input_display_splits_selected_range() {
        let cursor = UiTextInputCursor {
            position: 3,
            selection: Some(UiTextInputSelection { start: 1, end: 3 }),
        };

        assert_eq!(
            text_input_segments("abcd", &cursor).display,
            UiTextInputDisplay {
                plain: "a".to_string(),
                selected: "bc".to_string(),
                tail: "d".to_string(),
            }
        );
    }

    #[test]
    fn text_input_caret_prefix_tracks_cursor_without_display_character() {
        assert_eq!(text_input_caret_prefix("abcd", &cursor(2)), "ab");

        let selected_cursor = UiTextInputCursor {
            position: 3,
            selection: Some(UiTextInputSelection { start: 1, end: 3 }),
        };
        assert_eq!(text_input_caret_prefix("abcd", &selected_cursor), "abc");
    }

    #[test]
    fn text_input_segments_share_display_and_caret_boundaries() {
        let selection_start_cursor = UiTextInputCursor {
            position: 1,
            selection: Some(UiTextInputSelection { start: 1, end: 3 }),
        };
        assert_eq!(
            text_input_segments("abcd", &selection_start_cursor),
            UiTextInputSegments {
                display: UiTextInputDisplay {
                    plain: "a".to_string(),
                    selected: "bc".to_string(),
                    tail: "d".to_string(),
                },
                caret_prefix: "a".to_string(),
            }
        );

        let selection_end_cursor = UiTextInputCursor {
            position: 3,
            selection: Some(UiTextInputSelection { start: 1, end: 3 }),
        };
        assert_eq!(
            text_input_segments("abcd", &selection_end_cursor).caret_prefix,
            "abc"
        );
    }

    #[test]
    fn text_input_cursor_position_maps_ratio_to_char_boundary() {
        assert_eq!(text_input_cursor_position_from_ratio("abcd", 0.0), 0);
        assert_eq!(text_input_cursor_position_from_ratio("abcd", 0.5), 2);
        assert_eq!(text_input_cursor_position_from_ratio("abcd", 1.0), 4);
        assert_eq!(
            text_input_cursor_position_from_ratio("你好吗", 0.5),
            "你好".len()
        );
    }

    #[test]
    fn native_state_from_value_uses_cursor_or_selection() {
        let value = "abcd";
        let selected_cursor = UiTextInputCursor {
            position: 3,
            selection: Some(UiTextInputSelection { start: 1, end: 3 }),
        };

        assert_eq!(
            ui_text_input_native_state_from_value(value, &selected_cursor),
            UiTextInputNativeState {
                text: "abcd".to_string(),
                selection_start: 1,
                selection_end: 3,
            }
        );

        assert_eq!(
            ui_text_input_native_state_from_value(value, &cursor(2)),
            UiTextInputNativeState {
                text: "abcd".to_string(),
                selection_start: 2,
                selection_end: 2,
            }
        );
    }

    #[test]
    fn apply_native_state_clamps_selection_and_max_chars() {
        let mut value = String::new();
        let mut cursor = cursor(0);

        apply_native_text_input_state(
            &mut value,
            &mut cursor,
            UiTextInputNativeState {
                text: "你好吗".to_string(),
                selection_start: "你好".len(),
                selection_end: usize::MAX,
            },
            Some(2),
        );

        assert_eq!(value, "你好");
        assert_eq!(cursor.position, value.len());
        assert_eq!(cursor.selection, None);
    }

    #[test]
    fn apply_native_state_normalizes_reversed_selection() {
        let mut value = String::new();
        let mut cursor = cursor(0);

        apply_native_text_input_state(
            &mut value,
            &mut cursor,
            UiTextInputNativeState {
                text: "abcd".to_string(),
                selection_start: 3,
                selection_end: 1,
            },
            None,
        );

        assert_eq!(value, "abcd");
        assert_eq!(cursor.position, 3);
        assert_eq!(
            cursor.selection,
            Some(UiTextInputSelection { start: 1, end: 3 })
        );
    }

    #[test]
    fn keyboard_diagnostics_only_log_near_focus_changes() {
        let keyboard_input = KeyboardInput {
            key_code: KeyCode::KeyX,
            logical_key: Key::Character("x".into()),
            state: bevy::input::ButtonState::Pressed,
            text: Some("x".into()),
            repeat: false,
            window: Entity::PLACEHOLDER,
        };

        assert!(should_log_text_input_keyboard_event(
            &keyboard_input,
            TEXT_INPUT_FOCUS_SWITCH_LOG_TICKS,
            "ab",
            "axb",
            1,
            2,
            None,
            None,
        ));
        assert!(!should_log_text_input_keyboard_event(
            &keyboard_input,
            TEXT_INPUT_FOCUS_SWITCH_LOG_TICKS + 1,
            "ab",
            "axb",
            1,
            2,
            None,
            None,
        ));
    }

    #[test]
    fn readonly_does_not_edit_but_allows_cursor_movement() {
        let mut value = "abc".to_string();
        let mut cursor = cursor(2);

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::Insert("X"),
            readonly(),
        );
        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::Backspace,
            readonly(),
        );

        assert_eq!(value, "abc");
        assert_eq!(cursor.position, 2);

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::MoveLeft,
            readonly(),
        );

        assert_eq!(value, "abc");
        assert_eq!(cursor.position, 1);
    }

    #[test]
    fn disabled_does_not_edit_or_move_cursor() {
        let mut value = "abc".to_string();
        let mut cursor = cursor(2);

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::Insert("X"),
            disabled(),
        );
        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::MoveLeft,
            disabled(),
        );
        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::Delete,
            disabled(),
        );

        assert_eq!(value, "abc");
        assert_eq!(cursor.position, 2);
    }

    #[test]
    fn utf8_cursor_uses_char_boundaries() {
        let mut value = "你a".to_string();
        let mut cursor = cursor(value.len());

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::MoveLeft,
            editable(None),
        );
        assert_eq!(cursor.position, "你".len());

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::Backspace,
            editable(None),
        );

        assert_eq!(value, "a");
        assert_eq!(cursor.position, 0);
    }

    #[test]
    fn helper_text_displays_when_input_has_no_error() {
        assert_eq!(
            text_input_form_state("Pilot", Some("Visible helper"), None, None, false),
            UiTextInputFormState {
                message: Some("Visible helper".to_string()),
                is_error: false,
            }
        );
    }

    #[test]
    fn validation_message_overrides_helper_and_required() {
        let required = required("Required");

        assert_eq!(
            text_input_form_state(
                "",
                Some("Helper"),
                Some("Validation failed"),
                Some(&required),
                false,
            ),
            UiTextInputFormState {
                message: Some("Validation failed".to_string()),
                is_error: true,
            }
        );
    }

    #[test]
    fn alphanumeric_validation_clears_for_matching_value() {
        let rule = UiTextInputAlphanumeric::new(4, 8, "Use 4-8 letters or numbers.");

        assert_eq!(rule.validate("33333311"), None);
        assert_eq!(rule.validate("AB12"), None);
        assert_eq!(
            rule.validate("bad-code"),
            Some("Use 4-8 letters or numbers.")
        );
        assert_eq!(rule.validate("abc"), Some("Use 4-8 letters or numbers."));
        assert_eq!(
            rule.validate("abcdefghi"),
            Some("Use 4-8 letters or numbers.")
        );
    }

    #[test]
    fn required_empty_value_generates_error_state() {
        let required = required("Required");

        assert_eq!(
            text_input_form_state("", Some("Helper"), None, Some(&required), false),
            UiTextInputFormState {
                message: Some("Required".to_string()),
                is_error: true,
            }
        );
        assert_eq!(
            text_input_form_state("Pilot", Some("Helper"), None, Some(&required), false),
            UiTextInputFormState {
                message: Some("Helper".to_string()),
                is_error: false,
            }
        );
    }

    #[test]
    fn disabled_border_color_overrides_error_state() {
        let theme = UiTheme::default();

        assert_eq!(
            text_input_border_color(&theme, Interaction::None, true, true, true),
            theme.colors.secondary_button.disabled
        );
        assert_eq!(
            text_input_border_color(&theme, Interaction::None, true, false, true),
            theme.colors.error
        );
    }

    #[test]
    fn focused_text_input_border_is_stable_while_interacting() {
        let theme = UiTheme::default();

        assert_eq!(
            text_input_border_color(&theme, Interaction::Pressed, true, false, false),
            theme.colors.primary_button.focused
        );
        assert_eq!(
            text_input_border_color(&theme, Interaction::Hovered, true, false, false),
            theme.colors.primary_button.focused
        );
    }

    #[test]
    fn button_background_color_uses_documented_visual_priority() {
        let colors = UiTheme::default().colors.primary_button;

        assert_eq!(
            button_background_color(colors, Interaction::Pressed, true, true, true, true),
            colors.disabled
        );
        assert_eq!(
            button_background_color(colors, Interaction::Pressed, false, true, true, true),
            colors.loading
        );
        assert_eq!(
            button_background_color(colors, Interaction::Pressed, false, true, true, false),
            colors.pressed
        );
        assert_eq!(
            button_background_color(colors, Interaction::Hovered, false, true, true, false),
            colors.hovered
        );
        assert_eq!(
            button_background_color(colors, Interaction::None, false, true, true, false),
            colors.selected
        );
        assert_eq!(
            button_background_color(colors, Interaction::None, false, true, false, false),
            colors.focused
        );
        assert_eq!(
            button_background_color(colors, Interaction::None, false, false, false, false),
            colors.idle
        );
    }

    #[test]
    fn selection_visual_state_prioritizes_disabled_and_selected_colors() {
        let colors = UiTheme::default().colors.secondary_button;

        assert_eq!(
            selection_button_background_color(
                colors,
                Interaction::Hovered,
                true,
                SelectionVisualState::Disabled,
            ),
            colors.disabled
        );
        assert_eq!(
            selection_button_background_color(
                colors,
                Interaction::None,
                false,
                SelectionVisualState::Selected,
            ),
            colors.selected
        );
        assert_eq!(
            selection_button_background_color(
                colors,
                Interaction::None,
                true,
                SelectionVisualState::Idle,
            ),
            colors.focused
        );
    }

    #[test]
    fn selection_text_color_role_matches_disabled_state() {
        assert!(matches!(
            selection_button_text_color_role(SelectionVisualState::Disabled),
            UiThemeTextColorRole::Muted
        ));
        assert!(matches!(
            selection_button_text_color_role(SelectionVisualState::Selected),
            UiThemeTextColorRole::Primary
        ));
        assert!(matches!(
            selection_button_text_color_role(SelectionVisualState::Idle),
            UiThemeTextColorRole::Primary
        ));
    }

    #[test]
    fn selection_display_text_marks_state() {
        assert_eq!(
            selection_display_text("Medium", SelectionVisualState::Selected),
            "[x] Medium"
        );
        assert_eq!(
            selection_display_text("Medium", SelectionVisualState::Idle),
            "[ ] Medium"
        );
        assert_eq!(
            selection_display_text("Medium", SelectionVisualState::Disabled),
            "[-] Medium"
        );
    }

    #[test]
    fn button_event_target_resolves_child_to_parent_button() {
        let mut world = World::new();
        let button = world.spawn(Button).id();
        let label = world.spawn(Text::new("Label")).id();
        world.entity_mut(button).add_child(label);

        let mut buttons = world.query_filtered::<(), (
            With<Button>,
            Without<DisabledButton>,
            Without<LoadingButton>,
        )>();
        let mut parents = world.query::<&ChildOf>();

        let resolved =
            ui_button_event_target(label, &buttons.query(&world), &parents.query(&world));
        assert_eq!(resolved, Some(button));
    }

    #[test]
    fn selection_controls_toggle_only_on_click_event() {
        let mut app = App::new();
        app.add_message::<UiButtonEvent>()
            .add_systems(Update, update_selection_control_interactions);

        let checkbox = app
            .world_mut()
            .spawn((Button, UiCheckbox, Interaction::Pressed))
            .id();

        app.world_mut().write_message(UiButtonEvent {
            entity: checkbox,
            kind: UiButtonEventKind::Down,
            button: None,
        });
        app.update();
        assert!(!app.world().entity(checkbox).contains::<UiCheckboxChecked>());

        app.world_mut().write_message(UiButtonEvent {
            entity: checkbox,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.update();
        assert!(app.world().entity(checkbox).contains::<UiCheckboxChecked>());
    }

    #[test]
    fn stepper_changes_only_on_click_event() {
        let mut app = App::new();
        app.add_message::<UiButtonEvent>()
            .add_systems(Update, update_stepper_interactions);

        let stepper = app.world_mut().spawn(UiStepper::new(1, 1, 3, 1)).id();
        let increment = app
            .world_mut()
            .spawn((Button, UiStepperIncrementButton, Interaction::Pressed))
            .id();
        app.world_mut().entity_mut(stepper).add_child(increment);

        app.world_mut().write_message(UiButtonEvent {
            entity: increment,
            kind: UiButtonEventKind::Down,
            button: None,
        });
        app.update();
        assert_eq!(app.world().get::<UiStepper>(stepper).unwrap().value, 1);

        app.world_mut().write_message(UiButtonEvent {
            entity: increment,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.update();
        assert_eq!(app.world().get::<UiStepper>(stepper).unwrap().value, 2);
    }

    #[test]
    fn icon_button_background_and_text_roles_match_visual_state() {
        let colors = UiTheme::default().colors.secondary_button;

        assert_eq!(
            icon_button_background_color(colors, IconButtonVisualState::Idle),
            colors.idle
        );
        assert_eq!(
            icon_button_background_color(colors, IconButtonVisualState::Disabled),
            colors.disabled
        );
        assert_eq!(
            icon_button_background_color(colors, IconButtonVisualState::Loading),
            colors.loading
        );
        assert!(matches!(
            icon_button_text_color_role(IconButtonVisualState::Idle),
            UiThemeTextColorRole::Primary
        ));
        assert!(matches!(
            icon_button_text_color_role(IconButtonVisualState::Loading),
            UiThemeTextColorRole::Primary
        ));
        assert!(matches!(
            icon_button_text_color_role(IconButtonVisualState::Disabled),
            UiThemeTextColorRole::Muted
        ));
    }

    #[test]
    fn icon_button_node_uses_stable_square_button_size() {
        let theme = UiTheme::default();
        let metrics = UiMetrics::default();
        let node = icon_button_node(&theme, &metrics);

        assert_eq!(node.min_width, px(square_button_size(&metrics)));
        assert_eq!(node.width, px(square_button_size(&metrics)));
        assert_eq!(node.height, px(square_button_size(&metrics)));
        assert_eq!(node.padding, UiRect::ZERO);
        assert_eq!(
            node.border_radius,
            BorderRadius::all(px(theme.button.radius))
        );
    }

    #[test]
    fn compact_metrics_keep_core_control_nodes_at_touch_target() {
        let theme = UiTheme::default();
        let metrics = UiMetrics::default();
        let button = button_node(&theme, &metrics);
        let text_input = Node {
            min_height: px(metrics.input_height),
            ..default()
        };
        let icon = icon_button_node(&theme, &metrics);

        assert_eq!(button.height, px(metrics.button_height));
        assert!(metrics.button_height >= metrics.touch_target_min);
        assert!(metrics.input_height >= metrics.touch_target_min);
        assert_eq!(text_input.min_height, px(metrics.input_height));
        assert_eq!(icon.width, px(square_button_size(&metrics)));
        assert!(square_button_size(&metrics) >= metrics.touch_target_min);
    }

    #[test]
    fn stepper_value_width_is_metrics_derived_and_stable() {
        let metrics = UiMetrics::default();
        let first = stepper_value_node(&metrics);
        let second = stepper_value_node(&metrics);

        assert_eq!(first.width, px(stepper_value_width(&metrics)));
        assert_eq!(first.width, second.width);
        assert_eq!(first.min_height, second.min_height);
    }

    #[test]
    fn compact_numeric_controls_fit_phone_panel_width() {
        let theme = UiTheme::default();
        let viewport = crate::framework::ui::core::UiViewport::from_device_logical_size(
            1080.0 / 3.0,
            2400.0 / 3.0,
            crate::framework::ui::core::UiInputMode::MouseTouch,
            crate::framework::ui::core::UiSafeArea::default(),
        );
        let metrics = UiMetrics::from_viewport_and_theme(&viewport, &theme);
        let panel_inner_width = viewport.logical_width
            - metrics.page_padding * 2.0
            - theme.layout.panel_gap * 2.0
            - theme.panel.border * 2.0;
        let slider_min_width = numeric_control_label_width(&metrics)
            + slider_track_min_width(&metrics)
            + stepper_value_width(&metrics)
            + numeric_control_gap(&metrics) * 2.0
            + control_padding_x(&metrics) * 2.0;
        let stepper_min_width = numeric_control_label_width(&metrics)
            + square_button_size(&metrics) * 2.0
            + stepper_value_width(&metrics)
            + numeric_control_gap(&metrics) * 3.0;

        assert!(slider_min_width <= panel_inner_width);
        assert!(stepper_min_width <= panel_inner_width);
    }

    #[test]
    fn slider_ratio_orders_bounds_and_clamps_value() {
        assert_eq!(slider_ratio(50.0, 0.0, 100.0), 0.5);
        assert_eq!(slider_ratio(150.0, 0.0, 100.0), 1.0);
        assert_eq!(slider_ratio(-10.0, 0.0, 100.0), 0.0);
        assert_eq!(slider_ratio(25.0, 100.0, 0.0), 0.25);
        assert_eq!(slider_ratio(10.0, 10.0, 10.0), 0.0);
    }

    #[test]
    fn slider_model_orders_bounds_clamps_nan_and_formats_values() {
        let slider = UiSlider::new(f32::NAN, 100.0, 0.0);

        assert_eq!(slider.min, 0.0);
        assert_eq!(slider.max, 100.0);
        assert_eq!(slider.value, 0.0);
        assert_eq!(slider.ratio(), 0.0);
        assert_eq!(format_slider_value(42.02), "42");
        assert_eq!(format_slider_value(42.06), "42.1");
        assert_eq!(format_slider_value(42.16), "42.2");
    }

    #[test]
    fn slider_value_from_normalized_x_maps_track_position_to_value() {
        assert_eq!(slider_value_from_normalized_x(-0.5, 0.0, 100.0), 0.0);
        assert_eq!(slider_value_from_normalized_x(0.0, 0.0, 100.0), 50.0);
        assert_eq!(slider_value_from_normalized_x(0.5, 0.0, 100.0), 100.0);
        assert_eq!(slider_value_from_normalized_x(0.75, 0.0, 100.0), 100.0);
        assert_eq!(slider_value_from_normalized_x(-0.75, 0.0, 100.0), 0.0);
    }

    #[test]
    fn stepper_increment_and_decrement_clamp_to_bounds() {
        assert_eq!(stepper_increment_value(4, 1, 8, 2), 6);
        assert_eq!(stepper_increment_value(7, 1, 8, 2), 8);
        assert_eq!(stepper_decrement_value(4, 1, 8, 2), 2);
        assert_eq!(stepper_decrement_value(2, 1, 8, 2), 1);
        assert_eq!(stepper_increment_value(4, 8, 1, -2), 6);
        assert_eq!(stepper_decrement_value(4, 8, 1, 0), 3);
    }

    #[test]
    fn stepper_model_orders_bounds_clamps_value_and_normalizes_step() {
        let stepper = UiStepper::new(20, 10, 1, -3);

        assert_eq!(stepper.min, 1);
        assert_eq!(stepper.max, 10);
        assert_eq!(stepper.value, 10);
        assert_eq!(stepper.step, 3);

        let zero_stepper = UiStepper::new(5, 1, 10, 0);
        assert_eq!(zero_stepper.step, 1);
    }
}
