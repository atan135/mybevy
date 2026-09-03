// target.rs - Stage 3: Image resource management for UI 3D preview
// This is the only file allowed to touch Assets<Image> / Handle<Image>.

use bevy::{image::Image, prelude::*, render::render_resource::TextureFormat};
use std::collections::HashMap;

use super::config::PreviewPixelScale;

// ─────────────────────────────────────────────────────────────────────────────
// SessionId
// ─────────────────────────────────────────────────────────────────────────────

/// Stable identifier for a single preview viewport session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Preview3dSessionId(pub u64);

// ─────────────────────────────────────────────────────────────────────────────
// SessionCounter
// ─────────────────────────────────────────────────────────────────────────────

/// Monotonically increasing counter for session IDs. Starts at 1 to avoid 0.
#[derive(Resource)]
pub struct Preview3dSessionCounter {
    next_id: u64,
}

impl Default for Preview3dSessionCounter {
    fn default() -> Self {
        Self { next_id: 1 }
    }
}

impl Preview3dSessionCounter {
    /// Allocate and return the next session ID, then advance the counter.
    pub fn next(&mut self) -> Preview3dSessionId {
        let id = Preview3dSessionId(self.next_id);
        self.next_id += 1;
        id
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PreviewImageEntry
// ─────────────────────────────────────────────────────────────────────────────

/// Metadata for a registered preview image.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewImageEntry {
    /// Handle to the render-target Image.
    pub handle: Handle<Image>,
    /// Logical pixel size of the UI node at the time of creation / last resize.
    pub logical_size: Vec2,
    /// Physical pixel size of the GPU texture.
    pub physical_size: UVec2,
}

// ─────────────────────────────────────────────────────────────────────────────
// PreviewImageRegistry
// ─────────────────────────────────────────────────────────────────────────────

/// Registry mapping session IDs to their preview image entries.
#[derive(Resource, Default)]
pub struct PreviewImageRegistry {
    entries: HashMap<Preview3dSessionId, PreviewImageEntry>,
}

impl PreviewImageRegistry {
    /// Register (or overwrite) an entry for the given session.
    pub fn register(&mut self, session_id: Preview3dSessionId, entry: PreviewImageEntry) {
        self.entries.insert(session_id, entry);
    }

    /// Remove and return the entry for the given session, if any.
    pub fn unregister(&mut self, session_id: Preview3dSessionId) -> Option<PreviewImageEntry> {
        self.entries.remove(&session_id)
    }

    /// Borrow the entry for the given session, if any.
    pub fn get(&self, session_id: Preview3dSessionId) -> Option<&PreviewImageEntry> {
        self.entries.get(&session_id)
    }

    /// Update the logical and physical size for an existing entry.
    /// Returns None if the session has no registered entry.
    pub fn update_size(
        &mut self,
        session_id: Preview3dSessionId,
        logical_size: Vec2,
        physical_size: UVec2,
    ) -> Option<()> {
        if let Some(entry) = self.entries.get_mut(&session_id) {
            entry.logical_size = logical_size;
            entry.physical_size = physical_size;
            Some(())
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Pure utility functions
// ─────────────────────────────────────────────────────────────────────────────

/// Compute physical pixel dimensions from logical size and device scale.
/// Minimum 1x1 even if logical is sub-pixel.
pub fn compute_physical_size(logical_size: Vec2, device_scale: f32) -> UVec2 {
    if device_scale <= 0.0 {
        // Defensive fallback: treat as 1:1
        return UVec2::new(logical_size.x.ceil() as u32, logical_size.y.ceil() as u32);
    }
    let w = (logical_size.x * device_scale).ceil() as u32;
    let h = (logical_size.y * device_scale).ceil() as u32;
    UVec2::new(w.max(1), h.max(1))
}

// ─────────────────────────────────────────────────────────────────────────────
// Image lifecycle functions
// ─────────────────────────────────────────────────────────────────────────────

/// Create a new render-target image for a preview session.
/// Does NOT register the image in the registry; callers decide when to register.
pub fn create_preview_image(
    images: &mut Assets<Image>,
    _session_id: Preview3dSessionId,
    logical_size: Vec2,
    pixel_scale: PreviewPixelScale,
) -> PreviewImageEntry {
    let device_scale = pixel_scale.device_scale(logical_size);
    let physical_size = compute_physical_size(logical_size, device_scale);

    let image = Image::new_target_texture(
        physical_size.x,
        physical_size.y,
        TextureFormat::Rgba16Float,
        None,
    );

    let handle = images.add(image);

    PreviewImageEntry {
        handle,
        logical_size,
        physical_size,
    }
}

/// Resize an existing preview image entry.
/// Releases the old image from assets, creates a new one at the new size,
/// and returns the updated entry without touching the registry.
pub fn resize_preview_image(
    images: &mut Assets<Image>,
    current: PreviewImageEntry,
    new_logical_size: Vec2,
    pixel_scale: PreviewPixelScale,
) -> PreviewImageEntry {
    let device_scale = pixel_scale.device_scale(new_logical_size);
    let new_physical = compute_physical_size(new_logical_size, device_scale);

    // Release the old GPU texture
    images.remove(&current.handle);

    // Create a new render-target image at the updated resolution
    let image = Image::new_target_texture(
        new_physical.x,
        new_physical.y,
        TextureFormat::Rgba16Float,
        None,
    );
    let handle = images.add(image);

    PreviewImageEntry {
        handle,
        logical_size: new_logical_size,
        physical_size: new_physical,
    }
}

/// Release a preview image handle from assets.
/// Idempotent: removing a non-present handle does not panic.
pub fn release_preview_image(images: &mut Assets<Image>, handle: &Handle<Image>) {
    images.remove(handle);
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── compute_physical_size ──────────────────────────────────────────────

    #[test]
    fn compute_physical_size_scale_2x() {
        let physical = compute_physical_size(Vec2::new(100.0, 200.0), 2.0);
        assert_eq!(physical, UVec2::new(200, 400));
    }

    #[test]
    fn compute_physical_size_ceil_rounding() {
        let physical = compute_physical_size(Vec2::new(101.0, 101.0), 2.0);
        assert_eq!(physical, UVec2::new(202, 202));
    }

    #[test]
    fn compute_physical_size_min_1x1() {
        let physical = compute_physical_size(Vec2::new(0.5, 0.5), 1.0);
        assert_eq!(physical, UVec2::new(1, 1));
    }

    #[test]
    fn compute_physical_size_zero_scale_fallback() {
        let physical = compute_physical_size(Vec2::new(50.0, 80.0), 0.0);
        assert_eq!(physical, UVec2::new(50, 80));
    }

    // ── SessionCounter ─────────────────────────────────────────────────────

    #[test]
    fn session_counter_starts_at_1() {
        let mut counter = Preview3dSessionCounter::default();
        assert_eq!(counter.next().0, 1);
    }

    #[test]
    fn session_counter_increments() {
        let mut counter = Preview3dSessionCounter::default();
        assert_eq!(counter.next().0, 1);
        assert_eq!(counter.next().0, 2);
        assert_eq!(counter.next().0, 3);
    }

    // ── Registry ───────────────────────────────────────────────────────────

    #[test]
    fn registry_register_get_unregister() {
        let mut registry = PreviewImageRegistry::default();

        let id = Preview3dSessionId(42);
        let entry = PreviewImageEntry {
            handle: Handle::default(),
            logical_size: Vec2::new(100.0, 200.0),
            physical_size: UVec2::new(200, 400),
        };

        registry.register(id, entry.clone());
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.get(id), Some(&entry));

        let removed = registry.unregister(id);
        assert_eq!(removed.as_ref(), Some(&entry));
        assert!(registry.get(id).is_none());
        assert!(registry.is_empty());
    }

    #[test]
    fn registry_update_size_returns_none_for_unknown_session() {
        let mut registry = PreviewImageRegistry::default();
        let result = registry.update_size(
            Preview3dSessionId(99),
            Vec2::new(100.0, 100.0),
            UVec2::new(100, 100),
        );
        assert!(result.is_none());
    }

    // ── Image creation / release ──────────────────────────────────────────

    #[test]
    fn create_returns_entry_with_valid_handle() {
        let mut app = App::new();
        app.world_mut().init_resource::<Assets<Image>>();
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();

        let entry = create_preview_image(
            &mut images,
            Preview3dSessionId(1),
            Vec2::new(100.0, 200.0),
            PreviewPixelScale::DeviceScale(2.0),
        );

        assert!(images.get(&entry.handle).is_some());
        assert_eq!(entry.logical_size, Vec2::new(100.0, 200.0));
        assert_eq!(entry.physical_size, UVec2::new(200, 400));
    }

    #[test]
    fn resize_releases_old_handle() {
        let mut app = App::new();
        app.world_mut().init_resource::<Assets<Image>>();
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();

        let old_entry = create_preview_image(
            &mut images,
            Preview3dSessionId(1),
            Vec2::new(100.0, 200.0),
            PreviewPixelScale::DeviceScale(2.0),
        );
        let old_handle = old_entry.handle.clone();

        let new_entry = resize_preview_image(
            &mut images,
            old_entry,
            Vec2::new(50.0, 100.0),
            PreviewPixelScale::DeviceScale(1.0),
        );

        // Old handle should no longer be in assets
        assert!(images.get(&old_handle).is_none());
        // New handle should be present
        assert!(images.get(&new_entry.handle).is_some());
        assert_eq!(new_entry.logical_size, Vec2::new(50.0, 100.0));
        assert_eq!(new_entry.physical_size, UVec2::new(50, 100));
    }

    #[test]
    fn release_is_idempotent() {
        let mut app = App::new();
        app.world_mut().init_resource::<Assets<Image>>();
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();

        let entry = create_preview_image(
            &mut images,
            Preview3dSessionId(1),
            Vec2::new(100.0, 100.0),
            PreviewPixelScale::Logical,
        );

        // Release once
        release_preview_image(&mut images, &entry.handle);
        assert!(images.get(&entry.handle).is_none());

        // Release again - must not panic
        release_preview_image(&mut images, &entry.handle);
        assert!(images.get(&entry.handle).is_none());
    }
}
