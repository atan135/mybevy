use bevy::{prelude::*, window::PrimaryWindow};

#[cfg(not(target_os = "android"))]
use crate::config::window::{WindowSafeAreaSource, WindowStartupConfig};
use crate::framework::ui::style::UiTheme;

use super::safe_area::UiSafeAreaStatus;
#[cfg(not(target_os = "android"))]
use super::safe_area::{UiPhysicalSafeAreaInsets, UiSafeAreaSource};

pub(crate) const UI_VIEWPORT_WIDTH_MEDIUM_MIN: f32 = 480.0;
pub(crate) const UI_VIEWPORT_WIDTH_EXPANDED_MIN: f32 = 840.0;
pub(crate) const UI_VIEWPORT_HEIGHT_REGULAR_MIN: f32 = 600.0;
pub(crate) const UI_VIEWPORT_HEIGHT_TALL_MIN: f32 = 800.0;

pub(crate) struct UiViewportPlugin;

impl Plugin for UiViewportPlugin {
    fn build(&self, app: &mut App) {
        let initial_safe_area = initial_safe_area_status(app.world());
        let initial_viewport = initial_ui_viewport(app.world(), initial_safe_area.logical);
        let initial_metrics = if let Some(theme) = app.world().get_resource::<UiTheme>() {
            UiMetrics::from_viewport_and_theme(&initial_viewport, theme)
        } else {
            UiMetrics::from_viewport_and_theme(&initial_viewport, &UiTheme::default())
        };

        app.insert_resource(initial_safe_area)
            .insert_resource(initial_viewport)
            .insert_resource(initial_metrics)
            .add_systems(Update, update_ui_viewport_metrics);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Resource)]
pub(crate) struct UiViewport {
    pub logical_width: f32,
    pub logical_height: f32,
    pub window_logical_width: f32,
    pub window_logical_height: f32,
    pub device_width: f32,
    pub device_height: f32,
    pub device_scale: f32,
    pub preview_scale: f32,
    pub width_class: UiWidthClass,
    pub height_class: UiHeightClass,
    pub orientation: UiOrientation,
    pub input_mode: UiInputMode,
    pub safe_area: UiSafeArea,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UiWidthClass {
    Compact,
    Medium,
    Expanded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UiHeightClass {
    Short,
    Regular,
    Tall,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UiOrientation {
    Portrait,
    Landscape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum UiInputMode {
    MouseTouch,
    Touch,
    MouseKeyboard,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct UiSafeArea {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Resource)]
pub(crate) struct UiMetrics {
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
        Self::from_device_logical_size(
            1280.0,
            720.0,
            UiInputMode::MouseTouch,
            UiSafeArea::default(),
        )
    }
}

impl UiViewport {
    pub(crate) fn from_device_logical_size(
        logical_width: f32,
        logical_height: f32,
        input_mode: UiInputMode,
        safe_area: UiSafeArea,
    ) -> Self {
        Self::from_logical_size(
            logical_width,
            logical_height,
            logical_width,
            logical_height,
            logical_width,
            logical_height,
            1.0,
            1.0,
            input_mode,
            safe_area,
        )
    }

    pub(crate) fn from_logical_size(
        logical_width: f32,
        logical_height: f32,
        window_logical_width: f32,
        window_logical_height: f32,
        device_width: f32,
        device_height: f32,
        device_scale: f32,
        preview_scale: f32,
        input_mode: UiInputMode,
        safe_area: UiSafeArea,
    ) -> Self {
        let logical_width = logical_width.max(1.0);
        let logical_height = logical_height.max(1.0);
        let window_logical_width = window_logical_width.max(1.0);
        let window_logical_height = window_logical_height.max(1.0);

        Self {
            logical_width,
            logical_height,
            window_logical_width,
            window_logical_height,
            device_width: device_width.max(1.0),
            device_height: device_height.max(1.0),
            device_scale: device_scale.max(1.0),
            preview_scale: preview_scale.max(0.01),
            width_class: width_class_for(logical_width),
            height_class: height_class_for(logical_height),
            orientation: orientation_for(logical_width, logical_height),
            input_mode,
            safe_area,
        }
    }

    pub(crate) fn safe_area_padding(self, base: f32) -> UiRect {
        self.safe_area.padding_with_base(base)
    }
}

#[derive(Clone, Copy, Debug)]
struct ViewportSizeSource {
    logical_width: f32,
    logical_height: f32,
    window_logical_width: f32,
    window_logical_height: f32,
    device_width: f32,
    device_height: f32,
    device_scale: f32,
    preview_scale: f32,
}

impl UiSafeArea {
    pub(crate) fn padding_with_base(self, base: f32) -> UiRect {
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
    pub(crate) fn from_viewport_and_theme(viewport: &UiViewport, theme: &UiTheme) -> Self {
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
    #[cfg(not(target_os = "android"))] startup_config: Option<Res<WindowStartupConfig>>,
    theme: Res<UiTheme>,
    mut safe_area_status: ResMut<UiSafeAreaStatus>,
    mut viewport: ResMut<UiViewport>,
    mut metrics: ResMut<UiMetrics>,
) {
    let size_source = viewport_size_source(&window, {
        #[cfg(not(target_os = "android"))]
        {
            startup_config.as_deref()
        }
        #[cfg(target_os = "android")]
        {
            None::<&()>
        }
    });
    let next_safe_area = runtime_safe_area_status(&window, {
        #[cfg(not(target_os = "android"))]
        {
            startup_config.as_deref()
        }
        #[cfg(target_os = "android")]
        {
            None::<&()>
        }
    });
    if *safe_area_status != next_safe_area {
        *safe_area_status = next_safe_area;
    }
    let next_viewport = UiViewport::from_logical_size(
        size_source.logical_width,
        size_source.logical_height,
        size_source.window_logical_width,
        size_source.window_logical_height,
        size_source.device_width,
        size_source.device_height,
        size_source.device_scale,
        size_source.preview_scale,
        default_input_mode(),
        safe_area_status.logical,
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

#[cfg(not(target_os = "android"))]
fn viewport_size_source(
    window: &Window,
    startup_config: Option<&WindowStartupConfig>,
) -> ViewportSizeSource {
    if let Some(config) = startup_config {
        return ViewportSizeSource {
            logical_width: config.logical_width(),
            logical_height: config.logical_height(),
            window_logical_width: window.width(),
            window_logical_height: window.height(),
            device_width: config.size.width as f32,
            device_height: config.size.height as f32,
            device_scale: config.device_scale,
            preview_scale: config.preview_scale,
        };
    }

    runtime_window_size_source(window)
}

#[cfg(target_os = "android")]
fn viewport_size_source(window: &Window, _startup_config: Option<&()>) -> ViewportSizeSource {
    runtime_window_size_source(window)
}

fn runtime_window_size_source(window: &Window) -> ViewportSizeSource {
    ViewportSizeSource {
        logical_width: window.width(),
        logical_height: window.height(),
        window_logical_width: window.width(),
        window_logical_height: window.height(),
        device_width: window.physical_width() as f32,
        device_height: window.physical_height() as f32,
        device_scale: window.scale_factor() as f32,
        preview_scale: 1.0,
    }
}

fn initial_ui_viewport(_world: &World, _safe_area: UiSafeArea) -> UiViewport {
    #[cfg(not(target_os = "android"))]
    if let Some(config) = _world.get_resource::<WindowStartupConfig>() {
        return viewport_from_startup_config(config, default_input_mode(), _safe_area);
    }

    UiViewport::default()
}

#[cfg(not(target_os = "android"))]
fn viewport_from_startup_config(
    config: &WindowStartupConfig,
    input_mode: UiInputMode,
    safe_area: UiSafeArea,
) -> UiViewport {
    UiViewport::from_logical_size(
        config.logical_width(),
        config.logical_height(),
        config.logical_width(),
        config.logical_height(),
        config.size.width as f32,
        config.size.height as f32,
        config.device_scale,
        config.preview_scale,
        input_mode,
        safe_area,
    )
}

pub(crate) fn width_class_for(logical_width: f32) -> UiWidthClass {
    if logical_width < UI_VIEWPORT_WIDTH_MEDIUM_MIN {
        UiWidthClass::Compact
    } else if logical_width < UI_VIEWPORT_WIDTH_EXPANDED_MIN {
        UiWidthClass::Medium
    } else {
        UiWidthClass::Expanded
    }
}

pub(crate) fn height_class_for(logical_height: f32) -> UiHeightClass {
    if logical_height < UI_VIEWPORT_HEIGHT_REGULAR_MIN {
        UiHeightClass::Short
    } else if logical_height < UI_VIEWPORT_HEIGHT_TALL_MIN {
        UiHeightClass::Regular
    } else {
        UiHeightClass::Tall
    }
}

pub(crate) fn orientation_for(logical_width: f32, logical_height: f32) -> UiOrientation {
    if logical_height >= logical_width {
        UiOrientation::Portrait
    } else {
        UiOrientation::Landscape
    }
}

pub(crate) fn responsive_classes_are_satisfiable(
    width_class: Option<UiWidthClass>,
    height_class: Option<UiHeightClass>,
    orientation: Option<UiOrientation>,
) -> bool {
    let width = width_interval(width_class);
    let height = height_interval(height_class);
    match orientation {
        None => true,
        Some(UiOrientation::Portrait) => height
            .upper_exclusive
            .is_none_or(|height_upper| height_upper > width.lower),
        Some(UiOrientation::Landscape) => width
            .upper_exclusive
            .is_none_or(|width_upper| width_upper > height.lower),
    }
}

#[derive(Clone, Copy)]
struct ClassInterval {
    lower: f32,
    upper_exclusive: Option<f32>,
}

fn width_interval(class: Option<UiWidthClass>) -> ClassInterval {
    match class {
        None => ClassInterval {
            lower: 0.0,
            upper_exclusive: None,
        },
        Some(UiWidthClass::Compact) => ClassInterval {
            lower: 0.0,
            upper_exclusive: Some(UI_VIEWPORT_WIDTH_MEDIUM_MIN),
        },
        Some(UiWidthClass::Medium) => ClassInterval {
            lower: UI_VIEWPORT_WIDTH_MEDIUM_MIN,
            upper_exclusive: Some(UI_VIEWPORT_WIDTH_EXPANDED_MIN),
        },
        Some(UiWidthClass::Expanded) => ClassInterval {
            lower: UI_VIEWPORT_WIDTH_EXPANDED_MIN,
            upper_exclusive: None,
        },
    }
}

fn height_interval(class: Option<UiHeightClass>) -> ClassInterval {
    match class {
        None => ClassInterval {
            lower: 0.0,
            upper_exclusive: None,
        },
        Some(UiHeightClass::Short) => ClassInterval {
            lower: 0.0,
            upper_exclusive: Some(UI_VIEWPORT_HEIGHT_REGULAR_MIN),
        },
        Some(UiHeightClass::Regular) => ClassInterval {
            lower: UI_VIEWPORT_HEIGHT_REGULAR_MIN,
            upper_exclusive: Some(UI_VIEWPORT_HEIGHT_TALL_MIN),
        },
        Some(UiHeightClass::Tall) => ClassInterval {
            lower: UI_VIEWPORT_HEIGHT_TALL_MIN,
            upper_exclusive: None,
        },
    }
}

fn default_input_mode() -> UiInputMode {
    UiInputMode::MouseTouch
}

fn initial_safe_area_status(_world: &World) -> UiSafeAreaStatus {
    #[cfg(not(target_os = "android"))]
    if let Some(config) = _world.get_resource::<WindowStartupConfig>() {
        return desktop_safe_area_status(config);
    }

    UiSafeAreaStatus::default()
}

#[cfg(not(target_os = "android"))]
fn runtime_safe_area_status(
    _window: &Window,
    startup_config: Option<&WindowStartupConfig>,
) -> UiSafeAreaStatus {
    startup_config
        .map(desktop_safe_area_status)
        .unwrap_or_default()
}

#[cfg(target_os = "android")]
fn runtime_safe_area_status(window: &Window, _startup_config: Option<&()>) -> UiSafeAreaStatus {
    super::safe_area::android_safe_area_status(
        UVec2::new(window.physical_width(), window.physical_height()),
        window.scale_factor() as f32,
    )
}

#[cfg(not(target_os = "android"))]
fn desktop_safe_area_status(config: &WindowStartupConfig) -> UiSafeAreaStatus {
    let logical = UiSafeArea {
        left: config.safe_area.left,
        right: config.safe_area.right,
        top: config.safe_area.top,
        bottom: config.safe_area.bottom,
    };
    let source = match config.safe_area_source {
        WindowSafeAreaSource::None => UiSafeAreaSource::Unavailable,
        WindowSafeAreaSource::ProfileFixture => UiSafeAreaSource::DesktopProfileFixture,
        WindowSafeAreaSource::CommandLineOverride => UiSafeAreaSource::DesktopCommandLineOverride,
    };
    let physical = (source != UiSafeAreaSource::Unavailable).then(|| {
        UiPhysicalSafeAreaInsets::new(
            (logical.left * config.device_scale).round().max(0.0) as u32,
            (logical.right * config.device_scale).round().max(0.0) as u32,
            (logical.top * config.device_scale).round().max(0.0) as u32,
            (logical.bottom * config.device_scale).round().max(0.0) as u32,
        )
    });
    UiSafeAreaStatus {
        logical,
        physical,
        source,
        revision: u64::from(source != UiSafeAreaSource::Unavailable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_phone_portrait_logical_size() {
        let viewport = UiViewport::from_device_logical_size(
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
        let viewport = UiViewport::from_device_logical_size(
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
    fn viewport_responsive_class_satisfiability_respects_orientation() {
        assert!(!responsive_classes_are_satisfiable(
            Some(UiWidthClass::Compact),
            Some(UiHeightClass::Tall),
            Some(UiOrientation::Landscape),
        ));
        assert!(responsive_classes_are_satisfiable(
            Some(UiWidthClass::Compact),
            Some(UiHeightClass::Short),
            Some(UiOrientation::Landscape),
        ));
        assert!(!responsive_classes_are_satisfiable(
            Some(UiWidthClass::Expanded),
            Some(UiHeightClass::Short),
            Some(UiOrientation::Portrait),
        ));
        assert!(responsive_classes_are_satisfiable(
            Some(UiWidthClass::Expanded),
            Some(UiHeightClass::Tall),
            Some(UiOrientation::Portrait),
        ));
    }

    #[test]
    fn compact_metrics_keep_buttons_at_touch_target() {
        let viewport = UiViewport::from_device_logical_size(
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

    #[test]
    fn viewport_keeps_startup_device_and_preview_logical_sizes_distinct() {
        let viewport = UiViewport::from_logical_size(
            393.84616,
            852.9231,
            196.92308,
            426.46155,
            1280.0,
            2772.0,
            3.25,
            0.5,
            UiInputMode::MouseTouch,
            UiSafeArea::default(),
        );

        assert_eq!(viewport.width_class, UiWidthClass::Compact);
        assert_eq!(viewport.height_class, UiHeightClass::Tall);
        assert_eq!(viewport.orientation, UiOrientation::Portrait);
        assert_eq!(viewport.device_width, 1280.0);
        assert_eq!(viewport.device_scale, 3.25);
        assert_eq!(viewport.preview_scale, 0.5);
        assert_eq!(viewport.window_logical_width, 196.92308);
    }

    #[cfg(not(target_os = "android"))]
    #[test]
    fn viewport_from_startup_config_is_available_before_first_update() {
        let config = WindowStartupConfig {
            size: crate::config::window::WindowSize::new(1280, 2772),
            device_scale: 3.25,
            preview_scale: 0.5,
            warnings: Vec::new(),
            ..WindowStartupConfig::default()
        };
        let viewport =
            viewport_from_startup_config(&config, UiInputMode::MouseTouch, UiSafeArea::default());

        assert_eq!(viewport.width_class, UiWidthClass::Compact);
        assert_eq!(viewport.height_class, UiHeightClass::Tall);
        assert_eq!(viewport.orientation, UiOrientation::Portrait);
        assert_eq!(viewport.device_width, 1280.0);
        assert_eq!(viewport.device_height, 2772.0);
    }

    #[cfg(not(target_os = "android"))]
    #[test]
    fn desktop_profile_safe_area_is_explicit_and_stable() {
        let config = crate::config::window::resolve_from_args(["--window-profile", "phone-small"]);
        let first = desktop_safe_area_status(&config);
        let second = desktop_safe_area_status(&config);

        assert_eq!(first, second);
        assert_eq!(first.source, UiSafeAreaSource::DesktopProfileFixture);
        assert_eq!(first.logical.top, 24.0);
        assert_eq!(first.logical.bottom, 20.0);
        assert_eq!(first.physical.unwrap().top, 48);
    }

    #[cfg(not(target_os = "android"))]
    #[test]
    fn desktop_safe_area_override_remains_distinct_from_android_source() {
        let config = crate::config::window::resolve_from_args(["--safe-area-insets", "4,8,12,16"]);
        let status = desktop_safe_area_status(&config);

        assert_eq!(status.source, UiSafeAreaSource::DesktopCommandLineOverride);
        assert_eq!(status.logical.left, 4.0);
        assert_eq!(status.logical.right, 8.0);
    }
}
