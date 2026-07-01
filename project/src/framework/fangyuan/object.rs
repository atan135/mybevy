use bevy::prelude::*;

/// Shared root state for a logical Fangyuan object entity.
///
/// This component stores only whole-object activation, visibility, translation,
/// and scale. Primitive data stays in `FangyuanPrimitiveSet`; render-only child
/// entities should not own this state.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct FangyuanObjectState {
    pub active: bool,
    pub visible: bool,
    pub root_translation: Vec3,
    pub root_scale: Vec3,
}

impl Default for FangyuanObjectState {
    fn default() -> Self {
        Self {
            active: true,
            visible: true,
            root_translation: Vec3::ZERO,
            root_scale: Vec3::ONE,
        }
    }
}

impl FangyuanObjectState {
    pub const fn new(root_translation: Vec3, root_scale: Vec3) -> Self {
        Self {
            active: true,
            visible: true,
            root_translation,
            root_scale,
        }
    }

    pub const fn with_flags(
        active: bool,
        visible: bool,
        root_translation: Vec3,
        root_scale: Vec3,
    ) -> Self {
        Self {
            active,
            visible,
            root_translation,
            root_scale,
        }
    }

    pub const fn from_translation(root_translation: Vec3) -> Self {
        Self::new(root_translation, Vec3::ONE)
    }
}

#[cfg(test)]
mod tests {
    use super::super::primitive::FangyuanPrimitiveSet;
    use super::*;

    #[test]
    fn fangyuan_object_state_default_is_active_visible_identity_root() {
        let state = FangyuanObjectState::default();

        let FangyuanObjectState {
            active,
            visible,
            root_translation,
            root_scale,
        } = state;

        assert!(active);
        assert!(visible);
        assert_eq!(root_translation, Vec3::ZERO);
        assert_eq!(root_scale, Vec3::ONE);
    }

    #[test]
    fn fangyuan_primitive_set_and_object_state_share_logical_root_entity() {
        let mut app = App::new();
        let entity = app
            .world_mut()
            .spawn((FangyuanPrimitiveSet::new(), FangyuanObjectState::default()))
            .id();

        assert!(
            app.world()
                .entity(entity)
                .contains::<FangyuanPrimitiveSet>()
        );
        assert!(app.world().entity(entity).contains::<FangyuanObjectState>());

        let mut roots = app
            .world_mut()
            .query::<(&FangyuanPrimitiveSet, &FangyuanObjectState)>();
        let (primitive_set, object_state) = roots.single(app.world()).unwrap();
        assert!(primitive_set.is_empty());
        assert_eq!(*object_state, FangyuanObjectState::default());
    }

    #[test]
    fn fangyuan_object_state_exposes_only_root_translation_and_scale_for_transform_sync() {
        let state = FangyuanObjectState::new(Vec3::new(2.0, 3.0, -4.0), Vec3::new(1.5, 2.0, 0.5));
        let mut transform =
            Transform::from_translation(Vec3::new(-1.0, -1.0, -1.0)).with_scale(Vec3::splat(3.0));

        transform.translation = state.root_translation;
        transform.scale = state.root_scale;

        assert_eq!(transform.translation, state.root_translation);
        assert_eq!(transform.scale, state.root_scale);
    }

    #[test]
    fn fangyuan_object_state_constructor_preserves_flags() {
        let state = FangyuanObjectState::with_flags(
            false,
            false,
            Vec3::new(1.0, 0.0, 2.0),
            Vec3::splat(2.0),
        );

        assert!(!state.active);
        assert!(!state.visible);
        assert_eq!(state.root_translation, Vec3::new(1.0, 0.0, 2.0));
        assert_eq!(state.root_scale, Vec3::splat(2.0));
    }
}
