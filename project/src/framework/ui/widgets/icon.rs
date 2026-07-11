use bevy::{asset::LoadState, prelude::*};

use super::image::{UiImageError, UiImagePixelSize, UiImageTextureSource};

const ICON_SOURCE_PIXELS: u32 = 96;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct UiIconId(&'static str);

impl UiIconId {
    pub(crate) const ADD: Self = Self::new("add");
    pub(crate) const REMOVE: Self = Self::new("remove");
    pub(crate) const HELP: Self = Self::new("help");
    pub(crate) const CLOSE: Self = Self::new("close");
    pub(crate) const LOADING: Self = Self::new("loading");
    pub(crate) const ARROW_LEFT: Self = Self::new("arrow_left");
    pub(crate) const ARROW_RIGHT: Self = Self::new("arrow_right");
    pub(crate) const FULL_COLOR_BADGE: Self = Self::new("full_color_badge");
    pub(crate) const MISSING: Self = Self::new("missing");

    pub(crate) const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub(crate) const fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiIconTintPolicy {
    MonochromeTintable,
    FullColor,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UiIconDescriptor {
    pub id: UiIconId,
    pub path: &'static str,
    pub default_size: f32,
    pub source_size: UVec2,
    pub tint_policy: UiIconTintPolicy,
}

impl UiIconDescriptor {
    const fn new(
        id: UiIconId,
        path: &'static str,
        default_size: f32,
        tint_policy: UiIconTintPolicy,
    ) -> Self {
        Self {
            id,
            path,
            default_size,
            source_size: UVec2::splat(ICON_SOURCE_PIXELS),
            tint_policy,
        }
    }

    pub(crate) fn validate(self) -> Result<(), UiIconError> {
        if !self.default_size.is_finite() || self.default_size <= 0.0 {
            return Err(UiIconError::InvalidDefaultSize);
        }
        if self.source_size.x == 0 || self.source_size.y == 0 {
            return Err(UiIconError::InvalidSourceSize);
        }
        if !self.path.ends_with(".png") {
            return Err(UiIconError::InvalidSourcePath);
        }
        UiImageTextureSource::new(
            self.path,
            UiImagePixelSize::new(self.source_size.x, self.source_size.y),
        )
        .validate()
        .map_err(|error| match error {
            UiImageError::ZeroDeclaredSourceSize => UiIconError::InvalidSourceSize,
            _ => UiIconError::InvalidSourcePath,
        })
    }
}

pub(crate) const UI_ICON_DESCRIPTORS: &[UiIconDescriptor] = &[
    UiIconDescriptor::new(
        UiIconId::ADD,
        "ui/icons/add.png",
        22.0,
        UiIconTintPolicy::MonochromeTintable,
    ),
    UiIconDescriptor::new(
        UiIconId::REMOVE,
        "ui/icons/remove.png",
        22.0,
        UiIconTintPolicy::MonochromeTintable,
    ),
    UiIconDescriptor::new(
        UiIconId::HELP,
        "ui/icons/help.png",
        22.0,
        UiIconTintPolicy::MonochromeTintable,
    ),
    UiIconDescriptor::new(
        UiIconId::CLOSE,
        "ui/icons/close.png",
        22.0,
        UiIconTintPolicy::MonochromeTintable,
    ),
    UiIconDescriptor::new(
        UiIconId::LOADING,
        "ui/icons/loading.png",
        22.0,
        UiIconTintPolicy::MonochromeTintable,
    ),
    UiIconDescriptor::new(
        UiIconId::ARROW_LEFT,
        "ui/icons/arrow-left.png",
        20.0,
        UiIconTintPolicy::MonochromeTintable,
    ),
    UiIconDescriptor::new(
        UiIconId::ARROW_RIGHT,
        "ui/icons/arrow-right.png",
        20.0,
        UiIconTintPolicy::MonochromeTintable,
    ),
    UiIconDescriptor::new(
        UiIconId::FULL_COLOR_BADGE,
        "ui/icons/full-color-badge.png",
        36.0,
        UiIconTintPolicy::FullColor,
    ),
    UiIconDescriptor::new(
        UiIconId::MISSING,
        "ui/icons/missing.png",
        22.0,
        UiIconTintPolicy::FullColor,
    ),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiIconError {
    UnknownId,
    InvalidSourcePath,
    InvalidDefaultSize,
    InvalidSourceSize,
    AssetUnavailable,
}

impl UiIconError {
    #[allow(dead_code)]
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::UnknownId => "unknown_icon_id",
            Self::InvalidSourcePath => "invalid_icon_source_path",
            Self::InvalidDefaultSize => "invalid_icon_default_size",
            Self::InvalidSourceSize => "invalid_icon_source_size",
            Self::AssetUnavailable => "icon_asset_unavailable",
        }
    }
}

pub(crate) fn resolve_ui_icon_descriptor(
    id: UiIconId,
) -> Result<&'static UiIconDescriptor, UiIconError> {
    resolve_icon_descriptor(UI_ICON_DESCRIPTORS, id)
}

fn resolve_icon_descriptor(
    descriptors: &[UiIconDescriptor],
    id: UiIconId,
) -> Result<&UiIconDescriptor, UiIconError> {
    let descriptor = descriptors
        .iter()
        .find(|descriptor| descriptor.id == id)
        .ok_or(UiIconError::UnknownId)?;
    descriptor.validate()?;
    Ok(descriptor)
}

fn missing_icon_descriptor() -> &'static UiIconDescriptor {
    let descriptor = UI_ICON_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.id == UiIconId::MISSING)
        .expect("the built-in icon registry must define the missing placeholder");
    descriptor
        .validate()
        .expect("the built-in missing icon descriptor must be valid");
    descriptor
}

#[derive(Clone, Copy, Debug)]
struct ResolvedUiIcon {
    requested: UiIconId,
    descriptor: &'static UiIconDescriptor,
    fallback: Option<UiIconError>,
}

fn resolve_ui_icon(id: UiIconId) -> ResolvedUiIcon {
    match resolve_ui_icon_descriptor(id) {
        Ok(descriptor) => ResolvedUiIcon {
            requested: id,
            descriptor,
            fallback: None,
        },
        Err(error) => ResolvedUiIcon {
            requested: id,
            descriptor: missing_icon_descriptor(),
            fallback: Some(error),
        },
    }
}

fn asset_unavailable_resolution(requested: UiIconId) -> UiIconResolutionStatus {
    let placeholder = missing_icon_descriptor();
    UiIconResolutionStatus {
        requested,
        rendered: placeholder.id,
        path: placeholder.path,
        tint_policy: placeholder.tint_policy,
        fallback: Some(UiIconError::AssetUnavailable),
    }
}

#[derive(Clone, Copy, Debug, Component, Eq, PartialEq)]
pub(crate) struct UiIconResolutionStatus {
    pub requested: UiIconId,
    pub rendered: UiIconId,
    pub path: &'static str,
    pub tint_policy: UiIconTintPolicy,
    pub fallback: Option<UiIconError>,
}

impl UiIconResolutionStatus {
    fn from_resolved(resolved: ResolvedUiIcon) -> Self {
        Self {
            requested: resolved.requested,
            rendered: resolved.descriptor.id,
            path: resolved.descriptor.path,
            tint_policy: resolved.descriptor.tint_policy,
            fallback: resolved.fallback,
        }
    }
}

#[derive(Clone, Copy, Debug, Component, Eq, PartialEq)]
pub(crate) enum UiIconAssetStatus {
    Loading,
    Ready,
    FallbackLoading(UiIconError),
    FallbackReady(UiIconError),
    PlaceholderFailed(UiIconError),
}

#[derive(Clone, Debug, Component, PartialEq)]
pub(crate) struct UiIconVisual {
    requested: UiIconId,
    requested_tint: Color,
}

#[derive(Bundle)]
pub(crate) struct UiIconBundle {
    pub node: Node,
    pub image: ImageNode,
    pub resolution: UiIconResolutionStatus,
    pub asset_status: UiIconAssetStatus,
    pub visual: UiIconVisual,
    pub name: Name,
}

pub(crate) fn ui_icon(
    asset_server: &AssetServer,
    id: UiIconId,
    visual_size: f32,
    tint: Color,
) -> UiIconBundle {
    let resolved = resolve_ui_icon(id);
    let resolution = UiIconResolutionStatus::from_resolved(resolved);
    let size = if visual_size.is_finite() && visual_size > 0.0 {
        visual_size
    } else {
        resolved.descriptor.default_size
    };
    let mut image = ImageNode::new(asset_server.load(resolved.descriptor.path));
    image.color = effective_ui_icon_tint(resolved.descriptor.tint_policy, tint);
    let asset_status = resolved.fallback.map_or(
        UiIconAssetStatus::Loading,
        UiIconAssetStatus::FallbackLoading,
    );

    UiIconBundle {
        node: Node {
            width: px(size),
            min_width: px(size),
            height: px(size),
            min_height: px(size),
            flex_shrink: 0.0,
            ..default()
        },
        image,
        resolution,
        asset_status,
        visual: UiIconVisual {
            requested: id,
            requested_tint: tint,
        },
        name: Name::new(format!("UI icon: {}", id.as_str())),
    }
}

pub(crate) fn ui_icon_default_size(id: UiIconId) -> f32 {
    resolve_ui_icon(id).descriptor.default_size
}

pub(crate) fn effective_ui_icon_tint(policy: UiIconTintPolicy, requested: Color) -> Color {
    match policy {
        UiIconTintPolicy::MonochromeTintable => requested,
        UiIconTintPolicy::FullColor => Color::WHITE,
    }
}

pub(crate) fn apply_ui_icon_request(
    asset_server: &AssetServer,
    requested: UiIconId,
    requested_tint: Color,
    image: &mut ImageNode,
    visual: &mut UiIconVisual,
    resolution: &mut UiIconResolutionStatus,
    asset_status: &mut UiIconAssetStatus,
) {
    if visual.requested != requested {
        let resolved = resolve_ui_icon(requested);
        let next_resolution = UiIconResolutionStatus::from_resolved(resolved);
        image.image = asset_server.load(resolved.descriptor.path);
        if *resolution != next_resolution {
            *resolution = next_resolution;
        }
        let next_asset_status = resolved.fallback.map_or(
            UiIconAssetStatus::Loading,
            UiIconAssetStatus::FallbackLoading,
        );
        if *asset_status != next_asset_status {
            *asset_status = next_asset_status;
        }
        visual.requested = requested;
    }

    if visual.requested_tint != requested_tint {
        visual.requested_tint = requested_tint;
    }
    let next_tint = effective_ui_icon_tint(resolution.tint_policy, requested_tint);
    if image.color != next_tint {
        image.color = next_tint;
    }
}

pub(crate) fn sync_ui_icon_asset_status(
    asset_server: Res<AssetServer>,
    mut icons: Query<(
        &mut ImageNode,
        &UiIconVisual,
        &mut UiIconResolutionStatus,
        &mut UiIconAssetStatus,
    )>,
) {
    for (mut image, visual, mut resolution, mut asset_status) in &mut icons {
        match asset_server.get_load_state(image.image.id()) {
            Some(LoadState::Loaded) => {
                let next = resolution
                    .fallback
                    .map_or(UiIconAssetStatus::Ready, UiIconAssetStatus::FallbackReady);
                if *asset_status != next {
                    *asset_status = next;
                }
            }
            Some(LoadState::Failed(_)) if resolution.rendered != UiIconId::MISSING => {
                let placeholder = missing_icon_descriptor();
                image.image = asset_server.load(placeholder.path);
                image.color = Color::WHITE;
                let next_resolution = asset_unavailable_resolution(resolution.requested);
                if *resolution != next_resolution {
                    *resolution = next_resolution;
                }
                if *asset_status
                    != UiIconAssetStatus::FallbackLoading(UiIconError::AssetUnavailable)
                {
                    *asset_status =
                        UiIconAssetStatus::FallbackLoading(UiIconError::AssetUnavailable);
                }
            }
            Some(LoadState::Failed(_)) => {
                let reason = resolution.fallback.unwrap_or(UiIconError::AssetUnavailable);
                let next = UiIconAssetStatus::PlaceholderFailed(reason);
                if *asset_status != next {
                    *asset_status = next;
                }
            }
            Some(LoadState::NotLoaded | LoadState::Loading) | None => {
                let next = resolution.fallback.map_or(
                    UiIconAssetStatus::Loading,
                    UiIconAssetStatus::FallbackLoading,
                );
                if *asset_status != next {
                    *asset_status = next;
                }
            }
        }

        let next_tint = effective_ui_icon_tint(resolution.tint_policy, visual.requested_tint);
        if image.color != next_tint {
            image.color = next_tint;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, fs, path::Path};

    use serde::Deserialize;
    use sha2::{Digest, Sha256};

    use super::*;

    #[derive(Debug, Deserialize)]
    struct IconManifest {
        version: u32,
        raster_size: ManifestRasterSize,
        generator: String,
        source_sets: Vec<ManifestSourceSet>,
        icons: Vec<ManifestIcon>,
    }

    #[derive(Debug, Deserialize)]
    struct ManifestRasterSize {
        width: u32,
        height: u32,
    }

    #[derive(Debug, Deserialize)]
    struct ManifestSourceSet {
        name: String,
        version: String,
        license: String,
        license_path: String,
        source_base_url: String,
    }

    #[derive(Debug, Deserialize)]
    struct ManifestIcon {
        id: String,
        path: String,
        tint_policy: String,
        source: String,
        sha256: String,
    }

    #[test]
    fn icon_registry_has_stable_unique_ids_and_valid_paths() {
        let mut ids = HashSet::new();
        let mut paths = HashSet::new();

        for descriptor in UI_ICON_DESCRIPTORS {
            assert!(ids.insert(descriptor.id));
            assert!(paths.insert(descriptor.path));
            assert_eq!(descriptor.validate(), Ok(()));
            assert_eq!(descriptor.source_size, UVec2::splat(96));
        }
        assert!(ids.contains(&UiIconId::MISSING));
    }

    #[test]
    fn unknown_icon_id_resolves_to_visible_full_color_placeholder() {
        let requested = UiIconId::new("gallery_unknown_icon");
        let resolved = resolve_ui_icon(requested);

        assert_eq!(resolved.requested, requested);
        assert_eq!(resolved.descriptor.id, UiIconId::MISSING);
        assert_eq!(resolved.descriptor.tint_policy, UiIconTintPolicy::FullColor);
        assert_eq!(resolved.fallback, Some(UiIconError::UnknownId));
    }

    #[test]
    fn descriptors_reject_unsafe_or_non_png_asset_paths() {
        for path in ["../icons/add.png", "ui\\icons\\add.png", "ui/icons/add.svg"] {
            let descriptor = UiIconDescriptor {
                id: UiIconId::new("invalid"),
                path,
                default_size: 20.0,
                source_size: UVec2::splat(96),
                tint_policy: UiIconTintPolicy::MonochromeTintable,
            };
            assert_eq!(descriptor.validate(), Err(UiIconError::InvalidSourcePath));
        }
    }

    #[test]
    fn full_color_icons_ignore_tint_while_monochrome_icons_accept_it() {
        let tint = Color::srgb(0.2, 0.7, 0.4);
        assert_eq!(
            effective_ui_icon_tint(UiIconTintPolicy::MonochromeTintable, tint),
            tint
        );
        assert_eq!(
            effective_ui_icon_tint(UiIconTintPolicy::FullColor, tint),
            Color::WHITE
        );
    }

    #[test]
    fn packaged_icon_assets_match_manifest_hashes_and_png_dimensions() {
        let asset_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let manifest_path = asset_root.join("ui/icons/manifest.ron");
        let source = fs::read_to_string(manifest_path).expect("icon manifest should be readable");
        let manifest =
            ron::de::from_str::<IconManifest>(&source).expect("icon manifest should be valid RON");

        assert_eq!(manifest.version, 1);
        assert_eq!(manifest.raster_size.width, ICON_SOURCE_PIXELS);
        assert_eq!(manifest.raster_size.height, ICON_SOURCE_PIXELS);
        assert_eq!(manifest.generator, "scripts/generate-ui-icons.ps1");
        assert_eq!(manifest.icons.len(), UI_ICON_DESCRIPTORS.len());

        let lucide = manifest
            .source_sets
            .iter()
            .find(|source_set| source_set.name == "Lucide")
            .expect("manifest should declare the Lucide source set");
        assert_eq!(lucide.version, "0.468.0");
        assert_eq!(lucide.license, "ISC");
        assert_eq!(lucide.license_path, "ui/icons/LICENSE-LUCIDE.txt");
        assert_eq!(
            lucide.source_base_url,
            "https://github.com/lucide-icons/lucide/tree/0.468.0/icons"
        );

        let mut manifest_ids = HashSet::new();
        let mut manifest_paths = HashSet::new();
        for icon in &manifest.icons {
            assert!(manifest_ids.insert(icon.id.as_str()), "duplicate icon id");
            assert!(
                manifest_paths.insert(icon.path.as_str()),
                "duplicate icon path"
            );
            assert!(!icon.source.is_empty(), "icon source must be recorded");
        }

        for descriptor in UI_ICON_DESCRIPTORS {
            let entry = manifest
                .icons
                .iter()
                .find(|entry| entry.id == descriptor.id.as_str())
                .unwrap_or_else(|| panic!("manifest must define {}", descriptor.id.as_str()));
            assert_eq!(entry.path, descriptor.path);
            assert_eq!(
                entry.tint_policy,
                match descriptor.tint_policy {
                    UiIconTintPolicy::MonochromeTintable => "monochrome_tintable",
                    UiIconTintPolicy::FullColor => "full_color",
                }
            );

            let bytes = fs::read(asset_root.join(descriptor.path))
                .unwrap_or_else(|error| panic!("{} must exist: {error}", descriptor.path));
            assert_eq!(&bytes[0..8], b"\x89PNG\r\n\x1a\n");
            assert_eq!(
                u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
                descriptor.source_size.x
            );
            assert_eq!(
                u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
                descriptor.source_size.y
            );

            let hash = format!("{:x}", Sha256::digest(&bytes));
            assert_eq!(entry.sha256, hash, "hash mismatch for {}", descriptor.path);
        }
    }

    #[test]
    fn missing_resource_path_switches_to_stable_visible_placeholder() {
        let fallback = asset_unavailable_resolution(UiIconId::ADD);

        assert_eq!(fallback.requested, UiIconId::ADD);
        assert_eq!(fallback.rendered, UiIconId::MISSING);
        assert_eq!(fallback.path, "ui/icons/missing.png");
        assert_eq!(fallback.tint_policy, UiIconTintPolicy::FullColor);
        assert_eq!(fallback.fallback, Some(UiIconError::AssetUnavailable));
        assert_eq!(
            UiIconError::AssetUnavailable.code(),
            "icon_asset_unavailable"
        );
        assert_eq!(UiIconError::UnknownId.code(), "unknown_icon_id");
    }
}
