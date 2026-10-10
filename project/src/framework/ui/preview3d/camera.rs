// camera.rs - Stage 4: projection camera & RenderLayers allocation for UI 3D preview
// This is the only file allowed to touch Camera3d / Camera / Projection / RenderLayers / Transform.

use bevy::{
    camera::{ImageRenderTarget, RenderTarget, visibility::RenderLayers},
    prelude::*,
};

use super::config::PreviewCameraConfig;
use super::target::Preview3dSessionId;

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

pub(super) const PREVIEW3D_CAMERA_ORDER: isize = -2; // below SCENE_CAMERA_3D_ORDER (-1)
pub(super) const PREVIEW3D_LAYER_BASE: usize = 24;
pub(super) const PREVIEW3D_LAYER_MAX: usize = 31;

// ─────────────────────────────────────────────────────────────────────────────
// Layer allocator
// ─────────────────────────────────────────────────────────────────────────────

/// Errors that can arise from layer allocation.
#[derive(Clone, Debug)]
pub enum PreviewLayerError {
    /// No more layer slots available (24-31 exhausted).
    Exhausted,
    /// Attempted to release a layer outside the preview range.
    Invalid(usize),
}

impl std::fmt::Display for PreviewLayerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exhausted => write!(f, "preview layer space exhausted (24-31)"),
            Self::Invalid(n) => write!(f, "invalid layer {} (not in preview range 24-31)", n),
        }
    }
}

impl std::error::Error for PreviewLayerError {}

/// Allocates exclusive RenderLayers for preview cameras.
/// Layers 24-31 are reserved; released layers are NOT returned to the pool.
#[derive(Resource)]
pub struct Preview3dLayerAllocator {
    next: usize,
}

impl Default for Preview3dLayerAllocator {
    fn default() -> Self {
        Self {
            next: PREVIEW3D_LAYER_BASE,
        }
    }
}

impl Preview3dLayerAllocator {
    /// Allocate the next available layer number.
    pub fn allocate(&mut self) -> Result<usize, PreviewLayerError> {
        if self.next > PREVIEW3D_LAYER_MAX {
            return Err(PreviewLayerError::Exhausted);
        }
        let layer = self.next;
        self.next += 1;
        Ok(layer)
    }

    /// Release a layer. Idempotent; does NOT return the number to the free pool.
    pub fn release(&mut self, layer: usize) -> Result<(), PreviewLayerError> {
        if layer < PREVIEW3D_LAYER_BASE || layer > PREVIEW3D_LAYER_MAX {
            return Err(PreviewLayerError::Invalid(layer));
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Marker components
// ─────────────────────────────────────────────────────────────────────────────

/// Marker attached to every preview projection camera.
#[derive(Component)]
pub(super) struct Preview3dProjectionCamera;

/// Temporary marker for ownership tracking.
/// Will be renamed / merged with Preview3dOwned in stage 6.
#[derive(Component)]
pub(super) struct Preview3dCameraOwned(pub Preview3dSessionId);

// ─────────────────────────────────────────────────────────────────────────────
// Camera spawn / transform / despawn
// ─────────────────────────────────────────────────────────────────────────────

/// Spawn a 3-D projection camera for a preview session.
///
/// `Projection` is an enum component and does not implement `Bundle`, so the
/// concrete type is extracted via `match` and inserted separately.  Likewise
/// `ImageRenderTarget` does not implement `Bundle` and is inserted separately.
pub(super) fn spawn_projection_camera(
    commands: &mut Commands,
    session_id: Preview3dSessionId,
    layer: usize,
    image: Handle<Image>,
    config: &PreviewCameraConfig,
) -> Entity {
    let image_target = ImageRenderTarget {
        handle: image,
        scale_factor: 1.0,
    };

    let entity = commands
        .spawn((
            Camera3d::default(),
            Camera {
                order: PREVIEW3D_CAMERA_ORDER,
                clear_color: ClearColorConfig::Custom(config.clear_color),
                ..Default::default()
            },
            config.initial_transform.clone(),
            RenderLayers::from_layers(&[layer]),
            Preview3dProjectionCamera,
            Preview3dCameraOwned(session_id),
            Name::new("Preview3dProjectionCamera"),
        ))
        // Projection enum does not implement Bundle; insert concrete variant.
        .insert(match &config.projection {
            Projection::Perspective(p) => Projection::Perspective(p.clone()),
            Projection::Orthographic(o) => Projection::Orthographic(o.clone()),
            Projection::Custom(c) => Projection::Custom(c.clone()),
        })
        .insert(RenderTarget::Image(image_target))
        .id();

    entity
}

/// Update the world-space transform of a preview camera. Silently ignores missing entities.
pub(super) fn apply_camera_transform(
    world: &mut World,
    camera_entity: Entity,
    transform: Transform,
) {
    // Bevy 0.18: World::query takes 1 generic; filter is applied via query_filtered.
    let mut cameras = world.query_filtered::<&mut Transform, With<Preview3dProjectionCamera>>();
    if let Ok(mut cam_transform) = cameras.get_mut(world, camera_entity) {
        *cam_transform = transform;
    }
}

/// Despawn a preview projection camera.
pub(super) fn despawn_projection_camera(commands: &mut Commands, camera_entity: Entity) {
    commands.entity(camera_entity).despawn();
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::render_resource::TextureFormat;

    // ── LayerAllocator ─────────────────────────────────────────────────────

    #[test]
    fn layer_allocator_starts_at_24() {
        let alloc = Preview3dLayerAllocator::default();
        assert_eq!(alloc.next, 24);
    }

    #[test]
    fn layer_allocator_increments() {
        let mut alloc = Preview3dLayerAllocator::default();
        assert_eq!(alloc.allocate().unwrap(), 24);
        assert_eq!(alloc.allocate().unwrap(), 25);
        assert_eq!(alloc.allocate().unwrap(), 26);
    }

    #[test]
    fn layer_allocator_exhausted_after_max() {
        let mut alloc = Preview3dLayerAllocator::default();
        for i in 24..=31 {
            let result = alloc.allocate();
            assert!(result.is_ok(), "allocation {} should succeed", i);
        }
        assert!(matches!(
            alloc.allocate().unwrap_err(),
            PreviewLayerError::Exhausted
        ));
    }

    #[test]
    fn layer_allocator_release_does_not_reuse() {
        let mut alloc = Preview3dLayerAllocator::default();
        let _l1 = alloc.allocate().unwrap(); // 24
        let l2 = alloc.allocate().unwrap(); // 25
        let _l3 = alloc.allocate().unwrap(); // 26
        alloc.release(l2).unwrap();
        let l4 = alloc.allocate().unwrap(); // 27 (not 24 or 25)
        assert_eq!(l4, 27);
    }

    // ── PreviewLayerError ─────────────────────────────────────────────────

    #[test]
    fn preview_layer_error_display() {
        assert!(
            PreviewLayerError::Exhausted
                .to_string()
                .contains("exhausted")
        );
        assert!(PreviewLayerError::Invalid(99).to_string().contains("99"));
    }

    // ── apply_camera_transform ─────────────────────────────────────────────

    #[test]
    fn apply_camera_transform_silently_no_panic_on_missing() {
        let mut world = World::new();
        // from_raw_u32 returns Option<Entity>; use unwrap to get a valid entity
        // that still does not exist in the empty world.
        let fake = Entity::from_raw_u32(9999).unwrap();
        apply_camera_transform(&mut world, fake, Transform::from_xyz(1.0, 2.0, 3.0));
    }

    // ── despawn_projection_camera ─────────────────────────────────────────

    #[test]
    fn despawn_projection_camera_succeeds() {
        let mut app = App::new();
        let entity = app.world_mut().spawn(Preview3dProjectionCamera).id();
        assert!(app.world().get_entity(entity).is_ok());

        app.world_mut().entity_mut(entity).despawn();

        assert!(app.world().get_entity(entity).is_err());
    }
    #[test]
    fn spawn_projection_camera_sets_order_to_minus_two() {
        let mut world = World::new();
        let mut images = Assets::<Image>::default();
        let handle = images.add(Image::new_target_texture(
            1,
            1,
            TextureFormat::Rgba16Float,
            None,
        ));
        let config = PreviewCameraConfig {
            projection: Projection::Perspective(PerspectiveProjection::default()),
            initial_transform: Transform::default(),
            clear_color: Color::NONE,
            field_of_view: None,
        };
        let entity = spawn_projection_camera(
            &mut world.commands(),
            Preview3dSessionId(7),
            24,
            handle,
            &config,
        );
        world.flush();
        let cam = world.entity(entity).get::<Camera>().unwrap();
        assert_eq!(cam.order, -2, "preview camera order must be -2");
    }

    #[test]
    fn spawn_projection_camera_does_not_attach_is_default_ui_camera() {
        use bevy::ui::IsDefaultUiCamera;
        let mut world = World::new();
        let mut images = Assets::<Image>::default();
        let handle = images.add(Image::new_target_texture(
            1,
            1,
            TextureFormat::Rgba16Float,
            None,
        ));
        let config = PreviewCameraConfig {
            projection: Projection::Perspective(PerspectiveProjection::default()),
            initial_transform: Transform::default(),
            clear_color: Color::NONE,
            field_of_view: None,
        };
        let entity = spawn_projection_camera(
            &mut world.commands(),
            Preview3dSessionId(8),
            25,
            handle,
            &config,
        );
        world.flush();
        assert!(
            world.entity(entity).get::<IsDefaultUiCamera>().is_none(),
            "preview camera must NOT have IsDefaultUiCamera marker"
        );
    }

    #[test]
    fn spawn_projection_camera_does_not_attach_scene_owned() {
        use crate::framework::scene::SceneOwned;
        let mut world = World::new();
        let mut images = Assets::<Image>::default();
        let handle = images.add(Image::new_target_texture(
            1,
            1,
            TextureFormat::Rgba16Float,
            None,
        ));
        let config = PreviewCameraConfig {
            projection: Projection::Perspective(PerspectiveProjection::default()),
            initial_transform: Transform::default(),
            clear_color: Color::NONE,
            field_of_view: None,
        };
        let entity = spawn_projection_camera(
            &mut world.commands(),
            Preview3dSessionId(9),
            26,
            handle,
            &config,
        );
        world.flush();
        assert!(
            world.entity(entity).get::<SceneOwned>().is_none(),
            "preview camera must NOT have SceneOwned marker"
        );
    }

    #[test]
    fn spawn_projection_camera_attaches_ownership_marker() {
        let mut world = World::new();
        let mut images = Assets::<Image>::default();
        let handle = images.add(Image::new_target_texture(
            1,
            1,
            TextureFormat::Rgba16Float,
            None,
        ));
        let config = PreviewCameraConfig {
            projection: Projection::Perspective(PerspectiveProjection::default()),
            initial_transform: Transform::default(),
            clear_color: Color::NONE,
            field_of_view: None,
        };
        let session_id = Preview3dSessionId(42);
        let entity =
            spawn_projection_camera(&mut world.commands(), session_id, 24, handle, &config);
        world.flush();
        let marker = world
            .entity(entity)
            .get::<Preview3dCameraOwned>()
            .expect("preview camera must have ownership marker");
        assert_eq!(marker.0, session_id);
    }
}
