use super::*;

#[derive(Clone, Copy, Debug, Component)]
pub(crate) struct UiSlider {
    pub value: f32,
    pub min: f32,
    pub max: f32,
}

impl UiSlider {
    pub(crate) fn new(value: f32, min: f32, max: f32) -> Self {
        let (min, max) = ordered_slider_bounds(min, max);
        Self {
            value: clamp_slider_value(value, min, max),
            min,
            max,
        }
    }

    pub(crate) fn ratio(self) -> f32 {
        slider_ratio(self.value, self.min, self.max)
    }
}

#[derive(Component)]
pub(crate) struct UiSliderFill;

#[derive(Component)]
pub(crate) struct UiSliderTrack;

#[derive(Component)]
pub(crate) struct UiSliderValueText;

#[derive(Clone, Copy, Debug, Component)]
pub(crate) struct UiStepper {
    pub value: i32,
    pub min: i32,
    pub max: i32,
    pub step: i32,
}

impl UiStepper {
    pub(crate) fn new(value: i32, min: i32, max: i32, step: i32) -> Self {
        let (min, max) = ordered_stepper_bounds(min, max);
        let step = stepper_step(step);
        Self {
            value: clamp_stepper_value(value, min, max),
            min,
            max,
            step,
        }
    }
}

#[derive(Component)]
pub(crate) struct UiStepperDecrementButton;

#[derive(Component)]
pub(crate) struct UiStepperIncrementButton;

#[derive(Component)]
pub(crate) struct UiStepperValueText;
pub(crate) fn slider_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
    value: f32,
    min: f32,
    max: f32,
) -> impl Bundle {
    slider_bundle(
        theme,
        metrics,
        fonts,
        i18n.tr(key, fallback),
        value,
        min,
        max,
        UiI18nText::new(key, fallback),
        (),
        false,
    )
}

pub(crate) fn disabled_slider_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
    value: f32,
    min: f32,
    max: f32,
) -> impl Bundle {
    slider_bundle(
        theme,
        metrics,
        fonts,
        i18n.tr(key, fallback),
        value,
        min,
        max,
        UiI18nText::new(key, fallback),
        DisabledButton,
        true,
    )
}

pub(crate) fn stepper_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
    value: i32,
    min: i32,
    max: i32,
    step: i32,
) -> impl Bundle {
    stepper_bundle(
        theme,
        metrics,
        fonts,
        i18n.tr(key, fallback),
        value,
        min,
        max,
        step,
        UiI18nText::new(key, fallback),
        (),
        UiStepperDecrementButton,
        UiStepperIncrementButton,
        false,
    )
}

pub(crate) fn disabled_stepper_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
    value: i32,
    min: i32,
    max: i32,
    step: i32,
) -> impl Bundle {
    stepper_bundle(
        theme,
        metrics,
        fonts,
        i18n.tr(key, fallback),
        value,
        min,
        max,
        step,
        UiI18nText::new(key, fallback),
        DisabledButton,
        (UiStepperDecrementButton, DisabledButton),
        (UiStepperIncrementButton, DisabledButton),
        true,
    )
}
pub(crate) fn slider_bundle<T: Bundle>(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    label: impl Into<String>,
    value: f32,
    min: f32,
    max: f32,
    label_i18n_text: UiI18nText,
    marker: T,
    disabled: bool,
) -> impl Bundle {
    let slider = UiSlider::new(value, min, max);
    let fill_color = if disabled {
        theme.colors.secondary_button.disabled
    } else {
        theme.colors.primary_button.idle
    };
    let value_color = if disabled {
        UiThemeTextColorRole::Muted
    } else {
        UiThemeTextColorRole::Primary
    };

    (
        Button,
        FocusableButton,
        UiThemeButtonNodeRole::TextInput,
        marker,
        slider,
        RelativeCursorPosition::default(),
        Node {
            width: percent(100),
            min_height: px(metrics.input_height),
            align_items: AlignItems::Center,
            column_gap: px(numeric_control_gap(metrics)),
            row_gap: px(metrics.control_gap),
            flex_wrap: FlexWrap::Wrap,
            padding: UiRect::axes(px(control_padding_x(metrics)), px(0)),
            border: UiRect::all(px(theme.panel.border)),
            border_radius: BorderRadius::all(px(theme.button.radius)),
            ..default()
        },
        BackgroundColor(text_input_background_color(
            theme,
            Interaction::None,
            false,
            disabled,
        )),
        BorderColor::all(text_input_border_color(
            theme,
            Interaction::None,
            false,
            disabled,
            false,
        )),
        children![
            (
                slider_label_node(metrics),
                FocusPolicy::Pass,
                Text::new(label),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: theme.text.button,
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Center),
                TextColor(value_color.color(theme)),
                value_color,
                UiThemeTextStyleRole::Button,
                label_i18n_text,
            ),
            (
                slider_track_node(metrics),
                UiSliderTrack,
                FocusPolicy::Pass,
                BackgroundColor(theme.colors.panel_border),
                children![(
                    UiSliderFill,
                    FocusPolicy::Pass,
                    Node {
                        width: percent(slider.ratio() * 100.0),
                        height: percent(100),
                        border_radius: BorderRadius::all(px(slider_track_height(metrics) * 0.5)),
                        ..default()
                    },
                    BackgroundColor(fill_color),
                )],
            ),
            (
                slider_value_node(metrics),
                FocusPolicy::Pass,
                Text::new(format_slider_value(slider.value)),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: theme.text.button,
                    ..default()
                },
                TextColor(value_color.color(theme)),
                value_color,
                UiThemeTextStyleRole::Button,
                UiSliderValueText,
            ),
        ],
    )
}

pub(crate) fn stepper_bundle<T: Bundle, D: Bundle, I: Bundle>(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    label: impl Into<String>,
    value: i32,
    min: i32,
    max: i32,
    step: i32,
    label_i18n_text: UiI18nText,
    marker: T,
    decrement_marker: D,
    increment_marker: I,
    disabled: bool,
) -> impl Bundle {
    let stepper = UiStepper::new(value, min, max, step);
    let value_color = if disabled {
        UiThemeTextColorRole::Muted
    } else {
        UiThemeTextColorRole::Primary
    };
    let stepper_button_colors = theme.colors.secondary_button;

    (
        marker,
        stepper,
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            column_gap: px(numeric_control_gap(metrics)),
            row_gap: px(metrics.control_gap),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        },
        children![
            (
                stepper_label_node(metrics),
                Text::new(label),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: theme.text.button,
                    ..default()
                },
                TextColor(value_color.color(theme)),
                value_color,
                UiThemeTextStyleRole::Button,
                label_i18n_text,
            ),
            (
                stepper_button(theme, metrics, fonts, "-", stepper_button_colors, disabled),
                decrement_marker,
            ),
            (
                stepper_value_node(metrics),
                Text::new(stepper.value.to_string()),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: theme.text.button,
                    ..default()
                },
                TextColor(value_color.color(theme)),
                value_color,
                UiThemeTextStyleRole::Button,
                UiStepperValueText,
            ),
            (
                stepper_button(theme, metrics, fonts, "+", stepper_button_colors, disabled),
                increment_marker,
            ),
        ],
    )
}

pub(crate) fn stepper_button(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    colors: ButtonColors,
    disabled: bool,
) -> impl Bundle {
    (
        Button,
        FocusableButton,
        SecondaryButton,
        UiThemeButtonNodeRole::Button,
        square_button_node(theme, metrics),
        BackgroundColor(button_background_color(
            colors,
            Interaction::None,
            disabled,
            false,
            false,
            false,
        )),
        children![(
            Text::new(text),
            TextFont {
                font: fonts.regular.clone(),
                font_size: theme.text.button,
                ..default()
            },
            TextColor(if disabled {
                theme.colors.text_muted
            } else {
                theme.colors.text_primary
            }),
            if disabled {
                UiThemeTextColorRole::Muted
            } else {
                UiThemeTextColorRole::Primary
            },
            UiThemeTextStyleRole::Button,
        )],
    )
}
pub(crate) fn numeric_control_gap(metrics: &UiMetrics) -> f32 {
    if numeric_control_is_compact(metrics) {
        return metrics.control_gap;
    }

    metrics.control_gap.max(10.0)
}

pub(crate) fn numeric_control_label_width(metrics: &UiMetrics) -> f32 {
    if numeric_control_is_compact(metrics) {
        return (metrics.dialog_max_width * 0.28).clamp(88.0, 104.0);
    }

    NUMERIC_CONTROL_LABEL_WIDTH
        .min(metrics.content_max_width * 0.34)
        .max(72.0)
}

pub(crate) fn slider_track_height(metrics: &UiMetrics) -> f32 {
    (metrics.icon_size * 0.36).clamp(8.0, 10.0)
}

pub(crate) fn stepper_value_width(metrics: &UiMetrics) -> f32 {
    if numeric_control_is_compact(metrics) {
        return (metrics.touch_target_min + metrics.control_gap).max(52.0);
    }

    (square_button_size(metrics) * 1.6).max(metrics.touch_target_min + metrics.control_gap * 2.0)
}

pub(crate) fn stepper_value_min_height(metrics: &UiMetrics) -> f32 {
    (metrics.button_height * 0.78).max(metrics.touch_target_min * 0.75)
}

pub(crate) fn slider_label_node(metrics: &UiMetrics) -> Node {
    Node {
        width: px(numeric_control_label_width(metrics)),
        flex_shrink: 0.0,
        ..default()
    }
}

pub(crate) fn slider_track_node(metrics: &UiMetrics) -> Node {
    let track_height = slider_track_height(metrics);
    Node {
        min_width: px(slider_track_min_width(metrics)),
        height: px(track_height),
        flex_grow: 1.0,
        flex_shrink: 1.0,
        overflow: Overflow::clip(),
        border_radius: BorderRadius::all(px(track_height * 0.5)),
        ..default()
    }
}

pub(crate) fn slider_value_node(metrics: &UiMetrics) -> Node {
    Node {
        width: px(stepper_value_width(metrics)),
        flex_shrink: 0.0,
        justify_content: JustifyContent::FlexEnd,
        ..default()
    }
}

pub(crate) fn stepper_label_node(metrics: &UiMetrics) -> Node {
    Node {
        width: px(numeric_control_label_width(metrics)),
        flex_shrink: 0.0,
        ..default()
    }
}

pub(crate) fn stepper_value_node(metrics: &UiMetrics) -> Node {
    Node {
        width: px(stepper_value_width(metrics)),
        min_height: px(stepper_value_min_height(metrics)),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        padding: UiRect::horizontal(px(metrics.control_gap)),
        border: UiRect::all(px(1)),
        border_radius: BorderRadius::all(px(4)),
        ..default()
    }
}

pub(crate) fn slider_track_min_width(metrics: &UiMetrics) -> f32 {
    if numeric_control_is_compact(metrics) {
        return (metrics.dialog_max_width * 0.24).clamp(80.0, 96.0);
    }

    (metrics.touch_target_min * 3.0).min(metrics.content_max_width * 0.42)
}

pub(crate) fn numeric_control_is_compact(metrics: &UiMetrics) -> bool {
    metrics.content_max_width <= 480.0
}
pub(crate) fn update_slider_interactions(
    mut sliders: Query<
        (
            Entity,
            &Interaction,
            &RelativeCursorPosition,
            &ComputedNode,
            &UiGlobalTransform,
            &mut UiSlider,
            Option<&InheritedVisibility>,
        ),
        (
            With<Button>,
            Without<DisabledButton>,
            Without<UiSliderTrack>,
        ),
    >,
    tracks: Query<
        (
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            Option<&InheritedVisibility>,
        ),
        With<UiSliderTrack>,
    >,
    parents: Query<&ChildOf>,
) {
    for (
        slider_entity,
        interaction,
        relative_cursor,
        slider_node,
        slider_transform,
        mut slider,
        slider_inherited_visibility,
    ) in &mut sliders
    {
        if *interaction != Interaction::Pressed
            || slider_inherited_visibility.is_some_and(|visibility| !visibility.get())
        {
            continue;
        }

        let Some(slider_normalized) = relative_cursor.normalized else {
            continue;
        };

        let slider_local_position = slider_normalized * slider_node.size;
        let slider_global_position = slider_transform
            .affine()
            .transform_point2(slider_local_position);

        let Some((_, track_node, track_transform, _)) =
            tracks
                .iter()
                .find(|(track_entity, _, _, track_inherited_visibility)| {
                    track_inherited_visibility.is_none_or(|visibility| visibility.get())
                        && parents
                            .iter_ancestors(*track_entity)
                            .any(|ancestor| ancestor == slider_entity)
                })
        else {
            continue;
        };

        let Some(normalized_track_position) =
            track_node.normalize_point(*track_transform, slider_global_position)
        else {
            continue;
        };
        let normalized_track_x = normalized_track_position.x;
        let next_value = slider_value_from_normalized_x(normalized_track_x, slider.min, slider.max);
        if slider.value != next_value {
            slider.value = next_value;
        }
    }
}

pub(crate) fn update_stepper_interactions(
    parents: Query<&ChildOf>,
    mut steppers: Query<&mut UiStepper>,
    buttons: Query<
        (Has<UiStepperDecrementButton>, Has<UiStepperIncrementButton>),
        (
            With<Button>,
            Without<DisabledButton>,
            Without<LoadingButton>,
        ),
    >,
    mut button_events: MessageReader<UiButtonEvent>,
) {
    for event in button_events.read() {
        if event.kind != UiButtonEventKind::Click {
            continue;
        }

        let Ok((is_decrement, is_increment)) = buttons.get(event.entity) else {
            continue;
        };
        if !is_decrement && !is_increment {
            continue;
        }

        let Some(stepper_entity) = parents
            .iter_ancestors(event.entity)
            .find(|ancestor| steppers.get(*ancestor).is_ok())
        else {
            continue;
        };

        let Ok(mut stepper) = steppers.get_mut(stepper_entity) else {
            continue;
        };

        let next_value = if is_increment {
            stepper_increment_value(stepper.value, stepper.min, stepper.max, stepper.step)
        } else {
            stepper_decrement_value(stepper.value, stepper.min, stepper.max, stepper.step)
        };
        if stepper.value != next_value {
            stepper.value = next_value;
        }
    }
}
pub(crate) fn sync_numeric_control_display(
    sliders: Query<(Entity, &UiSlider), Changed<UiSlider>>,
    steppers: Query<(Entity, &UiStepper), Changed<UiStepper>>,
    children: Query<&Children>,
    mut slider_fills: Query<&mut Node, With<UiSliderFill>>,
    mut value_texts: ParamSet<(
        Query<&mut Text, With<UiSliderValueText>>,
        Query<&mut Text, With<UiStepperValueText>>,
    )>,
) {
    {
        let mut slider_value_texts = value_texts.p0();
        for (slider_entity, slider) in &sliders {
            let width = percent(slider.ratio() * 100.0);
            let display = format_slider_value(slider.value);
            for child in children.iter_descendants(slider_entity) {
                if let Ok(mut fill_node) = slider_fills.get_mut(child)
                    && fill_node.width != width
                {
                    fill_node.width = width;
                }

                if let Ok(mut text) = slider_value_texts.get_mut(child)
                    && text.0 != display
                {
                    text.0 = display.clone();
                }
            }
        }
    }

    {
        let mut stepper_value_texts = value_texts.p1();
        for (stepper_entity, stepper) in &steppers {
            let display = stepper.value.to_string();
            for child in children.iter_descendants(stepper_entity) {
                if let Ok(mut text) = stepper_value_texts.get_mut(child)
                    && text.0 != display
                {
                    text.0 = display.clone();
                }
            }
        }
    }
}
pub(crate) fn ordered_slider_bounds(min: f32, max: f32) -> (f32, f32) {
    if min <= max { (min, max) } else { (max, min) }
}

pub(crate) fn clamp_slider_value(value: f32, min: f32, max: f32) -> f32 {
    if value.is_nan() {
        return min;
    }

    value.clamp(min, max)
}

pub(crate) fn slider_ratio(value: f32, min: f32, max: f32) -> f32 {
    let (min, max) = ordered_slider_bounds(min, max);
    let range = max - min;
    if range <= f32::EPSILON {
        return 0.0;
    }

    (clamp_slider_value(value, min, max) - min) / range
}

pub(crate) fn slider_value_from_normalized_x(normalized_x: f32, min: f32, max: f32) -> f32 {
    let (min, max) = ordered_slider_bounds(min, max);
    let ratio = (normalized_x + 0.5).clamp(0.0, 1.0);
    min + (max - min) * ratio
}

pub(crate) fn format_slider_value(value: f32) -> String {
    if value.fract().abs() < 0.05 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

pub(crate) fn ordered_stepper_bounds(min: i32, max: i32) -> (i32, i32) {
    if min <= max { (min, max) } else { (max, min) }
}

pub(crate) fn stepper_step(step: i32) -> i32 {
    step.abs().max(1)
}

pub(crate) fn clamp_stepper_value(value: i32, min: i32, max: i32) -> i32 {
    value.clamp(min, max)
}

pub(crate) fn stepper_increment_value(value: i32, min: i32, max: i32, step: i32) -> i32 {
    let (min, max) = ordered_stepper_bounds(min, max);
    clamp_stepper_value(value.saturating_add(stepper_step(step)), min, max)
}

pub(crate) fn stepper_decrement_value(value: i32, min: i32, max: i32, step: i32) -> i32 {
    let (min, max) = ordered_stepper_bounds(min, max);
    clamp_stepper_value(value.saturating_sub(stepper_step(step)), min, max)
}
