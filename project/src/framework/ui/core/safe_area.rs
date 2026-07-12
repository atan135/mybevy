use bevy::prelude::*;

use super::viewport::UiSafeArea;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum UiSafeAreaSource {
    #[default]
    Unavailable,
    AndroidWindowInsets,
    DesktopProfileFixture,
    DesktopCommandLineOverride,
}

impl UiSafeAreaSource {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::AndroidWindowInsets => "android_window_insets",
            Self::DesktopProfileFixture => "desktop_profile_fixture",
            Self::DesktopCommandLineOverride => "desktop_command_line_override",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct UiPhysicalSafeAreaInsets {
    pub left: u32,
    pub right: u32,
    pub top: u32,
    pub bottom: u32,
}

impl UiPhysicalSafeAreaInsets {
    pub(crate) const fn new(left: u32, right: u32, top: u32, bottom: u32) -> Self {
        Self {
            left,
            right,
            top,
            bottom,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Resource)]
pub(crate) struct UiSafeAreaStatus {
    pub logical: UiSafeArea,
    pub physical: Option<UiPhysicalSafeAreaInsets>,
    pub source: UiSafeAreaSource,
    pub revision: u64,
}

#[cfg(any(target_os = "android", test))]
pub(crate) fn logical_safe_area_from_physical(
    physical: UiPhysicalSafeAreaInsets,
    physical_window_size: UVec2,
    device_scale: f32,
) -> Option<UiSafeArea> {
    if !device_scale.is_finite()
        || device_scale <= 0.0
        || physical_window_size.x == 0
        || physical_window_size.y == 0
    {
        return None;
    }

    let (left, right) = fit_inset_pair(physical.left, physical.right, physical_window_size.x);
    let (top, bottom) = fit_inset_pair(physical.top, physical.bottom, physical_window_size.y);
    Some(UiSafeArea {
        left: left as f32 / device_scale,
        right: right as f32 / device_scale,
        top: top as f32 / device_scale,
        bottom: bottom as f32 / device_scale,
    })
}

#[cfg(any(target_os = "android", test))]
fn fit_inset_pair(first: u32, second: u32, extent: u32) -> (u32, u32) {
    let total = u64::from(first) + u64::from(second);
    if total <= u64::from(extent) {
        return (first, second);
    }
    if total == 0 {
        return (0, 0);
    }

    let first_fitted = (u64::from(first) * u64::from(extent) / total) as u32;
    (first_fitted, extent.saturating_sub(first_fitted))
}

#[cfg(target_os = "android")]
mod android {
    use std::{ffi::c_void, sync::Mutex};

    use bevy::prelude::*;

    use super::{
        UiPhysicalSafeAreaInsets, UiSafeAreaSource, UiSafeAreaStatus,
        logical_safe_area_from_physical,
    };

    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    struct AndroidInsetState {
        physical: UiPhysicalSafeAreaInsets,
        revision: u64,
        received: bool,
    }

    static ANDROID_INSETS: Mutex<AndroidInsetState> = Mutex::new(AndroidInsetState {
        physical: UiPhysicalSafeAreaInsets::new(0, 0, 0, 0),
        revision: 0,
        received: false,
    });

    pub(crate) fn current_status(
        physical_window_size: UVec2,
        device_scale: f32,
    ) -> UiSafeAreaStatus {
        let state = *ANDROID_INSETS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !state.received {
            return UiSafeAreaStatus::default();
        }

        let logical =
            logical_safe_area_from_physical(state.physical, physical_window_size, device_scale)
                .unwrap_or_default();
        UiSafeAreaStatus {
            logical,
            physical: Some(state.physical),
            source: UiSafeAreaSource::AndroidWindowInsets,
            revision: state.revision,
        }
    }

    fn publish(left: i32, right: i32, top: i32, bottom: i32) {
        let physical = UiPhysicalSafeAreaInsets::new(
            left.max(0) as u32,
            right.max(0) as u32,
            top.max(0) as u32,
            bottom.max(0) as u32,
        );
        let mut state = ANDROID_INSETS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.received && state.physical == physical {
            return;
        }
        state.physical = physical;
        state.received = true;
        state.revision = state.revision.saturating_add(1);
    }

    #[allow(non_snake_case)]
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_mybevy_project_MainActivity_nativeOnWindowInsetsChanged(
        _env: *mut c_void,
        _class: *mut c_void,
        left: i32,
        right: i32,
        top: i32,
        bottom: i32,
    ) {
        publish(left, right, top, bottom);
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn duplicate_android_callback_does_not_advance_revision() {
            publish(4, 8, 12, 16);
            let first = current_status(UVec2::new(400, 800), 2.0);
            publish(4, 8, 12, 16);
            let second = current_status(UVec2::new(400, 800), 2.0);

            assert_eq!(first, second);
        }
    }
}

#[cfg(target_os = "android")]
pub(crate) use android::current_status as android_safe_area_status;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_physical_android_insets_to_logical_pixels() {
        let safe_area = logical_safe_area_from_physical(
            UiPhysicalSafeAreaInsets::new(30, 15, 90, 60),
            UVec2::new(1080, 2400),
            3.0,
        )
        .unwrap();

        assert_eq!(
            safe_area,
            UiSafeArea {
                left: 10.0,
                right: 5.0,
                top: 30.0,
                bottom: 20.0,
            }
        );
    }

    #[test]
    fn rejects_invalid_scale_or_window_size() {
        let physical = UiPhysicalSafeAreaInsets::new(1, 2, 3, 4);

        assert_eq!(
            logical_safe_area_from_physical(physical, UVec2::new(100, 200), 0.0),
            None
        );
        assert_eq!(
            logical_safe_area_from_physical(physical, UVec2::ZERO, 2.0),
            None
        );
    }

    #[test]
    fn malformed_inset_pairs_are_scaled_inside_the_window() {
        let safe_area = logical_safe_area_from_physical(
            UiPhysicalSafeAreaInsets::new(80, 80, 300, 300),
            UVec2::new(100, 400),
            1.0,
        )
        .unwrap();

        assert_eq!(safe_area.left + safe_area.right, 100.0);
        assert_eq!(safe_area.top + safe_area.bottom, 400.0);
    }
}
