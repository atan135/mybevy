use bevy::{prelude::*, window::PrimaryWindow};

use crate::game::ui::style::UiTheme;

pub(in crate::game) struct UiViewportPlugin;

impl Plugin for UiViewportPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiViewport>()
            .init_resource::<UiMetrics>()
            .add_systems(Update, update_ui_viewport_metrics);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Resource)]
pub(in crate::game) struct UiViewport {
    pub logical_width: f32,
    pub logical_height: f32,
    pub width_class: UiWidthClass,
    pub height_class: UiHeightClass,
    pub orientation: UiOrientation,
    pub input_mode: UiInputMode,
    pub safe_area: UiSafeArea,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum UiWidthClass {
    Compact,
    Medium,
    Expanded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum UiHeightClass {
    Short,
    Regular,
    Tall,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum UiOrientation {
    Portrait,
    Landscape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(in crate::game) enum UiInputMode {
    MouseTouch,
    Touch,
    MouseKeyboard,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::game) struct UiSafeArea {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Resource)]
pub(in crate::game) struct UiMetrics {
    pub page_padding: f32,
    pub panel_padding: f32,
    pub control_gap: f32,
    pub section_gap: f32,
    pub button_height: f32,
    pub input_height: f32,
    pub icon_size: f32,
    pub touch_target_min: f32,
    pub font_body: f32,
    pub font_button: f32,
    pub font_title: f32,
    pub content_max_width: f32,
    pub dialog_max_width: f32,
}

impl Default for UiViewport {
    fn default() -> Self {
        Self::from_logical_size(
            1280.0,
            720.0,
            UiInputMode::MouseTouch,
            UiSafeArea::default(),
        )
    }
}

impl UiViewport {
    pub(in crate::game) fn from_logical_size(
        logical_width: f32,
        logical_height: f32,
        input_mode: UiInputMode,
        safe_area: UiSafeArea,
    ) -> Self {
        let logical_width = logical_width.max(1.0);
        let logical_height = logical_height.max(1.0);

        Self {
            logical_width,
            logical_height,
            width_class: width_class_for(logical_width),
            height_class: height_class_for(logical_height),
            orientation: orientation_for(logical_width, logical_height),
            input_mode,
            safe_area,
        }
    }

    pub(in crate::game) fn safe_area_padding(self, base: f32) -> UiRect {
        self.safe_area.padding_with_base(base)
    }
}

impl UiSafeArea {
    pub(in crate::game) fn padding_with_base(self, base: f32) -> UiRect {
        UiRect {
            left: px(base + self.left),
            right: px(base + self.right),
            top: px(base + self.top),
            bottom: px(base + self.bottom),
        }
    }
}

impl Default for UiMetrics {
    fn default() -> Self {
        Self::from_viewport_and_theme(&UiViewport::default(), &UiTheme::default())
    }
}

impl UiMetrics {
    pub(in crate::game) fn from_viewport_and_theme(viewport: &UiViewport, theme: &UiTheme) -> Self {
        let touch_target_min = match viewport.input_mode {
            UiInputMode::MouseKeyboard => 40.0,
            UiInputMode::MouseTouch | UiInputMode::Touch => 44.0,
        };

        let (
            page_padding,
            panel_padding,
            control_gap,
            section_gap,
            button_height,
            input_height,
            icon_size,
            content_max_width,
            dialog_cap,
        ): (f32, f32, f32, f32, f32, f32, f32, f32, f32) = match viewport.width_class {
            UiWidthClass::Compact => (16.0, 16.0, 8.0, 14.0, 46.0, 46.0, 22.0, 480.0, 480.0),
            UiWidthClass::Medium => (24.0, 20.0, 12.0, 18.0, 46.0, 46.0, 24.0, 680.0, 600.0),
            UiWidthClass::Expanded => (32.0, 24.0, 12.0, 24.0, 44.0, 44.0, 24.0, 920.0, 680.0),
        };

        let safe_horizontal = viewport.safe_area.left + viewport.safe_area.right;
        let available_width = (viewport.logical_width - safe_horizontal).max(1.0);
        let dialog_max_width = dialog_cap.min((available_width - page_padding * 2.0).max(1.0));

        Self {
            page_padding: page_padding.max(theme.layout.screen_padding * 0.5),
            panel_padding: panel_padding.max(theme.panel.padding * 0.45),
            control_gap: control_gap.max(theme.layout.row_gap),
            section_gap: section_gap.max(theme.layout.card_gap),
            button_height: button_height
                .max(theme.button.height.min(48.0))
                .max(touch_target_min),
            input_height: input_height
                .max(theme.button.height.min(48.0))
                .max(touch_target_min),
            icon_size,
            touch_target_min,
            font_body: theme.text.body.clamp(18.0, 24.0),
            font_button: theme.text.button.clamp(16.0, 20.0),
            font_title: theme.text.title.clamp(28.0, 38.0),
            content_max_width,
            dialog_max_width,
        }
    }
}

fn update_ui_viewport_metrics(
    window: Single<&Window, With<PrimaryWindow>>,
    theme: Res<UiTheme>,
    mut viewport: ResMut<UiViewport>,
    mut metrics: ResMut<UiMetrics>,
) {
    let next_viewport = UiViewport::from_logical_size(
        window.width(),
        window.height(),
        default_input_mode(),
        platform_safe_area(),
    );

    if *viewport != next_viewport {
        *viewport = next_viewport;
    }

    if viewport.is_changed() || theme.is_changed() {
        let next_metrics = UiMetrics::from_viewport_and_theme(&viewport, &theme);
        if *metrics != next_metrics {
            *metrics = next_metrics;
        }
    }
}

fn width_class_for(logical_width: f32) -> UiWidthClass {
    if logical_width < 480.0 {
        UiWidthClass::Compact
    } else if logical_width < 840.0 {
        UiWidthClass::Medium
    } else {
        UiWidthClass::Expanded
    }
}

fn height_class_for(logical_height: f32) -> UiHeightClass {
    if logical_height < 600.0 {
        UiHeightClass::Short
    } else if logical_height < 800.0 {
        UiHeightClass::Regular
    } else {
        UiHeightClass::Tall
    }
}

fn orientation_for(logical_width: f32, logical_height: f32) -> UiOrientation {
    if logical_height >= logical_width {
        UiOrientation::Portrait
    } else {
        UiOrientation::Landscape
    }
}

fn default_input_mode() -> UiInputMode {
    UiInputMode::MouseTouch
}

fn platform_safe_area() -> UiSafeArea {
    UiSafeArea::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_phone_portrait_logical_size() {
        let viewport = UiViewport::from_logical_size(
            394.0,
            853.0,
            UiInputMode::MouseTouch,
            UiSafeArea::default(),
        );

        assert_eq!(viewport.width_class, UiWidthClass::Compact);
        assert_eq!(viewport.height_class, UiHeightClass::Tall);
        assert_eq!(viewport.orientation, UiOrientation::Portrait);
    }

    #[test]
    fn classifies_desktop_landscape_logical_size() {
        let viewport = UiViewport::from_logical_size(
            1280.0,
            720.0,
            UiInputMode::MouseTouch,
            UiSafeArea::default(),
        );

        assert_eq!(viewport.width_class, UiWidthClass::Expanded);
        assert_eq!(viewport.height_class, UiHeightClass::Regular);
        assert_eq!(viewport.orientation, UiOrientation::Landscape);
    }

    #[test]
    fn compact_metrics_keep_buttons_at_touch_target() {
        let viewport = UiViewport::from_logical_size(
            394.0,
            853.0,
            UiInputMode::MouseTouch,
            UiSafeArea::default(),
        );
        let metrics = UiMetrics::from_viewport_and_theme(&viewport, &UiTheme::default());

        assert!(metrics.button_height >= metrics.touch_target_min);
    }

    #[test]
    fn safe_area_padding_adds_base_to_each_edge() {
        let safe_area = UiSafeArea {
            left: 1.0,
            right: 2.0,
            top: 3.0,
            bottom: 4.0,
        };

        assert_eq!(
            safe_area.padding_with_base(10.0),
            UiRect {
                left: px(11.0),
                right: px(12.0),
                top: px(13.0),
                bottom: px(14.0),
            }
        );
    }
}
