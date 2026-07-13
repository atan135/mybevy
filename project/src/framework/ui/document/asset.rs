use super::{UiDocument, UiNode, UiStyleId, UiVisualFieldError};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path};

pub const UI_ASSET_MAX_DIMENSION: u32 = 4096;
pub const UI_ASSET_MAX_DECODED_BYTES: u64 = 16 * 1024 * 1024;
pub const UI_ASSET_MAX_TOTAL_DECODED_BYTES: u64 = 64 * 1024 * 1024;
pub const UI_ATLAS_MAX_FRAMES: usize = 256;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiAssetEntry {
    pub kind: UiAssetKind,
    pub source: UiAssetSource,
    #[serde(default)]
    pub declared_size: Option<UiAssetDeclaredSize>,
    #[serde(default)]
    pub frames: std::collections::BTreeMap<UiStyleId, UiAtlasFrameDescription>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiAssetKind {
    Image,
    Font,
    Icon,
    Atlas,
    Material,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiAssetSource {
    Packaged { path: String },
    ContentCache { logical_id: String },
    BuiltInMaterial { material: String },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiAssetDeclaredSize {
    pub width: u32,
    pub height: u32,
    pub decoded_bytes: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiAtlasFrameDescription {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub original_width: u32,
    pub original_height: u32,
    #[serde(default)]
    pub pivot: UiImageFocus,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiImageFocus {
    #[serde(default = "default_focus_axis")]
    pub x: f32,
    #[serde(default = "default_focus_axis")]
    pub y: f32,
}

impl Default for UiImageFocus {
    fn default() -> Self {
        Self { x: 0.5, y: 0.5 }
    }
}

fn default_focus_axis() -> f32 {
    0.5
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiImageFit {
    #[default]
    Contain,
    Cover,
    Stretch,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiImagePresentation {
    Fit {
        #[serde(default)]
        fit: UiImageFit,
        #[serde(default)]
        focus: UiImageFocus,
    },
    NineSlice {
        insets: UiNineSliceInsets,
        #[serde(default)]
        center: UiSliceScaleMode,
        #[serde(default)]
        sides: UiSliceScaleMode,
        #[serde(default = "default_corner_scale")]
        max_corner_scale: f32,
        #[serde(default = "default_slice_budget")]
        max_generated_slices: u32,
    },
    Tiled {
        axis: UiTileAxis,
        #[serde(default = "default_tile_stretch")]
        stretch_value: f32,
        #[serde(default = "default_tile_budget")]
        max_repeats: u32,
    },
    AtlasFrame {
        frame: UiStyleId,
        #[serde(default)]
        fit: UiImageFit,
        #[serde(default)]
        focus: UiImageFocus,
    },
}

impl Default for UiImagePresentation {
    fn default() -> Self {
        Self::Fit {
            fit: UiImageFit::Contain,
            focus: UiImageFocus::default(),
        }
    }
}

impl UiImagePresentation {
    #[allow(dead_code)]
    pub(crate) fn to_widget_fit(&self) -> Option<crate::framework::ui::widgets::UiImageFit> {
        let (fit, focus) = match self {
            Self::Fit { fit, focus } | Self::AtlasFrame { fit, focus, .. } => (*fit, *focus),
            Self::NineSlice { .. } | Self::Tiled { .. } => return None,
        };
        Some(match fit {
            UiImageFit::Contain => crate::framework::ui::widgets::UiImageFit::Contain,
            UiImageFit::Cover => crate::framework::ui::widgets::UiImageFit::cover(
                crate::framework::ui::widgets::UiImageFocus::new(focus.x, focus.y),
            ),
            UiImageFit::Stretch => crate::framework::ui::widgets::UiImageFit::Stretch,
        })
    }

    #[allow(dead_code)]
    pub(crate) fn to_widget_advanced_mode(
        &self,
    ) -> Option<crate::framework::ui::widgets::UiAdvancedImageMode> {
        match self {
            Self::NineSlice {
                insets,
                center,
                sides,
                max_corner_scale,
                max_generated_slices,
            } => Some(
                crate::framework::ui::widgets::UiAdvancedImageMode::NineSlice(
                    crate::framework::ui::widgets::UiNineSlice {
                        insets: crate::framework::ui::widgets::UiNineSliceInsets {
                            left: insets.left,
                            right: insets.right,
                            top: insets.top,
                            bottom: insets.bottom,
                        },
                        center: to_widget_slice_mode(*center),
                        sides: to_widget_slice_mode(*sides),
                        max_corner_scale: *max_corner_scale,
                        max_generated_slices: *max_generated_slices,
                    },
                ),
            ),
            Self::Tiled {
                axis,
                stretch_value,
                max_repeats,
            } => Some(crate::framework::ui::widgets::UiAdvancedImageMode::Tiled(
                crate::framework::ui::widgets::UiImageTiling {
                    axis: match axis {
                        UiTileAxis::X => crate::framework::ui::widgets::UiTileAxis::X,
                        UiTileAxis::Y => crate::framework::ui::widgets::UiTileAxis::Y,
                        UiTileAxis::Both => crate::framework::ui::widgets::UiTileAxis::Both,
                    },
                    stretch_value: *stretch_value,
                    max_repeats: *max_repeats,
                },
            )),
            Self::Fit { .. } | Self::AtlasFrame { .. } => None,
        }
    }
}

#[allow(dead_code)]
fn to_widget_slice_mode(mode: UiSliceScaleMode) -> crate::framework::ui::widgets::UiSliceScaleMode {
    match mode {
        UiSliceScaleMode::Stretch => crate::framework::ui::widgets::UiSliceScaleMode::Stretch,
        UiSliceScaleMode::Tile { stretch_value } => {
            crate::framework::ui::widgets::UiSliceScaleMode::Tile { stretch_value }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiNineSliceInsets {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiSliceScaleMode {
    #[default]
    Stretch,
    Tile {
        stretch_value: f32,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiTileAxis {
    X,
    Y,
    Both,
}

fn default_corner_scale() -> f32 {
    1.0
}

fn default_slice_budget() -> u32 {
    256
}

fn default_tile_stretch() -> f32 {
    1.0
}

fn default_tile_budget() -> u32 {
    256
}

impl UiDocument {
    pub(crate) fn validate_assets(&self) -> Vec<UiVisualFieldError> {
        let mut errors = Vec::new();
        let material_count = self
            .assets
            .values()
            .filter(|entry| entry.kind == UiAssetKind::Material)
            .count();
        if material_count > super::UI_DOCUMENT_MAX_MATERIALS {
            errors.push(asset_error("UI_ASSET_MATERIAL_BUDGET_EXCEEDED", "$.assets"));
        }

        let mut total_decoded_bytes = 0_u64;
        for (id, entry) in &self.assets {
            let path = format!("$.assets.{id}");
            validate_asset_source(entry, &path, &mut errors);
            validate_declared_size(entry, &path, &mut total_decoded_bytes, &mut errors);
            validate_atlas_frames(entry, &path, &mut errors);
        }
        if total_decoded_bytes > UI_ASSET_MAX_TOTAL_DECODED_BYTES {
            errors.push(asset_error(
                "UI_ASSET_TOTAL_DECODED_BYTES_BUDGET_EXCEEDED",
                "$.assets",
            ));
        }
        validate_node_assets(self, &self.root, "$.root", &mut errors);
        errors
    }
}

fn validate_asset_source(entry: &UiAssetEntry, path: &str, errors: &mut Vec<UiVisualFieldError>) {
    match &entry.source {
        UiAssetSource::Packaged { path: source_path } => {
            if entry.kind == UiAssetKind::Material {
                errors.push(asset_error(
                    "UI_ASSET_KIND_MISMATCH",
                    &format!("{path}.source"),
                ));
                return;
            }
            if !is_safe_packaged_path(source_path) {
                errors.push(asset_error(
                    "UI_ASSET_PATH_INVALID",
                    &format!("{path}.source.path"),
                ));
            } else if !extension_matches_kind(source_path, entry.kind) {
                errors.push(asset_error(
                    "UI_ASSET_EXTENSION_MISMATCH",
                    &format!("{path}.source.path"),
                ));
            }
        }
        UiAssetSource::ContentCache { logical_id } => {
            if entry.kind == UiAssetKind::Material
                || super::UiAssetId::new(logical_id.clone()).is_err()
            {
                errors.push(asset_error(
                    "UI_ASSET_SOURCE_INVALID",
                    &format!("{path}.source.logical_id"),
                ));
            }
        }
        UiAssetSource::BuiltInMaterial { material } => {
            if entry.kind != UiAssetKind::Material {
                errors.push(asset_error(
                    "UI_ASSET_KIND_MISMATCH",
                    &format!("{path}.source"),
                ));
            } else if material != "frosted_panel_v1" {
                errors.push(asset_error(
                    "UI_MATERIAL_NOT_ALLOWLISTED",
                    &format!("{path}.source.material"),
                ));
            }
        }
    }
}

fn is_safe_packaged_path(path: &str) -> bool {
    if path.is_empty()
        || path.trim() != path
        || !path.is_ascii()
        || path
            .bytes()
            .any(|byte| byte.is_ascii_uppercase() || byte == 0)
        || path.contains('\\')
        || path.contains(':')
        || path.starts_with('/')
        || !path.starts_with("ui/")
    {
        return false;
    }
    let lowered = path.to_ascii_lowercase();
    if lowered.contains("%2f") || lowered.contains("%5c") || lowered.starts_with("data:") {
        return false;
    }
    path.split('/').all(is_safe_path_segment)
        && !Path::new(path).is_absolute()
        && !Path::new(path).components().any(|component| {
            matches!(
                component,
                Component::Prefix(_)
                    | Component::RootDir
                    | Component::CurDir
                    | Component::ParentDir
            )
        })
}

fn is_safe_path_segment(segment: &str) -> bool {
    if segment.is_empty() || matches!(segment, "." | "..") {
        return false;
    }
    let mut bytes = segment.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn extension_matches_kind(path: &str, kind: UiAssetKind) -> bool {
    let extension = Path::new(path).extension().and_then(|value| value.to_str());
    match kind {
        UiAssetKind::Image | UiAssetKind::Atlas | UiAssetKind::Icon => {
            matches!(extension, Some("png" | "jpg" | "jpeg" | "webp"))
        }
        UiAssetKind::Font => matches!(extension, Some("ttf" | "otf")),
        UiAssetKind::Material => false,
    }
}

fn validate_declared_size(
    entry: &UiAssetEntry,
    path: &str,
    total_decoded_bytes: &mut u64,
    errors: &mut Vec<UiVisualFieldError>,
) {
    let Some(size) = entry.declared_size else {
        return;
    };
    if !matches!(
        entry.kind,
        UiAssetKind::Image | UiAssetKind::Atlas | UiAssetKind::Icon
    ) {
        errors.push(asset_error(
            "UI_ASSET_KIND_MISMATCH",
            &format!("{path}.declared_size"),
        ));
        return;
    }
    if size.width == 0 || size.height == 0 || size.decoded_bytes == 0 {
        errors.push(asset_error(
            "UI_ASSET_SIZE_INVALID",
            &format!("{path}.declared_size"),
        ));
    }
    if size.width > UI_ASSET_MAX_DIMENSION || size.height > UI_ASSET_MAX_DIMENSION {
        errors.push(asset_error(
            "UI_ASSET_DIMENSION_BUDGET_EXCEEDED",
            &format!("{path}.declared_size"),
        ));
    }
    if size.decoded_bytes > UI_ASSET_MAX_DECODED_BYTES {
        errors.push(asset_error(
            "UI_ASSET_DECODED_BYTES_BUDGET_EXCEEDED",
            &format!("{path}.declared_size.decoded_bytes"),
        ));
    }
    *total_decoded_bytes = total_decoded_bytes.saturating_add(size.decoded_bytes);
}

fn validate_atlas_frames(entry: &UiAssetEntry, path: &str, errors: &mut Vec<UiVisualFieldError>) {
    if entry.frames.is_empty() {
        return;
    }
    if entry.kind != UiAssetKind::Atlas {
        errors.push(asset_error(
            "UI_ASSET_KIND_MISMATCH",
            &format!("{path}.frames"),
        ));
        return;
    }
    if entry.frames.len() > UI_ATLAS_MAX_FRAMES {
        errors.push(asset_error(
            "UI_ASSET_ATLAS_FRAME_BUDGET_EXCEEDED",
            &format!("{path}.frames"),
        ));
    }
    let Some(size) = entry.declared_size else {
        errors.push(asset_error(
            "UI_ASSET_ATLAS_SIZE_REQUIRED",
            &format!("{path}.declared_size"),
        ));
        return;
    };
    for (id, frame) in &entry.frames {
        let frame_path = format!("{path}.frames.{id}");
        let max_x = frame.x.checked_add(frame.width);
        let max_y = frame.y.checked_add(frame.height);
        if frame.width == 0
            || frame.height == 0
            || max_x.is_none_or(|value| value > size.width)
            || max_y.is_none_or(|value| value > size.height)
            || frame.original_width < frame.width
            || frame.original_height < frame.height
            || frame.original_width > UI_ASSET_MAX_DIMENSION
            || frame.original_height > UI_ASSET_MAX_DIMENSION
            || !valid_focus(frame.pivot)
        {
            errors.push(asset_error("UI_ASSET_ATLAS_FRAME_INVALID", &frame_path));
        }
    }
}

fn validate_node_assets(
    document: &UiDocument,
    node: &UiNode,
    path: &str,
    errors: &mut Vec<UiVisualFieldError>,
) {
    if let UiNode::Image {
        asset,
        presentation,
        ..
    } = node
    {
        match document.assets.get(asset) {
            None => errors.push(asset_error("UI_ASSET_UNKNOWN", &format!("{path}.asset"))),
            Some(entry) => validate_presentation(entry, presentation, path, errors),
        }
    }
    for (index, child) in node.children().iter().enumerate() {
        validate_node_assets(
            document,
            child,
            &format!("{path}.children[{index}]"),
            errors,
        );
    }
}

fn validate_presentation(
    entry: &UiAssetEntry,
    presentation: &UiImagePresentation,
    path: &str,
    errors: &mut Vec<UiVisualFieldError>,
) {
    let presentation_path = format!("{path}.presentation");
    match presentation {
        UiImagePresentation::Fit { focus, .. } => {
            if entry.kind != UiAssetKind::Image {
                errors.push(asset_error("UI_ASSET_KIND_MISMATCH", &presentation_path));
            }
            if !valid_focus(*focus) {
                errors.push(asset_error(
                    "UI_IMAGE_FOCUS_INVALID",
                    &format!("{presentation_path}.focus"),
                ));
            }
        }
        UiImagePresentation::AtlasFrame { frame, focus, .. } => {
            if entry.kind != UiAssetKind::Atlas {
                errors.push(asset_error("UI_ASSET_KIND_MISMATCH", &presentation_path));
            } else if !entry.frames.contains_key(frame) {
                errors.push(asset_error(
                    "UI_ASSET_ATLAS_FRAME_UNKNOWN",
                    &format!("{presentation_path}.frame"),
                ));
            }
            if !valid_focus(*focus) {
                errors.push(asset_error(
                    "UI_IMAGE_FOCUS_INVALID",
                    &format!("{presentation_path}.focus"),
                ));
            }
        }
        UiImagePresentation::NineSlice {
            insets,
            center,
            sides,
            max_corner_scale,
            max_generated_slices,
        } => {
            if entry.kind != UiAssetKind::Image {
                errors.push(asset_error("UI_ASSET_KIND_MISMATCH", &presentation_path));
            }
            let inset_values = [insets.left, insets.right, insets.top, insets.bottom];
            let invalid_insets = inset_values
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
                || entry.declared_size.is_some_and(|size| {
                    insets.left + insets.right >= size.width as f32
                        || insets.top + insets.bottom >= size.height as f32
                });
            if invalid_insets
                || !max_corner_scale.is_finite()
                || !(0.0..=1.0).contains(max_corner_scale)
                || *max_corner_scale == 0.0
                || *max_generated_slices == 0
                || *max_generated_slices
                    > crate::framework::ui::widgets::image::MAX_SLICE_REPEAT_BUDGET
                || !valid_slice_mode(*center)
                || !valid_slice_mode(*sides)
            {
                errors.push(asset_error(
                    "UI_IMAGE_NINE_SLICE_INVALID",
                    &presentation_path,
                ));
            }
        }
        UiImagePresentation::Tiled {
            stretch_value,
            max_repeats,
            ..
        } => {
            if entry.kind != UiAssetKind::Image {
                errors.push(asset_error("UI_ASSET_KIND_MISMATCH", &presentation_path));
            }
            if !stretch_value.is_finite()
                || !(0.001..=1.0).contains(stretch_value)
                || *max_repeats == 0
                || *max_repeats > crate::framework::ui::widgets::image::MAX_TILE_REPEAT_BUDGET
            {
                errors.push(asset_error("UI_IMAGE_TILING_INVALID", &presentation_path));
            }
        }
    }
}

fn valid_focus(focus: UiImageFocus) -> bool {
    focus.x.is_finite()
        && focus.y.is_finite()
        && (0.0..=1.0).contains(&focus.x)
        && (0.0..=1.0).contains(&focus.y)
}

fn valid_slice_mode(mode: UiSliceScaleMode) -> bool {
    match mode {
        UiSliceScaleMode::Stretch => true,
        UiSliceScaleMode::Tile { stretch_value } => {
            stretch_value.is_finite() && (0.001..=1.0).contains(&stretch_value)
        }
    }
}

fn asset_error(code: &'static str, path: &str) -> UiVisualFieldError {
    UiVisualFieldError {
        code,
        path: path.to_owned(),
    }
}
