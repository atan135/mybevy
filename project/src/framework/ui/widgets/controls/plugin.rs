use super::*;

pub(crate) struct UiWidgetsPlugin;

impl Plugin for UiWidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiScrollPlugin)
            .init_resource::<UiTextInputClipboard>()
            .init_resource::<UiTextInputDiagnostics>()
            .add_message::<UiButtonEvent>()
            .add_message::<UiTextInputSubmitted>()
            .add_systems(
                PreUpdate,
                emit_ui_button_events.after(PickingSystems::Hover),
            )
            .add_systems(
                Update,
                handle_text_input_keyboard
                    .after(UiFocusSystems::SyncFocusedMarkers)
                    .before(UiFocusSystems::Visuals),
            )
            .add_systems(
                Update,
                (
                    update_text_input_cursor_from_pointer,
                    sync_android_text_input
                        .after(update_text_input_cursor_from_pointer)
                        .before(sync_text_input_display),
                    update_selection_control_interactions,
                    update_slider_interactions,
                    update_stepper_interactions,
                    sync_selection_control_visuals,
                    sync_text_input_display,
                    sync_text_input_form_messages,
                    sync_numeric_control_display,
                    sync_icon_button_accessible_labels.after(UiI18nSystems::Refresh),
                    sync_icon_button_nodes,
                    sync_button_style_labels,
                    update_button_visuals,
                    update_icon_button_visuals,
                    update_text_input_visuals,
                )
                    .in_set(UiFocusSystems::Visuals),
            )
            .add_systems(
                PostUpdate,
                (
                    sync_text_input_caret,
                    sync_ui_icon_asset_status,
                    crate::framework::ui::widgets::image::update_ui_images,
                )
                    .after(UiSystems::PostLayout),
            );
    }
}
