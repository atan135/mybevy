use bevy::prelude::*;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum UiVisualCapability {
    Layout,
    Typography,
    Image,
    Slice,
    Icon,
    Surface,
    Border,
    Shadow,
    Gradient,
    Animation,
    ControlState,
}

impl UiVisualCapability {
    #[allow(dead_code)]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Layout => "layout",
            Self::Typography => "typography",
            Self::Image => "image",
            Self::Slice => "slice",
            Self::Icon => "icon",
            Self::Surface => "surface",
            Self::Border => "border",
            Self::Shadow => "shadow",
            Self::Gradient => "gradient",
            Self::Animation => "animation",
            Self::ControlState => "control_state",
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum UiVisualSupport {
    FrameworkSupported,
    DirectBevyAllowed,
    Unsupported,
}

impl UiVisualSupport {
    #[allow(dead_code)]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::FrameworkSupported => "framework_supported",
            Self::DirectBevyAllowed => "direct_bevy_allowed",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Marks an intentional use of Bevy UI primitives that has no framework wrapper yet.
#[allow(dead_code)]
#[derive(Clone, Copy, Component, Debug, Eq, PartialEq)]
pub(crate) struct UiDirectBevyVisual {
    pub capability: UiVisualCapability,
    pub reason: &'static str,
}

impl UiDirectBevyVisual {
    #[allow(dead_code)]
    pub(crate) fn new(capability: UiVisualCapability, reason: &'static str) -> Self {
        assert!(
            !reason.trim().is_empty(),
            "direct Bevy visual usage requires a reason"
        );
        Self { capability, reason }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_capability_ids_are_stable() {
        let ids = [
            UiVisualCapability::Layout,
            UiVisualCapability::Typography,
            UiVisualCapability::Image,
            UiVisualCapability::Slice,
            UiVisualCapability::Icon,
            UiVisualCapability::Surface,
            UiVisualCapability::Border,
            UiVisualCapability::Shadow,
            UiVisualCapability::Gradient,
            UiVisualCapability::Animation,
            UiVisualCapability::ControlState,
        ]
        .map(UiVisualCapability::as_str);

        assert_eq!(
            ids,
            [
                "layout",
                "typography",
                "image",
                "slice",
                "icon",
                "surface",
                "border",
                "shadow",
                "gradient",
                "animation",
                "control_state",
            ]
        );
    }

    #[test]
    fn support_status_ids_are_stable() {
        assert_eq!(
            UiVisualSupport::FrameworkSupported.as_str(),
            "framework_supported"
        );
        assert_eq!(
            UiVisualSupport::DirectBevyAllowed.as_str(),
            "direct_bevy_allowed"
        );
        assert_eq!(UiVisualSupport::Unsupported.as_str(), "unsupported");
    }

    #[test]
    fn direct_bevy_marker_records_capability_and_reason() {
        let marker =
            UiDirectBevyVisual::new(UiVisualCapability::Shadow, "temporary BoxShadow experiment");

        assert_eq!(marker.capability, UiVisualCapability::Shadow);
        assert_eq!(marker.reason, "temporary BoxShadow experiment");
    }

    #[test]
    #[should_panic(expected = "direct Bevy visual usage requires a reason")]
    fn direct_bevy_marker_rejects_empty_reason() {
        UiDirectBevyVisual::new(UiVisualCapability::Gradient, "   ");
    }
}
