//! Bounded atlas gallery state for Fangyuan Home diagnostics.
//!
//! The gallery is a development view over shared atlas indices. It owns no
//! image handles or render entities and is intentionally independent from
//! authority, replay, and gameplay state.

use bevy::prelude::*;

use super::{
    FangyuanCatalogOrientation, FangyuanCatalogProjection, FangyuanCatalogRenderModel,
    FangyuanPrimitiveKind, FangyuanSurfacePresetRegistry,
    fangyuan_atlas_gallery_noise_tile_indices, fangyuan_atlas_gallery_reserved_noise_tile_indices,
    fangyuan_atlas_gallery_surface_tile_indices, fangyuan_atlas_tile_content_uv_rect,
    fangyuan_noise_tile_content_uv_rect, fangyuan_resolve_surface_projection,
    fangyuan_special_surface_fallback,
};

/// Canonical first-package preset catalog used by the Home diagnostics view.
/// Loading is lazy so tests and fallback HUDs can operate without filesystem access.
#[derive(Debug, Default, Resource)]
pub struct FangyuanAtlasGalleryCatalog {
    registry: Option<FangyuanSurfacePresetRegistry>,
}

impl FangyuanAtlasGalleryCatalog {
    pub fn ensure_loaded(&mut self) {
        if self.registry.is_none() {
            self.registry = super::load_first_package_fangyuan_surface_preset_registry().ok();
        }
    }

    pub fn registry(&self) -> Option<&FangyuanSurfacePresetRegistry> {
        self.registry.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FangyuanAtlasGalleryChannel {
    #[default]
    Final,
    Base,
    Normal,
    Noise,
}

impl FangyuanAtlasGalleryChannel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Final => "final",
            Self::Base => "base",
            Self::Normal => "normal",
            Self::Noise => "noise",
        }
    }
}

#[derive(Clone, Debug, Resource, PartialEq, Eq)]
pub struct FangyuanAtlasGalleryState {
    pub visible: bool,
    pub channel: FangyuanAtlasGalleryChannel,
    pub surface_tile_index: u16,
    pub noise_tile_index: Option<u16>,
    pub reserved_noise_tile_index: Option<u16>,
    pub selected_preset_id: Option<String>,
}

impl Default for FangyuanAtlasGalleryState {
    fn default() -> Self {
        Self {
            visible: false,
            channel: FangyuanAtlasGalleryChannel::Final,
            surface_tile_index: 0,
            noise_tile_index: fangyuan_atlas_gallery_noise_tile_indices().first().copied(),
            reserved_noise_tile_index: None,
            selected_preset_id: None,
        }
    }
}

impl FangyuanAtlasGalleryState {
    pub fn set_channel(&mut self, channel: FangyuanAtlasGalleryChannel) {
        self.channel = channel;
        self.visible = true;
    }

    pub fn toggle_visible(&mut self) {
        self.visible = !self.visible;
    }

    pub fn select_surface_tile(&mut self, index: u16) -> bool {
        if !fangyuan_atlas_gallery_surface_tile_indices().contains(&index) {
            return false;
        }
        self.surface_tile_index = index;
        self.visible = true;
        true
    }

    pub fn select_noise_tile(&mut self, index: Option<u16>) -> bool {
        if let Some(index) = index {
            if fangyuan_noise_tile_content_uv_rect(index).is_none() {
                return false;
            }
        }
        self.noise_tile_index = index;
        self.reserved_noise_tile_index = None;
        self.visible = true;
        true
    }

    /// Select a reserved/sentinel cell for diagnostics without ever exposing
    /// it as a shader sampling index.
    pub fn select_reserved_noise_tile(&mut self, index: u16) -> bool {
        if !fangyuan_atlas_gallery_reserved_noise_tile_indices().contains(&index) {
            return false;
        }
        self.noise_tile_index = None;
        self.reserved_noise_tile_index = Some(index);
        self.visible = true;
        true
    }

    pub fn select_preset(
        &mut self,
        registry: &FangyuanSurfacePresetRegistry,
        preset_id: &str,
    ) -> bool {
        let Some(preset) = registry.get(preset_id) else {
            return false;
        };
        self.selected_preset_id = Some(preset_id.to_string());
        self.surface_tile_index = preset.surface_tile_index;
        self.noise_tile_index = preset.noise_tile_index;
        self.reserved_noise_tile_index = None;
        self.visible = true;
        true
    }

    pub fn cycle_surface_tile(&mut self, delta: i32) {
        let count = fangyuan_atlas_gallery_surface_tile_indices().len() as i32;
        let next = (i32::from(self.surface_tile_index) + delta).rem_euclid(count);
        self.surface_tile_index = next as u16;
        self.visible = true;
    }

    pub fn cycle_noise_tile(&mut self, delta: i32) {
        let indices = fangyuan_atlas_gallery_noise_tile_indices();
        if indices.is_empty() {
            self.noise_tile_index = None;
            return;
        }
        let current = self
            .noise_tile_index
            .and_then(|index| indices.iter().position(|candidate| *candidate == index))
            .unwrap_or(0) as i32;
        let next = (current + delta).rem_euclid(indices.len() as i32);
        self.noise_tile_index = indices.get(next as usize).copied();
        self.reserved_noise_tile_index = None;
        self.visible = true;
    }

    pub fn reserved_noise_tile_count(&self) -> usize {
        fangyuan_atlas_gallery_reserved_noise_tile_indices().len()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanAtlasGallerySelection {
    pub requested_preset_id: Option<String>,
    pub resolved_preset_id: String,
    pub surface_tile_id: String,
    pub surface_tile_index: u16,
    pub surface_uv_rect: Option<(Vec2, Vec2)>,
    pub noise_tile_id: Option<String>,
    pub noise_tile_index: Option<u16>,
    pub noise_uv_rect: Option<(Vec2, Vec2)>,
    pub projection_requested: FangyuanCatalogProjection,
    pub projection: FangyuanCatalogProjection,
    pub projection_fallback_reason: Option<String>,
    pub orientation_policy: FangyuanCatalogOrientation,
    pub render_model: FangyuanCatalogRenderModel,
    pub render_model_fallback: Option<String>,
    pub roughness: f32,
    pub metallic: f32,
    pub reflectance: f32,
    pub ambient_occlusion: f32,
    pub fallback_reason: Option<String>,
    /// State-level noise selection diagnostics. This is separate from the
    /// resolver fallback because a gallery user may intentionally disable
    /// noise or inspect a reserved sentinel cell.
    pub noise_selection_fallback_reason: Option<String>,
    pub channel: FangyuanAtlasGalleryChannel,
}

impl FangyuanAtlasGallerySelection {
    pub fn format_line(&self) -> String {
        format!(
            "gallery channel={} preset={} resolved={} surface={}#{} uv={:?} noise={}#{:?} noise_uv={:?} noise_selection={} projection={:?}->{:?} projection_fallback={} orientation={:?} render={:?} render_fallback={} pbr=r{:.3} m{:.3} refl{:.3} ao{:.3} fallback={}",
            self.channel.as_str(),
            self.requested_preset_id.as_deref().unwrap_or("<default>"),
            self.resolved_preset_id,
            self.surface_tile_id,
            self.surface_tile_index,
            self.surface_uv_rect,
            self.noise_tile_id.as_deref().unwrap_or("<none>"),
            self.noise_tile_index,
            self.noise_uv_rect,
            self.noise_selection_fallback_reason
                .as_deref()
                .unwrap_or("none"),
            self.projection_requested,
            self.projection,
            self.projection_fallback_reason.as_deref().unwrap_or("none"),
            self.orientation_policy,
            self.render_model,
            self.render_model_fallback.as_deref().unwrap_or("none"),
            self.roughness,
            self.metallic,
            self.reflectance,
            self.ambient_occlusion,
            self.fallback_reason.as_deref().unwrap_or("none"),
        )
    }
}

/// Build one diagnostic selection from the canonical resolver. This function
/// performs no AssetServer work and remains suitable for a compact mobile HUD.
pub fn fangyuan_atlas_gallery_selection(
    registry: &FangyuanSurfacePresetRegistry,
    primitive_kind: FangyuanPrimitiveKind,
    requested_preset_id: Option<&str>,
    noise_enabled: bool,
    channel: FangyuanAtlasGalleryChannel,
) -> FangyuanAtlasGallerySelection {
    let resolution = registry.resolve_with_noise(requested_preset_id, noise_enabled, None);
    let preset = &resolution.preset;
    let projection = fangyuan_resolve_surface_projection(primitive_kind, preset.projection);
    let metadata = registry
        .catalog_metadata()
        .iter()
        .find(|metadata| metadata.preset_id == resolution.resolved_preset_id);
    let render_fallback = fangyuan_special_surface_fallback(
        preset.render_model,
        metadata
            .map(|metadata| metadata.material_family.as_str())
            .unwrap_or(""),
    );
    let mut fallback_reason = resolution
        .fallback_reason
        .as_ref()
        .map(|reason| format_surface_fallback_reason(reason));
    if fallback_reason.is_none() {
        fallback_reason = projection
            .fallback_reason
            .map(|reason| reason.as_str().to_string());
    }
    FangyuanAtlasGallerySelection {
        requested_preset_id: resolution.requested_preset_id,
        resolved_preset_id: resolution.resolved_preset_id,
        surface_tile_id: preset.surface_tile_id.clone(),
        surface_tile_index: preset.surface_tile_index,
        surface_uv_rect: fangyuan_atlas_tile_content_uv_rect(preset.surface_tile_index)
            .map(|rect| (rect.min, rect.max)),
        noise_tile_id: preset.noise_tile_id.clone(),
        noise_tile_index: preset.noise_tile_index,
        noise_uv_rect: preset
            .noise_tile_index
            .and_then(fangyuan_noise_tile_content_uv_rect),
        projection_requested: projection.requested,
        projection: projection.effective,
        projection_fallback_reason: projection
            .fallback_reason
            .map(|reason| reason.as_str().to_string()),
        orientation_policy: preset.orientation_policy,
        render_model: preset.render_model,
        render_model_fallback: render_fallback.map(|fallback| fallback.as_str().to_string()),
        roughness: preset.roughness,
        metallic: preset.metallic,
        reflectance: preset.reflectance,
        ambient_occlusion: preset.ambient_occlusion,
        fallback_reason,
        noise_selection_fallback_reason: None,
        channel,
    }
}

/// Resolve canonical preset semantics while projecting the currently selected
/// gallery cells into the diagnostic snapshot. The state setters bound these
/// indices before this helper is called, and reserved noise cells remain
/// explicitly rejected with no sampling UV.
pub fn fangyuan_atlas_gallery_selection_for_state(
    registry: &FangyuanSurfacePresetRegistry,
    primitive_kind: FangyuanPrimitiveKind,
    gallery: &FangyuanAtlasGalleryState,
) -> FangyuanAtlasGallerySelection {
    let mut selection = fangyuan_atlas_gallery_selection(
        registry,
        primitive_kind,
        gallery.selected_preset_id.as_deref(),
        gallery.noise_tile_index.is_some(),
        gallery.channel,
    );

    if let Some(rect) = fangyuan_atlas_tile_content_uv_rect(gallery.surface_tile_index) {
        selection.surface_tile_id = gallery_surface_tile_id(gallery.surface_tile_index);
        selection.surface_tile_index = gallery.surface_tile_index;
        selection.surface_uv_rect = Some((rect.min, rect.max));
    }

    match (gallery.noise_tile_index, gallery.reserved_noise_tile_index) {
        (Some(index), _) if fangyuan_noise_tile_content_uv_rect(index).is_some() => {
            selection.noise_tile_id = Some(gallery_noise_tile_id(index));
            selection.noise_tile_index = Some(index);
            selection.noise_uv_rect = fangyuan_noise_tile_content_uv_rect(index);
            selection.noise_selection_fallback_reason = None;
        }
        (None, Some(index)) => {
            selection.noise_tile_id = Some(gallery_noise_tile_id(index));
            selection.noise_tile_index = None;
            selection.noise_uv_rect = None;
            selection.noise_selection_fallback_reason =
                Some("reserved_noise_tile_rejected".to_string());
        }
        _ => {
            selection.noise_tile_id = None;
            selection.noise_tile_index = None;
            selection.noise_uv_rect = None;
            selection.noise_selection_fallback_reason = Some("noise_disabled".to_string());
        }
    }

    selection
}

fn gallery_surface_tile_id(index: u16) -> String {
    format!(
        "surface_x{:02}_y{:02}",
        index % super::FANGYUAN_ATLAS_COLUMNS,
        index / super::FANGYUAN_ATLAS_COLUMNS
    )
}

fn gallery_noise_tile_id(index: u16) -> String {
    format!(
        "noise_x{:02}_y{:02}",
        index % super::FANGYUAN_ATLAS_COLUMNS,
        index / super::FANGYUAN_ATLAS_COLUMNS
    )
}

fn format_surface_fallback_reason(reason: &super::FangyuanSurfacePresetFallbackReason) -> String {
    match reason {
        super::FangyuanSurfacePresetFallbackReason::UnknownPresetId => "unknown_preset_id",
        super::FangyuanSurfacePresetFallbackReason::InvalidPresetId => "invalid_preset_id",
        super::FangyuanSurfacePresetFallbackReason::NoiseSuggestionUnavailable => {
            "noise_suggestion_unavailable"
        }
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::load_first_package_fangyuan_surface_preset_registry;

    #[test]
    fn gallery_is_bounded_and_rejects_reserved_noise_cells() {
        assert_eq!(fangyuan_atlas_gallery_surface_tile_indices().len(), 256);
        assert_eq!(fangyuan_atlas_gallery_noise_tile_indices().len(), 122);
        assert_eq!(
            fangyuan_atlas_gallery_reserved_noise_tile_indices().len(),
            134
        );
        let mut state = FangyuanAtlasGalleryState::default();
        assert!(!state.visible);
        state.cycle_surface_tile(-1);
        assert_eq!(state.surface_tile_index, 255);
        state.cycle_noise_tile(-1);
        assert!(state.noise_tile_index.is_some());
        assert!(!state.select_noise_tile(Some(255)));
        assert!(state.select_reserved_noise_tile(255));
        assert_eq!(state.noise_tile_index, None);
        assert!(state.select_noise_tile(None));
    }

    #[test]
    fn gallery_selection_contains_projection_uv_pbr_and_fallback_diagnostics() {
        let registry = load_first_package_fangyuan_surface_preset_registry().unwrap();
        let selection = fangyuan_atlas_gallery_selection(
            &registry,
            FangyuanPrimitiveKind::Cube,
            Some("material:surface/x00/y00"),
            true,
            FangyuanAtlasGalleryChannel::Final,
        );
        assert_eq!(selection.surface_tile_index, 0);
        assert!(selection.surface_uv_rect.is_some());
        assert!(selection.format_line().contains("projection="));
        assert!(selection.format_line().contains("pbr="));
    }
}
