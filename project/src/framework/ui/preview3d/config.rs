// config.rs – Stage 2: configuration types for UI 3D preview
// All types below will be fleshed out in stages 2–7; this file is the only
// place touched in this stage.

use bevy::{camera::visibility::RenderLayers, prelude::*, ui::UiRect};
use std::fmt;

// ─────────────────────────────────────────────────────────────────────────────
// PreviewFit
// ─────────────────────────────────────────────────────────────────────────────

/// How the rendered 3D image fills the UI node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PreviewFit {
    /// Stretch to fill the node (may distort).
    Stretch,
    /// Letterbox / contain: show entire image, preserve aspect ratio.
    Contain,
    /// Crop / cover: fill the node, preserve aspect ratio, optionally offset
    /// the focal point inside the viewport.
    Cover { focus: Vec2 },
}

// ─────────────────────────────────────────────────────────────────────────────
// PreviewPixelScale
// ─────────────────────────────────────────────────────────────────────────────

/// Policy for converting the UI logical size to physical pixel resolution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PreviewPixelScale {
    /// Render at 1× device pixel ratio (i.e. logical_size == physical pixels).
    Logical,
    /// Multiply logical size by this factor.
    DeviceScale(f32),
    /// Directly specify the desired physical resolution; logical_size is used
    /// only for aspect-ratio bookkeeping.
    FixedPhysical { width: u32, height: u32 },
}

impl PreviewPixelScale {
    /// Returns the device-scale multiplier for [Logical] and [DeviceScale].
    /// Returns 1.0 for [FixedPhysical] because physical dimensions are
    /// already absolute; callers use logical_size only for aspect ratio.
    pub fn device_scale(&self, _logical_size: Vec2) -> f32 {
        match self {
            PreviewPixelScale::Logical => 1.0,
            PreviewPixelScale::DeviceScale(s) => *s,
            PreviewPixelScale::FixedPhysical { .. } => 1.0,
        }
    }

    fn is_finite(&self) -> bool {
        match self {
            PreviewPixelScale::Logical => true,
            PreviewPixelScale::DeviceScale(s) => s.is_finite() && *s > 0.0,
            PreviewPixelScale::FixedPhysical { width, height } => *width > 0 && *height > 0,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PreviewStyle
// ─────────────────────────────────────────────────────────────────────────────

/// Visual styling for the preview UI node.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewStyle {
    /// Corner radius in logical pixels. 0 = sharp corners.
    pub corner_radius: f32,
    /// Optional background fill color shown behind the 3D image.
    pub background: Option<Color>,
    /// Optional border decoration (color + widths). None = no border.
    pub border: Option<UiRect>,
}

// ─────────────────────────────────────────────────────────────────────────────
// PreviewCameraConfig
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for the embedded 3D projection camera.
#[derive(Clone, Debug)]
pub struct PreviewCameraConfig {
    /// RenderLayers this camera writes to. Must contain at least one layer.
    pub layer: RenderLayers,
    /// Projection type (perspective or orthographic).
    pub projection: Projection,
    /// Initial world-space transform of the camera.
    pub initial_transform: Transform,
    /// ClearColor used when this camera renders. Color::NONE produces a
    /// transparent background so the UI background shows through.
    pub clear_color: Color,
    /// Horizontal field-of-view in radians, stored redundantly so it can be
    /// re-applied if the projection is rebuilt. None = leave projection
    /// untouched after initial creation.
    pub field_of_view: Option<f32>,
}

// ─────────────────────────────────────────────────────────────────────────────
// PreviewContentSpawner
// ─────────────────────────────────────────────────────────────────────────────

/// Closure that spawns 3D content into a ChildSpawner (the preview session
/// root entity). The framework provides the Commands, Assets<Mesh> and
/// Assets<StandardMaterial> access; business code provides meshes, lights,
/// etc.
///
/// Note: ChildSpawner is the Bevy 0.18 replacement for the removed
/// ChildBuilder type.
pub type PreviewContentSpawner = Box<
    dyn FnOnce(&mut ChildSpawner, &mut Commands, &mut Assets<Mesh>, &mut Assets<StandardMaterial>),
>;

// ─────────────────────────────────────────────────────────────────────────────
// PreviewContent
// ─────────────────────────────────────────────────────────────────────────────

/// Source of 3D content for a preview viewport.
///
/// This type intentionally does NOT implement Clone because the Spawn
/// variant wraps a dyn FnOnce which cannot be cloned. Call sites that need
/// to duplicate a config should construct a fresh Preview3dConfig value
/// rather than cloning.
pub enum PreviewContent {
    /// No 3D content is present yet; call dd_preview3d_content later.
    None,
    /// Tag existing entities (already in the world) with the viewport's
    /// RenderLayers so they become visible to the preview camera.
    TagEntities(Vec<Entity>),
    /// Spawn new 3D content via the provided closure. The closure is called
    /// exactly once when the viewport is first created.
    Spawn(PreviewContentSpawner),
}

impl fmt::Debug for PreviewContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "PreviewContent::None"),
            Self::TagEntities(entities) => f
                .debug_tuple("PreviewContent::TagEntities")
                .field(entities)
                .finish(),
            Self::Spawn(_) => write!(f, "PreviewContent::Spawn(...)"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Preview3dConfig
// ─────────────────────────────────────────────────────────────────────────────

/// Complete configuration for a single UI 3D preview viewport.
#[derive(Debug)]
pub struct Preview3dConfig {
    /// Logical pixel size of the preview node inside the UI layout tree.
    pub logical_size: Vec2,
    /// Camera / rendering parameters.
    pub camera: PreviewCameraConfig,
    /// What 3D content to render.
    pub content: PreviewContent,
    /// How the rendered image fills the node.
    pub fit: PreviewFit,
    /// Visual style of the container node.
    pub style: PreviewStyle,
    /// Physical pixel resolution policy.
    pub pixel_scale: PreviewPixelScale,
}

/// Preview3dConfig does not derive Clone because its content field
/// contains a PreviewContent enum whose Spawn variant holds a dyn FnOnce
/// that cannot be cloned. Call sites that need to duplicate a config should
/// construct a fresh Preview3dConfig value rather than cloning.
impl Clone for Preview3dConfig {
    fn clone(&self) -> Self {
        // TagEntities can be cloned; None and Spawn cannot.
        // Panic if non-cloneable variants are used.
        let content = match &self.content {
            PreviewContent::None => PreviewContent::None,
            PreviewContent::TagEntities(entities) => {
                // Vec<Entity> is Clone
                PreviewContent::TagEntities(entities.clone())
            }
            PreviewContent::Spawn(_) => {
                panic!("Preview3dConfig cannot be cloned when content is Spawn")
            }
        };
        Preview3dConfig {
            logical_size: self.logical_size,
            camera: self.camera.clone(),
            content,
            fit: self.fit,
            style: self.style.clone(),
            pixel_scale: self.pixel_scale,
        }
    }
}

impl Preview3dConfig {
    /// Validate the entire config. Returns Ok(()) if all fields are valid,
    /// otherwise returns the first error encountered.
    pub fn validate(&self) -> Result<(), PreviewConfigError> {
        // logical_size
        if self.logical_size.x <= 0.0 || self.logical_size.y <= 0.0 {
            return Err(PreviewConfigError::NonPositiveSize(self.logical_size));
        }
        if !self.logical_size.is_finite() {
            return Err(PreviewConfigError::NonFiniteSize(self.logical_size));
        }

        // camera.layer — must contain at least one layer bit
        if self.camera.layer == RenderLayers::none() {
            return Err(PreviewConfigError::EmptyLayer);
        }

        // camera.field_of_view
        if let Some(fov) = self.camera.field_of_view {
            if !(0.0 < fov && fov < std::f32::consts::PI) {
                return Err(PreviewConfigError::InvalidFieldOfView(fov));
            }
        }

        // pixel_scale
        if !self.pixel_scale.is_finite() {
            return Err(PreviewConfigError::NonFinitePixelScale);
        }

        // style.corner_radius
        if self.style.corner_radius < 0.0 || !self.style.corner_radius.is_finite() {
            return Err(PreviewConfigError::InvalidCornerRadius(
                self.style.corner_radius,
            ));
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PreviewConfigError
// ─────────────────────────────────────────────────────────────────────────────

/// Errors that can arise from an invalid [Preview3dConfig].
#[derive(Clone, Debug)]
pub enum PreviewConfigError {
    /// logical_size has a zero or negative component.
    NonPositiveSize(Vec2),
    /// logical_size contains non-finite (NaN / Inf) components.
    NonFiniteSize(Vec2),
    /// camera.layer has no bits set.
    EmptyLayer,
    /// camera.field_of_view is outside the range (0, π).
    InvalidFieldOfView(f32),
    /// style.corner_radius is negative or non-finite.
    InvalidCornerRadius(f32),
    /// pixel_scale contains non-finite values or invalid parameters.
    NonFinitePixelScale,
}

impl fmt::Display for PreviewConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonPositiveSize(s) => {
                write!(f, "logical_size must be positive, got ({}, {})", s.x, s.y)
            }
            Self::NonFiniteSize(s) => {
                write!(f, "logical_size must be finite, got ({}, {})", s.x, s.y)
            }
            Self::EmptyLayer => write!(f, "camera.layer must contain at least one layer bit"),
            Self::InvalidFieldOfView(fov) => {
                write!(f, "field_of_view must be in (0, π), got {}", fov)
            }
            Self::InvalidCornerRadius(r) => {
                write!(f, "corner_radius must be >= 0 and finite, got {}", r)
            }
            Self::NonFinitePixelScale => {
                write!(f, "pixel_scale contains non-finite or invalid values")
            }
        }
    }
}

impl std::error::Error for PreviewConfigError {}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config() -> Preview3dConfig {
        Preview3dConfig {
            logical_size: Vec2::new(360.0, 480.0),
            camera: PreviewCameraConfig {
                layer: RenderLayers::layer(24),
                projection: Projection::Perspective(PerspectiveProjection {
                    fov: 35.0_f32.to_radians(),
                    ..Default::default()
                }),
                initial_transform: Transform::from_xyz(0.0, 0.6, 1.8),
                clear_color: Color::NONE,
                field_of_view: Some(35.0_f32.to_radians()),
            },
            content: PreviewContent::None,
            fit: PreviewFit::Contain,
            style: PreviewStyle {
                corner_radius: 16.0,
                background: Some(Color::srgba(0.05, 0.05, 0.07, 1.0)),
                border: None,
            },
            pixel_scale: PreviewPixelScale::DeviceScale(2.0),
        }
    }

    #[test]
    fn valid_config_passes() {
        assert!(valid_config().validate().is_ok());
    }

    #[test]
    fn zero_width_fails() {
        let mut cfg = valid_config();
        cfg.logical_size.x = 0.0;
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, PreviewConfigError::NonPositiveSize(_)));
    }

    #[test]
    fn zero_height_fails() {
        let mut cfg = valid_config();
        cfg.logical_size.y = 0.0;
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, PreviewConfigError::NonPositiveSize(_)));
    }

    #[test]
    fn negative_size_fails() {
        let mut cfg = valid_config();
        cfg.logical_size = Vec2::new(-10.0, 480.0);
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, PreviewConfigError::NonPositiveSize(_)));
    }

    #[test]
    fn nan_size_fails() {
        let mut cfg = valid_config();
        cfg.logical_size.x = f32::NAN;
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, PreviewConfigError::NonFiniteSize(_)));
    }

    #[test]
    fn inf_size_fails() {
        let mut cfg = valid_config();
        cfg.logical_size.y = f32::INFINITY;
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, PreviewConfigError::NonFiniteSize(_)));
    }

    #[test]
    fn empty_layer_fails() {
        let mut cfg = valid_config();
        cfg.camera.layer = RenderLayers::none();
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, PreviewConfigError::EmptyLayer));
    }

    #[test]
    fn fov_zero_fails() {
        let mut cfg = valid_config();
        cfg.camera.field_of_view = Some(0.0);
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, PreviewConfigError::InvalidFieldOfView(0.0)));
    }

    #[test]
    fn fov_pi_fails() {
        let mut cfg = valid_config();
        cfg.camera.field_of_view = Some(std::f32::consts::PI);
        let err = cfg.validate().unwrap_err();
        assert!(matches!(
            err,
            PreviewConfigError::InvalidFieldOfView(fov) if fov == std::f32::consts::PI
        ));
    }

    #[test]
    fn fov_too_large_fails() {
        let mut cfg = valid_config();
        cfg.camera.field_of_view = Some(std::f32::consts::PI + 0.1);
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, PreviewConfigError::InvalidFieldOfView(_)));
    }

    #[test]
    fn negative_corner_radius_fails() {
        let mut cfg = valid_config();
        cfg.style.corner_radius = -1.0;
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, PreviewConfigError::InvalidCornerRadius(-1.0)));
    }

    #[test]
    fn nan_corner_radius_fails() {
        let mut cfg = valid_config();
        cfg.style.corner_radius = f32::NAN;
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, PreviewConfigError::InvalidCornerRadius(_)));
    }

    #[test]
    fn device_scale_zero_fails() {
        let mut cfg = valid_config();
        cfg.pixel_scale = PreviewPixelScale::DeviceScale(0.0);
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, PreviewConfigError::NonFinitePixelScale));
    }

    #[test]
    fn device_scale_nan_fails() {
        let mut cfg = valid_config();
        cfg.pixel_scale = PreviewPixelScale::DeviceScale(f32::NAN);
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, PreviewConfigError::NonFinitePixelScale));
    }

    #[test]
    fn fixed_physical_zero_dimension_fails() {
        let mut cfg = valid_config();
        cfg.pixel_scale = PreviewPixelScale::FixedPhysical {
            width: 0,
            height: 720,
        };
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, PreviewConfigError::NonFinitePixelScale));
    }

    #[test]
    fn error_display_is_user_friendly() {
        let err = PreviewConfigError::NonPositiveSize(Vec2::new(0.0, 100.0));
        let s = err.to_string();
        assert!(s.contains("0"));
        assert!(s.contains("100"));
    }

    #[test]
    fn config_with_tag_entities_can_be_cloned() {
        let cfg = Preview3dConfig {
            logical_size: Vec2::new(100.0, 100.0),
            camera: PreviewCameraConfig {
                layer: RenderLayers::layer(24),
                projection: Projection::Perspective(Default::default()),
                initial_transform: Default::default(),
                clear_color: Color::NONE,
                field_of_view: None,
            },
            content: PreviewContent::TagEntities(vec![]),
            fit: PreviewFit::Stretch,
            style: PreviewStyle {
                corner_radius: 0.0,
                background: None,
                border: None,
            },
            pixel_scale: PreviewPixelScale::Logical,
        };
        // TagEntities variant should not panic on clone
        let _cloned = cfg.clone();
    }

    #[test]
    fn config_with_spawn_panics_on_clone() {
        let cfg = Preview3dConfig {
            logical_size: Vec2::new(100.0, 100.0),
            camera: PreviewCameraConfig {
                layer: RenderLayers::layer(24),
                projection: Projection::Perspective(Default::default()),
                initial_transform: Default::default(),
                clear_color: Color::NONE,
                field_of_view: None,
            },
            content: PreviewContent::Spawn(Box::new(|_, _, _, _| {})),
            fit: PreviewFit::Stretch,
            style: PreviewStyle {
                corner_radius: 0.0,
                background: None,
                border: None,
            },
            pixel_scale: PreviewPixelScale::Logical,
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cfg.clone()));
        assert!(result.is_err());
    }
}
