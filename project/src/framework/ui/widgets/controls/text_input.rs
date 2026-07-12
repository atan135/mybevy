use super::*;

#[derive(Component)]
pub(crate) struct UiTextInput;

#[derive(Clone, Debug, Default, Component)]
pub(crate) struct UiTextInputValue(pub String);

#[derive(Clone, Debug, Default, Component)]
pub(crate) struct UiTextInputCursor {
    pub(crate) position: usize,
    pub(crate) selection: Option<UiTextInputSelection>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UiTextInputSelection {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Clone, Copy, Debug, Component)]
pub(crate) struct UiTextInputMaxChars(pub usize);

#[derive(Component)]
pub(crate) struct ReadonlyTextInput;

#[derive(Component)]
pub(crate) struct DisabledTextInput;

#[derive(Clone, Debug, Component)]
pub(crate) struct UiTextInputRequired {
    message: String,
}

impl UiTextInputRequired {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Component)]
pub(crate) struct UiTextInputAlphanumeric {
    min_chars: usize,
    max_chars: usize,
    message: String,
}

impl UiTextInputAlphanumeric {
    pub(crate) fn new(min_chars: usize, max_chars: usize, message: impl Into<String>) -> Self {
        let min_chars = min_chars.min(max_chars);
        Self {
            min_chars,
            max_chars,
            message: message.into(),
        }
    }

    pub(crate) fn validate<'a>(&'a self, value: &str) -> Option<&'a str> {
        let char_count = value.chars().count();
        let valid = (self.min_chars..=self.max_chars).contains(&char_count)
            && value.chars().all(|chr| chr.is_ascii_alphanumeric());

        (!valid).then_some(self.message.as_str())
    }
}

#[derive(Component)]
pub(crate) struct UiTextInputError;

#[derive(Clone, Debug, Default, Component)]
pub(crate) struct UiTextInputHelperText(pub String);

#[derive(Clone, Debug, Default, Component)]
pub(crate) struct UiTextInputValidationMessage(pub String);

#[derive(Clone, Debug, Default, Component)]
pub(crate) struct UiTextInputPlaceholder(pub String);

#[derive(Component)]
pub(crate) struct UiTextInputText;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Component)]
pub(crate) enum UiTextInputTextPart {
    Plain,
    Selected,
    Tail,
}

#[derive(Component)]
pub(crate) struct UiTextInputCaret;

#[derive(Component)]
pub(crate) struct UiTextInputCaretMeasure;

#[derive(Clone, Copy, Debug, Component)]
pub(crate) struct UiTextInputFormMessage {
    input: Entity,
}

#[derive(Debug, Default, Resource)]
pub(crate) struct UiTextInputClipboard {
    text: String,
}

#[derive(Debug, Default, Resource)]
pub(crate) struct UiTextInputDiagnostics {
    tick: u64,
    focused_entity: Option<Entity>,
    focus_changed_tick: u64,
    #[cfg(target_os = "android")]
    android_soft_keyboard_visible: bool,
    #[cfg(target_os = "android")]
    android_text_input_entity: Option<Entity>,
    #[cfg(target_os = "android")]
    android_text_input_snapshot: Option<UiTextInputNativeState>,
    #[cfg(target_os = "android")]
    android_text_input_skip_pull_until_tick: u64,
    #[cfg(target_os = "android")]
    android_text_input_pressed_entity: Option<Entity>,
    #[cfg(target_os = "android")]
    android_text_input_pressed_tick: u64,
    missing_pointer_position_logged: HashSet<Entity>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) struct UiTextInputNativeState {
    pub(crate) text: String,
    pub(crate) selection_start: usize,
    pub(crate) selection_end: usize,
}

#[derive(Clone, Debug, Message)]
pub(crate) struct UiTextInputSubmitted {
    pub entity: Entity,
    pub value: String,
}
pub(crate) fn text_input(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    placeholder: impl Into<String>,
    value: impl Into<String>,
) -> impl Bundle {
    let value = value.into();
    let placeholder = placeholder.into();
    let initial_cursor_position = value.len();
    let display_text = if value.is_empty() {
        placeholder.clone()
    } else {
        value.clone()
    };
    let display_color = if value.is_empty() {
        theme.colors.text_muted
    } else {
        theme.colors.text_primary
    };

    (
        Button,
        FocusableButton,
        UiTextInput,
        RelativeCursorPosition::default(),
        UiTextInputValue(value),
        UiTextInputCursor {
            position: initial_cursor_position,
            selection: None,
        },
        UiTextInputPlaceholder(placeholder),
        UiThemeButtonNodeRole::TextInput,
        Node {
            width: percent(100),
            min_height: px(metrics.input_height),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexStart,
            padding: UiRect::axes(px(control_padding_x(metrics)), px(0)),
            border: UiRect::all(px(theme.panel.border)),
            border_radius: BorderRadius::all(px(theme.button.radius)),
            ..default()
        },
        BackgroundColor(text_input_background_color(
            theme,
            Interaction::None,
            false,
            false,
        )),
        BorderColor::all(text_input_border_color(
            theme,
            Interaction::None,
            false,
            false,
            false,
        )),
        children![
            (
                Text::new(""),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: theme.text.button,
                    ..default()
                },
                TextColor(display_color),
                TextLayout::new_with_no_wrap(),
                UiTextInputText,
                UiTextInputTextPart::Plain,
                UiThemeTextStyleRole::Button,
                children![
                    (
                        TextSpan::new(display_text),
                        TextFont {
                            font: fonts.regular.clone(),
                            font_size: theme.text.button,
                            ..default()
                        },
                        TextColor(display_color),
                        TextBackgroundColor(Color::NONE),
                        UiTextInputTextPart::Plain,
                        UiThemeTextStyleRole::Button,
                    ),
                    (
                        TextSpan::new(""),
                        TextFont {
                            font: fonts.regular.clone(),
                            font_size: theme.text.button,
                            ..default()
                        },
                        TextColor(theme.colors.text_primary),
                        TextBackgroundColor(Color::NONE),
                        UiTextInputTextPart::Selected,
                        UiThemeTextStyleRole::Button,
                    ),
                    (
                        TextSpan::new(""),
                        TextFont {
                            font: fonts.regular.clone(),
                            font_size: theme.text.button,
                            ..default()
                        },
                        TextColor(display_color),
                        TextBackgroundColor(Color::NONE),
                        UiTextInputTextPart::Tail,
                        UiThemeTextStyleRole::Button,
                    ),
                ],
            ),
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: px(control_padding_x(metrics)),
                    top: px((metrics.input_height - theme.text.button) * 0.5),
                    width: px(TEXT_INPUT_CARET_WIDTH),
                    height: px(theme.text.button),
                    ..default()
                },
                BackgroundColor(theme.colors.text_primary),
                Visibility::Hidden,
                UiTextInputCaret,
            ),
            (
                Text::new(""),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: theme.text.button,
                    ..default()
                },
                TextColor(Color::NONE),
                TextLayout::new_with_no_wrap(),
                UiTextInputCaretMeasure,
                UiThemeTextStyleRole::Button,
                Node {
                    position_type: PositionType::Absolute,
                    left: px(-10000),
                    top: px(-10000),
                    ..default()
                },
            ),
        ],
    )
}

pub(crate) fn text_input_form_message(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    input: Entity,
) -> impl Bundle {
    (
        Text::new(""),
        TextFont {
            font: fonts.regular.clone(),
            font_size: theme.text.caption,
            ..default()
        },
        TextColor(theme.colors.text_muted),
        UiTextInputFormMessage { input },
        UiThemeTextStyleRole::Caption,
    )
}
pub(crate) fn update_text_input_cursor_from_pointer(
    mut diagnostics: ResMut<UiTextInputDiagnostics>,
    mut text_inputs: Query<
        (
            Entity,
            &Interaction,
            &RelativeCursorPosition,
            &ComputedNode,
            &mut UiTextInputCursor,
            &UiTextInputValue,
            Has<DisabledTextInput>,
        ),
        (With<Button>, With<UiTextInput>),
    >,
    children: Query<&Children>,
    text_nodes: Query<&ComputedNode, With<UiTextInputText>>,
) {
    for (entity, interaction, relative_cursor, input_node, mut cursor, value, is_disabled) in
        &mut text_inputs
    {
        if *interaction != Interaction::Pressed || is_disabled {
            diagnostics.missing_pointer_position_logged.remove(&entity);
            continue;
        }

        #[cfg(target_os = "android")]
        {
            diagnostics.android_text_input_pressed_entity = Some(entity);
            diagnostics.android_text_input_pressed_tick = diagnostics.tick;
        }

        let Some(normalized) = relative_cursor.normalized else {
            if diagnostics.missing_pointer_position_logged.insert(entity) {
                debug!(
                    ?entity,
                    input_size = ?input_node.size,
                    content_size = ?input_node.content_size,
                    cursor_position = cursor.position,
                    value_len = value.0.len(),
                    "text input pressed without relative cursor position"
                );
            }
            continue;
        };
        diagnostics.missing_pointer_position_logged.remove(&entity);

        let text_width = children
            .get(entity)
            .ok()
            .and_then(|children| {
                children
                    .iter()
                    .filter_map(|child| text_nodes.get(child).ok())
                    .map(|node| node.size.x)
                    .find(|width| *width > 0.0)
            })
            .unwrap_or(input_node.content_size.x);
        let local_x = (normalized.x + 0.5) * input_node.size.x;
        let text_x = (local_x - input_node.padding.min_inset.x).clamp(0.0, text_width);
        let text_ratio = text_x / text_width.max(f32::EPSILON);
        cursor.position = text_input_cursor_position_from_ratio(&value.0, text_ratio);
        cursor.selection = None;
    }
}

#[cfg(target_os = "android")]
pub(crate) fn sync_android_text_input(
    focus_state: Res<UiFocusState>,
    mut diagnostics: ResMut<UiTextInputDiagnostics>,
    mut text_inputs: Query<
        (
            &mut UiTextInputValue,
            &mut UiTextInputCursor,
            Option<&UiTextInputMaxChars>,
            Has<ReadonlyTextInput>,
            Has<DisabledTextInput>,
        ),
        With<UiTextInput>,
    >,
) {
    let focused_text_input = focus_state.focused_entity.and_then(|entity| {
        text_inputs
            .get(entity)
            .ok()
            .and_then(|(_, _, _, _, is_disabled)| (!is_disabled).then_some(entity))
    });

    let Some(android_app) = bevy::android::ANDROID_APP.get() else {
        if focused_text_input.is_some() {
            warn!("cannot sync Android text input without AndroidApp");
        }
        return;
    };

    if diagnostics.android_text_input_entity != focused_text_input {
        if let Some(entity) = focused_text_input {
            let Ok((value, cursor, _, _, _)) = text_inputs.get(entity) else {
                return;
            };
            let state = ui_text_input_native_state_from_value(&value.0, cursor);

            android_app.set_ime_editor_info(
                bevy::android::android_activity::input::InputType::TYPE_CLASS_TEXT,
                bevy::android::android_activity::input::TextInputAction::Done,
                bevy::android::android_activity::input::ImeOptions::IME_FLAG_NO_FULLSCREEN,
            );
            android_app.set_text_input_state(state.to_android_text_input_state());
            android_app.show_soft_input(true);

            diagnostics.android_soft_keyboard_visible = true;
            diagnostics.android_text_input_entity = Some(entity);
            diagnostics.android_text_input_snapshot = Some(state.clone());
            diagnostics.android_text_input_skip_pull_until_tick =
                diagnostics.tick.saturating_add(1);
            debug!(
                ?entity,
                text = %state.text,
                selection_start = state.selection_start,
                selection_end = state.selection_end,
                "initialized Android text input state for focused field"
            );
        } else {
            if diagnostics.android_soft_keyboard_visible {
                android_app.hide_soft_input(false);
                debug!("requested Android soft keyboard hide after text input blur");
            }
            diagnostics.android_soft_keyboard_visible = false;
            diagnostics.android_text_input_entity = None;
            diagnostics.android_text_input_snapshot = None;
            diagnostics.android_text_input_skip_pull_until_tick = 0;
        }
        return;
    }

    let Some(entity) = focused_text_input else {
        return;
    };
    let text_input_pressed_this_tick = diagnostics.android_text_input_pressed_entity
        == Some(entity)
        && diagnostics.android_text_input_pressed_tick == diagnostics.tick;

    let Ok((mut value, mut cursor, max_chars, is_readonly, is_disabled)) =
        text_inputs.get_mut(entity)
    else {
        return;
    };
    if is_disabled {
        return;
    }

    let app_state = ui_text_input_native_state_from_value(&value.0, &cursor);
    if diagnostics.android_text_input_snapshot.as_ref() != Some(&app_state) {
        android_app.set_text_input_state(app_state.to_android_text_input_state());
        diagnostics.android_text_input_snapshot = Some(app_state.clone());
        diagnostics.android_text_input_skip_pull_until_tick = diagnostics.tick.saturating_add(1);
        if text_input_pressed_this_tick {
            android_app.show_soft_input(true);
            diagnostics.android_soft_keyboard_visible = true;
        }
        debug!(
            ?entity,
            text = %app_state.text,
            selection_start = app_state.selection_start,
            selection_end = app_state.selection_end,
            "pushed Bevy text input state to Android IME"
        );
        return;
    }

    if text_input_pressed_this_tick {
        android_app.show_soft_input(true);
        diagnostics.android_soft_keyboard_visible = true;
        debug!(
            ?entity,
            "requested Android soft keyboard show after focused text input press"
        );
    }

    if diagnostics.tick <= diagnostics.android_text_input_skip_pull_until_tick {
        return;
    }

    let native_state =
        UiTextInputNativeState::from_android_text_input_state(android_app.text_input_state());
    if diagnostics.android_text_input_snapshot.as_ref() == Some(&native_state) {
        return;
    }

    if is_readonly {
        android_app.set_text_input_state(app_state.to_android_text_input_state());
        diagnostics.android_text_input_snapshot = Some(app_state);
        diagnostics.android_text_input_skip_pull_until_tick = diagnostics.tick.saturating_add(1);
        debug!(
            ?entity,
            ime_text = %native_state.text,
            "rejected Android IME edit for readonly text input"
        );
        return;
    }

    let before_value = value.0.clone();
    let before_cursor = cursor.position;
    let before_selection = cursor.selection;
    apply_native_text_input_state(
        &mut value.0,
        &mut cursor,
        native_state,
        max_chars.map(|max_chars| max_chars.0),
    );
    let applied_state = ui_text_input_native_state_from_value(&value.0, &cursor);

    if value.0 != before_value
        || cursor.position != before_cursor
        || cursor.selection != before_selection
    {
        debug!(
            ?entity,
            before_value = %before_value,
            after_value = %value.0,
            before_cursor,
            after_cursor = cursor.position,
            before_selection = ?before_selection,
            after_selection = ?cursor.selection,
            "pulled Android IME text input state into Bevy"
        );
    }

    if diagnostics.android_text_input_snapshot.as_ref() != Some(&applied_state) {
        android_app.set_text_input_state(applied_state.to_android_text_input_state());
    }
    diagnostics.android_text_input_snapshot = Some(applied_state);
}

#[cfg(not(target_os = "android"))]
pub(crate) fn sync_android_text_input() {}

pub(crate) fn handle_text_input_keyboard(
    mut keyboard_inputs: MessageReader<KeyboardInput>,
    key_codes: Res<ButtonInput<KeyCode>>,
    focus_state: Res<UiFocusState>,
    mut diagnostics: ResMut<UiTextInputDiagnostics>,
    mut text_inputs: Query<
        (
            &mut UiTextInputValue,
            &mut UiTextInputCursor,
            Option<&UiTextInputMaxChars>,
            Has<ReadonlyTextInput>,
            Has<DisabledTextInput>,
        ),
        With<UiTextInput>,
    >,
    mut clipboard: ResMut<UiTextInputClipboard>,
    mut submissions: MessageWriter<UiTextInputSubmitted>,
) {
    diagnostics.tick = diagnostics.tick.wrapping_add(1);
    let previous_focused = diagnostics.focused_entity;
    if previous_focused != focus_state.focused_entity {
        let previous_was_text_input =
            previous_focused.is_some_and(|entity| text_inputs.contains(entity));
        let focused_is_text_input = focus_state
            .focused_entity
            .is_some_and(|entity| text_inputs.contains(entity));
        if previous_was_text_input || focused_is_text_input {
            debug!(
                tick = diagnostics.tick,
                ?previous_focused,
                focused_entity = ?focus_state.focused_entity,
                "text input focus changed"
            );
        }
        diagnostics.focused_entity = focus_state.focused_entity;
        diagnostics.focus_changed_tick = diagnostics.tick;
    }
    let focus_ticks_ago = diagnostics
        .tick
        .saturating_sub(diagnostics.focus_changed_tick);

    let Some(focused_entity) = focus_state.focused_entity else {
        for _ in keyboard_inputs.read() {}
        return;
    };

    let Ok((mut value, mut cursor, max_chars, is_readonly, is_disabled)) =
        text_inputs.get_mut(focused_entity)
    else {
        for _ in keyboard_inputs.read() {}
        return;
    };

    let mode = UiTextInputEditMode {
        readonly: is_readonly,
        disabled: is_disabled,
        max_chars: max_chars.map(|max_chars| max_chars.0),
    };

    for keyboard_input in keyboard_inputs.read() {
        if !keyboard_input.state.is_pressed() {
            continue;
        }

        let before_value = value.0.clone();
        let before_cursor = cursor.position;
        let before_selection = cursor.selection;
        let edit_event = ui_text_input_edit_event(keyboard_input, &key_codes);
        if should_skip_keyboard_text_edit_for_native_ime(&edit_event) {
            debug!(
                tick = diagnostics.tick,
                ?focused_entity,
                key_code = ?keyboard_input.key_code,
                logical_key = ?keyboard_input.logical_key,
                text = ?keyboard_input.text.as_deref(),
                "skipped keyboard text edit while Android IME state is authoritative"
            );
            continue;
        }

        match edit_event {
            UiTextInputEditEvent::Submit => {
                if is_readonly || is_disabled {
                    continue;
                }

                submissions.write(UiTextInputSubmitted {
                    entity: focused_entity,
                    value: value.0.clone(),
                });
            }
            UiTextInputEditEvent::Copy => {
                if is_disabled {
                    continue;
                }

                clipboard.text =
                    selected_text(&value.0, &cursor).unwrap_or_else(|| value.0.clone());
                write_system_clipboard_text(&clipboard.text);
            }
            UiTextInputEditEvent::Paste => {
                let clipboard_text = read_system_clipboard_text()
                    .filter(|text| !text.is_empty())
                    .unwrap_or_else(|| clipboard.text.clone());
                apply_text_input_edit(
                    &mut value.0,
                    &mut cursor,
                    UiTextInputEditAction::Paste(&clipboard_text),
                    mode,
                );
            }
            UiTextInputEditEvent::Edit(action) => {
                apply_text_input_edit(&mut value.0, &mut cursor, action, mode);
            }
            UiTextInputEditEvent::None => {}
        }

        if should_log_text_input_keyboard_event(
            keyboard_input,
            focus_ticks_ago,
            &before_value,
            &value.0,
            before_cursor,
            cursor.position,
            before_selection,
            cursor.selection,
        ) {
            debug!(
                tick = diagnostics.tick,
                ?focused_entity,
                focus_ticks_ago,
                key_code = ?keyboard_input.key_code,
                logical_key = ?keyboard_input.logical_key,
                text = ?keyboard_input.text.as_deref(),
                before_value = %before_value,
                after_value = %value.0,
                before_cursor,
                after_cursor = cursor.position,
                before_selection = ?before_selection,
                after_selection = ?cursor.selection,
                "text input keyboard event"
            );
        }
    }
}

pub(crate) fn sync_text_input_display(
    theme: Res<UiTheme>,
    focus_state: Res<UiFocusState>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    text_inputs: Query<
        (
            &UiTextInputValue,
            &UiTextInputPlaceholder,
            &UiTextInputCursor,
            Has<DisabledTextInput>,
            Option<&UiResolvedInputStyle>,
        ),
        With<UiTextInput>,
    >,
    mut roots: Query<(Entity, &mut Text, &mut TextColor), With<UiTextInputText>>,
    mut measures: Query<&mut Text, (With<UiTextInputCaretMeasure>, Without<UiTextInputText>)>,
    mut spans: Query<
        (
            &mut TextSpan,
            &UiTextInputTextPart,
            &mut TextColor,
            Option<&mut TextBackgroundColor>,
        ),
        Without<UiTextInputText>,
    >,
) {
    for (root_entity, mut root_text, mut root_text_color) in &mut roots {
        let Some(input_entity) = parents
            .iter_ancestors(root_entity)
            .find(|ancestor| text_inputs.get(*ancestor).is_ok())
        else {
            continue;
        };

        let Ok((value, placeholder, cursor, is_disabled, scoped_style)) =
            text_inputs.get(input_entity)
        else {
            continue;
        };

        let is_focused = focus_state.focused_entity == Some(input_entity);
        let (display, focused_caret_prefix) = if is_focused && !is_disabled {
            let segments = text_input_segments(&value.0, cursor);
            (segments.display, Some(segments.caret_prefix))
        } else if value.0.is_empty() && !is_focused {
            (UiTextInputDisplay::placeholder(placeholder.0.clone()), None)
        } else {
            (UiTextInputDisplay::plain(value.0.clone()), None)
        };
        let color = if is_disabled || value.0.is_empty() && !is_focused {
            scoped_style.map_or(theme.colors.text_muted, |style| style.placeholder)
        } else {
            scoped_style.map_or(theme.colors.text_primary, |style| style.text)
        };
        let selected_text_color =
            scoped_style.map_or(theme.colors.screen_background, |style| style.selection_text);
        let selected_background = scoped_style
            .map_or(theme.colors.primary_button.focused, |style| {
                style.selection_background
            });

        if !root_text.0.is_empty() {
            root_text.0.clear();
        }
        if root_text_color.0 != color {
            root_text_color.0 = color;
        }

        for child in children.iter_descendants(input_entity) {
            let Ok(mut measure_text) = measures.get_mut(child) else {
                continue;
            };
            let next_measure = focused_caret_prefix
                .clone()
                .unwrap_or_else(|| text_input_caret_prefix(&value.0, cursor));
            if measure_text.0 != next_measure {
                measure_text.0 = next_measure;
            }
        }

        let Ok(children) = children.get(root_entity) else {
            continue;
        };

        for child in children {
            let Ok((mut span, part, mut span_color, background)) = spans.get_mut(*child) else {
                continue;
            };

            let next_text = match part {
                UiTextInputTextPart::Plain => display.plain.as_str(),
                UiTextInputTextPart::Selected => display.selected.as_str(),
                UiTextInputTextPart::Tail => display.tail.as_str(),
            };
            if span.as_str() != next_text {
                span.0 = next_text.to_string();
            }

            let next_color = match part {
                UiTextInputTextPart::Selected if !display.selected.is_empty() => {
                    selected_text_color
                }
                _ => color,
            };
            if span_color.0 != next_color {
                span_color.0 = next_color;
            }

            if let Some(mut background) = background {
                let next_background = match part {
                    UiTextInputTextPart::Selected if !display.selected.is_empty() => {
                        selected_background
                    }
                    _ => Color::NONE,
                };
                if background.0 != next_background {
                    background.0 = next_background;
                }
            }
        }
    }
}

pub(crate) fn sync_text_input_caret(
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    focus_state: Res<UiFocusState>,
    children: Query<&Children>,
    text_inputs: Query<
        (
            Entity,
            &UiTextInputValue,
            &UiTextInputCursor,
            Has<DisabledTextInput>,
            Option<&UiResolvedInputStyle>,
        ),
        With<UiTextInput>,
    >,
    measures: Query<&TextLayoutInfo, With<UiTextInputCaretMeasure>>,
    mut carets: Query<(&mut Node, &mut BackgroundColor, &mut Visibility), With<UiTextInputCaret>>,
) {
    for (input_entity, _value, cursor, is_disabled, scoped_style) in &text_inputs {
        let is_focused = focus_state.focused_entity == Some(input_entity);
        let caret_visible = is_focused && !is_disabled && cursor.selection.is_none();
        let caret_x = children
            .iter_descendants(input_entity)
            .find_map(|child| {
                measures
                    .get(child)
                    .ok()
                    .map(|layout| control_padding_x(&metrics) + layout.size.x)
            })
            .unwrap_or_else(|| control_padding_x(&metrics));

        for child in children.iter_descendants(input_entity) {
            let Ok((mut node, mut background, mut visibility)) = carets.get_mut(child) else {
                continue;
            };

            let next_visibility = if caret_visible {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *visibility != next_visibility {
                *visibility = next_visibility;
            }
            if node.left != px(caret_x) {
                node.left = px(caret_x);
            }
            let caret_color = scoped_style.map_or(theme.colors.text_primary, |style| style.text);
            if background.0 != caret_color {
                background.0 = caret_color;
            }
            if node.width != px(TEXT_INPUT_CARET_WIDTH) {
                node.width = px(TEXT_INPUT_CARET_WIDTH);
            }
        }
    }
}

pub(crate) fn sync_text_input_form_messages(
    theme: Res<UiTheme>,
    text_inputs: Query<(
        &UiTextInputValue,
        Option<&UiTextInputHelperText>,
        Option<&UiTextInputValidationMessage>,
        Option<&UiTextInputAlphanumeric>,
        Option<&UiTextInputRequired>,
        Has<UiTextInputError>,
        Has<DisabledTextInput>,
        Option<&UiResolvedInputStyle>,
    )>,
    mut messages: Query<(&UiTextInputFormMessage, &mut Text, &mut TextColor)>,
) {
    for (message, mut text, mut text_color) in &mut messages {
        let Ok((
            value,
            helper_text,
            validation_message,
            alphanumeric,
            required,
            has_error,
            is_disabled,
            scoped_style,
        )) = text_inputs.get(message.input)
        else {
            continue;
        };

        let state = text_input_form_state(
            &value.0,
            helper_text.map(|helper| helper.0.as_str()),
            text_input_validation_message(&value.0, validation_message, alphanumeric),
            required,
            has_error,
        );
        let display = state.message.unwrap_or_default();
        let color = if is_disabled {
            scoped_style.map_or(theme.colors.text_muted, |style| style.placeholder)
        } else if state.is_error {
            scoped_style.map_or(theme.colors.text_error, |style| style.error_text)
        } else {
            scoped_style.map_or(theme.colors.text_muted, |style| style.placeholder)
        };

        if text.0 != display {
            text.0 = display;
        }
        if text_color.0 != color {
            text_color.0 = color;
        }
    }
}
pub(crate) fn update_text_input_visuals(
    theme: Res<UiTheme>,
    mut text_inputs: Query<
        (
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
            Has<FocusedButton>,
            Has<DisabledTextInput>,
            Has<UiTextInputError>,
            &UiTextInputValue,
            Option<&UiTextInputValidationMessage>,
            Option<&UiTextInputAlphanumeric>,
            Option<&UiTextInputRequired>,
            Option<&UiResolvedInputStyle>,
        ),
        (With<Button>, With<UiTextInput>),
    >,
) {
    for (
        interaction,
        mut background,
        mut border,
        is_focused,
        is_disabled,
        has_error,
        value,
        validation_message,
        alphanumeric,
        required,
        scoped_style,
    ) in &mut text_inputs
    {
        let is_error = text_input_has_error(
            &value.0,
            text_input_validation_message(&value.0, validation_message, alphanumeric),
            required,
            has_error,
        );
        let background_color = scoped_style.map_or_else(
            || text_input_background_color(&theme, *interaction, is_focused, is_disabled),
            |style| {
                text_input_background_color_from_tokens(
                    style,
                    *interaction,
                    is_focused,
                    is_disabled,
                )
            },
        );
        if background.0 != background_color {
            *background = BackgroundColor(background_color);
        }

        let next_border = BorderColor::all(scoped_style.map_or_else(
            || text_input_border_color(&theme, *interaction, is_focused, is_disabled, is_error),
            |style| {
                text_input_border_color_from_tokens(
                    style,
                    *interaction,
                    is_focused,
                    is_disabled,
                    is_error,
                )
            },
        ));
        if *border != next_border {
            *border = next_border;
        }
    }
}

pub(crate) fn text_input_background_color(
    theme: &UiTheme,
    interaction: Interaction,
    is_focused: bool,
    is_disabled: bool,
) -> Color {
    if is_disabled {
        return theme.colors.secondary_button.disabled;
    }

    match interaction {
        Interaction::Pressed => theme.colors.secondary_button.pressed,
        Interaction::Hovered => theme.colors.secondary_button.hovered,
        Interaction::None if is_focused => theme.colors.secondary_button.focused,
        Interaction::None => theme.colors.secondary_button.idle,
    }
}

pub(crate) fn text_input_background_color_from_tokens(
    style: &UiResolvedInputStyle,
    interaction: Interaction,
    is_focused: bool,
    is_disabled: bool,
) -> Color {
    if is_disabled {
        return style.backgrounds.disabled;
    }

    match interaction {
        Interaction::Pressed => style.backgrounds.pressed,
        Interaction::Hovered => style.backgrounds.hovered,
        Interaction::None if is_focused => style.backgrounds.focused,
        Interaction::None => style.backgrounds.idle,
    }
}

pub(crate) fn text_input_border_color(
    theme: &UiTheme,
    interaction: Interaction,
    is_focused: bool,
    is_disabled: bool,
    is_error: bool,
) -> Color {
    if is_disabled {
        return theme.colors.secondary_button.disabled;
    }

    if is_error {
        return theme.colors.error;
    }

    if is_focused {
        return theme.colors.primary_button.focused;
    }

    match interaction {
        Interaction::Pressed => theme.colors.primary_button.pressed,
        Interaction::Hovered => theme.colors.secondary_button.focused,
        Interaction::None => theme.colors.panel_border,
    }
}

pub(crate) fn text_input_border_color_from_tokens(
    style: &UiResolvedInputStyle,
    interaction: Interaction,
    is_focused: bool,
    is_disabled: bool,
    is_error: bool,
) -> Color {
    if is_disabled {
        return style.border_disabled;
    }
    if is_error {
        return style.border_error;
    }
    if is_focused {
        return style.border_focused;
    }
    match interaction {
        Interaction::Pressed => style.border_pressed,
        Interaction::Hovered => style.border_hovered,
        Interaction::None => style.border_idle,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UiTextInputFormState {
    pub(crate) message: Option<String>,
    pub(crate) is_error: bool,
}

pub(crate) fn text_input_form_state(
    value: &str,
    helper_text: Option<&str>,
    validation_message: Option<&str>,
    required: Option<&UiTextInputRequired>,
    has_error: bool,
) -> UiTextInputFormState {
    if let Some(message) = validation_message.filter(|message| !message.is_empty()) {
        return UiTextInputFormState {
            message: Some(message.to_string()),
            is_error: true,
        };
    }

    if has_error {
        return UiTextInputFormState {
            message: None,
            is_error: true,
        };
    }

    if let Some(required) = required
        && value.is_empty()
    {
        return UiTextInputFormState {
            message: (!required.message.is_empty()).then(|| required.message.clone()),
            is_error: true,
        };
    }

    UiTextInputFormState {
        message: helper_text
            .filter(|message| !message.is_empty())
            .map(str::to_string),
        is_error: false,
    }
}

pub(crate) fn text_input_has_error(
    value: &str,
    validation_message: Option<&str>,
    required: Option<&UiTextInputRequired>,
    has_error: bool,
) -> bool {
    text_input_form_state(value, None, validation_message, required, has_error).is_error
}

pub(crate) fn text_input_validation_message<'a>(
    value: &str,
    validation_message: Option<&'a UiTextInputValidationMessage>,
    alphanumeric: Option<&'a UiTextInputAlphanumeric>,
) -> Option<&'a str> {
    validation_message
        .map(|validation| validation.0.as_str())
        .filter(|message| !message.is_empty())
        .or_else(|| alphanumeric.and_then(|rule| rule.validate(value)))
}
#[derive(Clone, Copy)]
pub(crate) struct UiTextInputEditMode {
    pub(crate) readonly: bool,
    pub(crate) disabled: bool,
    pub(crate) max_chars: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiTextInputEditAction<'a> {
    Insert(&'a str),
    Paste(&'a str),
    Backspace,
    Delete,
    MoveLeft,
    MoveRight,
    MoveHome,
    MoveEnd,
    SelectAll,
}

pub(crate) enum UiTextInputEditEvent<'a> {
    Edit(UiTextInputEditAction<'a>),
    Copy,
    Paste,
    Submit,
    None,
}

pub(crate) fn should_skip_keyboard_text_edit_for_native_ime(
    edit_event: &UiTextInputEditEvent<'_>,
) -> bool {
    cfg!(target_os = "android")
        && matches!(
            edit_event,
            UiTextInputEditEvent::Edit(
                UiTextInputEditAction::Insert(_)
                    | UiTextInputEditAction::Paste(_)
                    | UiTextInputEditAction::Backspace
                    | UiTextInputEditAction::Delete
            ) | UiTextInputEditEvent::Paste
        )
}

pub(crate) fn ui_text_input_edit_event<'a>(
    keyboard_input: &'a KeyboardInput,
    key_codes: &ButtonInput<KeyCode>,
) -> UiTextInputEditEvent<'a> {
    let is_control_pressed = key_codes.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);

    if is_control_pressed {
        match keyboard_input.key_code {
            KeyCode::KeyA => return UiTextInputEditEvent::Edit(UiTextInputEditAction::SelectAll),
            KeyCode::KeyC => return UiTextInputEditEvent::Copy,
            KeyCode::KeyV => return UiTextInputEditEvent::Paste,
            _ => {}
        }
    }

    match &keyboard_input.logical_key {
        Key::Enter => UiTextInputEditEvent::Submit,
        Key::Backspace => UiTextInputEditEvent::Edit(UiTextInputEditAction::Backspace),
        Key::Delete => UiTextInputEditEvent::Edit(UiTextInputEditAction::Delete),
        Key::ArrowLeft => UiTextInputEditEvent::Edit(UiTextInputEditAction::MoveLeft),
        Key::ArrowRight => UiTextInputEditEvent::Edit(UiTextInputEditAction::MoveRight),
        Key::Home => UiTextInputEditEvent::Edit(UiTextInputEditAction::MoveHome),
        Key::End => UiTextInputEditEvent::Edit(UiTextInputEditAction::MoveEnd),
        Key::Space => {
            if is_control_pressed {
                UiTextInputEditEvent::None
            } else {
                UiTextInputEditEvent::Edit(UiTextInputEditAction::Insert(
                    keyboard_input.text.as_deref().unwrap_or(" "),
                ))
            }
        }
        _ => {
            if is_control_pressed {
                return UiTextInputEditEvent::None;
            }

            if let Some(inserted_text) = keyboard_input
                .text
                .as_deref()
                .filter(|text| text.chars().all(is_printable_char))
            {
                UiTextInputEditEvent::Edit(UiTextInputEditAction::Insert(inserted_text))
            } else {
                UiTextInputEditEvent::None
            }
        }
    }
}

pub(crate) fn should_log_text_input_keyboard_event(
    keyboard_input: &KeyboardInput,
    focus_ticks_ago: u64,
    before_value: &str,
    after_value: &str,
    before_cursor: usize,
    after_cursor: usize,
    before_selection: Option<UiTextInputSelection>,
    after_selection: Option<UiTextInputSelection>,
) -> bool {
    if focus_ticks_ago > TEXT_INPUT_FOCUS_SWITCH_LOG_TICKS {
        return false;
    }

    keyboard_input.text.is_some()
        || before_value != after_value
        || before_cursor != after_cursor
        || before_selection != after_selection
}

#[cfg(target_os = "android")]
impl UiTextInputNativeState {
    fn from_android_text_input_state(
        state: bevy::android::android_activity::input::TextInputState,
    ) -> Self {
        Self {
            text: state.text,
            selection_start: state.selection.start,
            selection_end: state.selection.end,
        }
    }

    fn to_android_text_input_state(
        &self,
    ) -> bevy::android::android_activity::input::TextInputState {
        bevy::android::android_activity::input::TextInputState {
            text: self.text.clone(),
            selection: bevy::android::android_activity::input::TextSpan {
                start: self.selection_start,
                end: self.selection_end,
            },
            compose_region: None,
        }
    }
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn ui_text_input_native_state_from_value(
    value: &str,
    cursor: &UiTextInputCursor,
) -> UiTextInputNativeState {
    let mut cursor = cursor.clone();
    clamp_text_input_cursor(value, &mut cursor);
    let (selection_start, selection_end) = selection_range(&cursor)
        .map(|selection| (selection.start, selection.end))
        .unwrap_or((cursor.position, cursor.position));

    UiTextInputNativeState {
        text: value.to_string(),
        selection_start,
        selection_end,
    }
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn apply_native_text_input_state(
    value: &mut String,
    cursor: &mut UiTextInputCursor,
    state: UiTextInputNativeState,
    max_chars: Option<usize>,
) {
    let text = limit_text_input_text(state.text, max_chars);
    let selection_start = native_selection_to_char_boundary(&text, state.selection_start);
    let selection_end = native_selection_to_char_boundary(&text, state.selection_end);
    let (selection_start, selection_end) = if selection_start <= selection_end {
        (selection_start, selection_end)
    } else {
        (selection_end, selection_start)
    };

    *value = text;
    cursor.position = selection_end;
    cursor.selection = (selection_start < selection_end).then_some(UiTextInputSelection {
        start: selection_start,
        end: selection_end,
    });
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn limit_text_input_text(text: String, max_chars: Option<usize>) -> String {
    let Some(max_chars) = max_chars else {
        return text;
    };

    text.chars().take(max_chars).collect()
}

pub(crate) fn apply_text_input_edit(
    value: &mut String,
    cursor: &mut UiTextInputCursor,
    action: UiTextInputEditAction,
    mode: UiTextInputEditMode,
) {
    clamp_text_input_cursor(value, cursor);

    if mode.disabled {
        return;
    }

    match action {
        UiTextInputEditAction::MoveLeft => {
            cursor.selection = None;
            cursor.position = previous_char_boundary(value, cursor.position);
        }
        UiTextInputEditAction::MoveRight => {
            cursor.selection = None;
            cursor.position = next_char_boundary(value, cursor.position);
        }
        UiTextInputEditAction::MoveHome => {
            cursor.selection = None;
            cursor.position = 0;
        }
        UiTextInputEditAction::MoveEnd => {
            cursor.selection = None;
            cursor.position = value.len();
        }
        UiTextInputEditAction::SelectAll => {
            cursor.position = value.len();
            cursor.selection = (!value.is_empty()).then_some(UiTextInputSelection {
                start: 0,
                end: value.len(),
            });
        }
        UiTextInputEditAction::Insert(text) | UiTextInputEditAction::Paste(text) => {
            if mode.readonly {
                return;
            }

            replace_selection_or_insert(value, cursor, text, mode.max_chars);
        }
        UiTextInputEditAction::Backspace => {
            if mode.readonly {
                return;
            }

            if delete_selection(value, cursor) {
                return;
            }

            let delete_from = previous_char_boundary(value, cursor.position);
            if delete_from != cursor.position {
                value.replace_range(delete_from..cursor.position, "");
                cursor.position = delete_from;
            }
        }
        UiTextInputEditAction::Delete => {
            if mode.readonly {
                return;
            }

            if delete_selection(value, cursor) {
                return;
            }

            let delete_to = next_char_boundary(value, cursor.position);
            if delete_to != cursor.position {
                value.replace_range(cursor.position..delete_to, "");
            }
        }
    }
}

pub(crate) fn replace_selection_or_insert(
    value: &mut String,
    cursor: &mut UiTextInputCursor,
    text: &str,
    max_chars: Option<usize>,
) {
    let (selection_start, selection_end) = selection_range(cursor)
        .map(|selection| (selection.start, selection.end))
        .unwrap_or((cursor.position, cursor.position));
    let selected_chars = value[selection_start..selection_end].chars().count();
    let current_chars = value.chars().count();
    let available_chars = max_chars
        .map(|max_chars| max_chars.saturating_sub(current_chars.saturating_sub(selected_chars)))
        .unwrap_or(usize::MAX);
    let inserted_text = text
        .chars()
        .filter(|chr| is_printable_char(*chr))
        .take(available_chars)
        .collect::<String>();

    value.replace_range(selection_start..selection_end, &inserted_text);
    cursor.position = selection_start + inserted_text.len();
    cursor.selection = None;
}

pub(crate) fn delete_selection(value: &mut String, cursor: &mut UiTextInputCursor) -> bool {
    let Some(selection) = selection_range(cursor) else {
        cursor.selection = None;
        return false;
    };

    value.replace_range(selection.start..selection.end, "");
    cursor.position = selection.start;
    cursor.selection = None;
    true
}

pub(crate) fn selected_text(value: &str, cursor: &UiTextInputCursor) -> Option<String> {
    let selection = selection_range(cursor)?;
    Some(value[selection.start..selection.end].to_string())
}

#[cfg(not(target_os = "android"))]
pub(crate) fn read_system_clipboard_text() -> Option<String> {
    arboard::Clipboard::new()
        .ok()
        .and_then(|mut clipboard| clipboard.get_text().ok())
}

#[cfg(target_os = "android")]
pub(crate) fn read_system_clipboard_text() -> Option<String> {
    None
}

#[cfg(not(target_os = "android"))]
pub(crate) fn write_system_clipboard_text(text: &str) {
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_text(text.to_string());
    }
}

#[cfg(target_os = "android")]
pub(crate) fn write_system_clipboard_text(_text: &str) {}

pub(crate) fn selection_range(cursor: &UiTextInputCursor) -> Option<UiTextInputSelection> {
    cursor
        .selection
        .filter(|selection| selection.start < selection.end)
}

pub(crate) fn clamp_text_input_cursor(value: &str, cursor: &mut UiTextInputCursor) {
    cursor.position = nearest_char_boundary(value, cursor.position.min(value.len()));

    cursor.selection = cursor.selection.and_then(|selection| {
        let start = nearest_char_boundary(value, selection.start.min(value.len()));
        let end = nearest_char_boundary(value, selection.end.min(value.len()));
        (start < end).then_some(UiTextInputSelection { start, end })
    });
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn native_selection_to_char_boundary(value: &str, position: usize) -> usize {
    nearest_char_boundary(value, position.min(value.len()))
}

pub(crate) fn previous_char_boundary(value: &str, position: usize) -> usize {
    if position == 0 {
        return 0;
    }

    value[..position]
        .char_indices()
        .last()
        .map(|(index, _)| index)
        .unwrap_or(0)
}

pub(crate) fn next_char_boundary(value: &str, position: usize) -> usize {
    value[position..]
        .char_indices()
        .nth(1)
        .map(|(offset, _)| position + offset)
        .unwrap_or(value.len())
}

pub(crate) fn nearest_char_boundary(value: &str, position: usize) -> usize {
    let mut position = position.min(value.len());
    while position > 0 && !value.is_char_boundary(position) {
        position -= 1;
    }
    position
}

pub(crate) fn text_input_cursor_position_from_ratio(value: &str, ratio: f32) -> usize {
    if value.is_empty() {
        return 0;
    }

    let char_count = value.chars().count();
    let char_index = (ratio.clamp(0.0, 1.0) * char_count as f32).round() as usize;
    value
        .char_indices()
        .map(|(index, _)| index)
        .nth(char_index)
        .unwrap_or(value.len())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UiTextInputDisplay {
    pub(crate) plain: String,
    pub(crate) selected: String,
    pub(crate) tail: String,
}

impl UiTextInputDisplay {
    fn plain(text: String) -> Self {
        Self {
            plain: text,
            selected: String::new(),
            tail: String::new(),
        }
    }

    fn placeholder(text: String) -> Self {
        Self::plain(text)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UiTextInputSegments {
    pub(crate) display: UiTextInputDisplay,
    pub(crate) caret_prefix: String,
}

pub(crate) fn text_input_segments(value: &str, cursor: &UiTextInputCursor) -> UiTextInputSegments {
    let cursor_position = nearest_char_boundary(value, cursor.position.min(value.len()));

    if let Some(selection) = selection_range(cursor) {
        let start = nearest_char_boundary(value, selection.start.min(value.len()));
        let end = nearest_char_boundary(value, selection.end.min(value.len()));
        let caret_end = if cursor_position <= start { start } else { end };

        return UiTextInputSegments {
            display: UiTextInputDisplay {
                plain: value[..start].to_string(),
                selected: value[start..end].to_string(),
                tail: value[end..].to_string(),
            },
            caret_prefix: value[..caret_end].to_string(),
        };
    }

    UiTextInputSegments {
        display: UiTextInputDisplay {
            plain: value[..cursor_position].to_string(),
            selected: String::new(),
            tail: value[cursor_position..].to_string(),
        },
        caret_prefix: value[..cursor_position].to_string(),
    }
}

pub(crate) fn text_input_caret_prefix(value: &str, cursor: &UiTextInputCursor) -> String {
    text_input_segments(value, cursor).caret_prefix
}

pub(crate) fn is_printable_char(chr: char) -> bool {
    let is_in_private_use_area = ('\u{e000}'..='\u{f8ff}').contains(&chr)
        || ('\u{f0000}'..='\u{ffffd}').contains(&chr)
        || ('\u{100000}'..='\u{10fffd}').contains(&chr);

    !is_in_private_use_area && !chr.is_ascii_control()
}
