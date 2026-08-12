//! Shared runtime rendering boundary for Fangyuan player-like objects.
//!
//! This module only understands compiled blueprints (`FangyuanAvatar` and
//! `FangyuanPrimitiveSet`). Game-layer systems own identity, networking and
//! lifecycle policy; they provide an optional bundle when spawning a root.

use bevy::{prelude::*, transform::TransformSystems};

use super::{
    FangyuanAvatar, FangyuanObjectState, FangyuanPrimitiveKind, FangyuanPrimitiveSet,
    FangyuanRenderAssetCache, fangyuan_render_transform_from_primitive,
};

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FangyuanPlayer;

#[derive(Component, Clone, Debug, PartialEq)]
pub struct FangyuanPlayerState {
    pub active: bool,
}

impl Default for FangyuanPlayerState {
    fn default() -> Self {
        Self { active: true }
    }
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct FangyuanPlayerPosition {
    pub translation: Vec3,
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FangyuanPlayerVisualsSpawned;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct FangyuanPlayerPrimitiveVisual {
    pub kind: FangyuanPrimitiveKind,
    pub index: usize,
    pub alpha: f32,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct FangyuanPlayerRenderAssets {
    pub cache: FangyuanRenderAssetCache,
}

impl FangyuanPlayerRenderAssets {
    #[cfg(test)]
    pub fn material_count(&self) -> usize {
        self.cache.material_count()
    }

    #[cfg(test)]
    pub fn unit_cube_mesh(&self) -> Option<&Handle<Mesh>> {
        self.cache.unit_cube_mesh()
    }

    #[cfg(test)]
    pub fn unit_sphere_mesh(&self) -> Option<&Handle<Mesh>> {
        self.cache.unit_sphere_mesh()
    }
}

/// Adds shared player visual generation and root Transform synchronization.
pub struct FangyuanPlayerRuntimePlugin;

impl Plugin for FangyuanPlayerRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<FangyuanPlayerRenderAssets>()
            .add_systems(
                PostUpdate,
                (
                    spawn_fangyuan_player_primitive_visuals,
                    sync_fangyuan_player_transform,
                )
                    .chain()
                    .before(TransformSystems::Propagate),
            );
    }
}

/// Spawn a logical player root and attach an optional game-layer bundle.
pub fn spawn_fangyuan_player<B: Bundle>(
    commands: &mut Commands,
    blueprint_id: impl Into<String>,
    display_name: impl Into<String>,
    primitive_set: FangyuanPrimitiveSet,
    transform: Transform,
    extra: B,
) -> Entity {
    let translation = transform.translation;
    let scale = transform.scale;
    commands
        .spawn((
            extra,
            FangyuanPlayer,
            FangyuanPlayerState::default(),
            FangyuanPlayerPosition { translation },
            FangyuanObjectState::new(translation, scale),
            transform,
            GlobalTransform::default(),
            FangyuanAvatar::new(blueprint_id, display_name, primitive_set.clone()),
            primitive_set,
        ))
        .id()
}

fn spawn_fangyuan_player_primitive_visuals(
    mut commands: Commands,
    players: Query<
        (Entity, &FangyuanPrimitiveSet),
        (With<FangyuanPlayer>, Without<FangyuanPlayerVisualsSpawned>),
    >,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut render_assets: ResMut<FangyuanPlayerRenderAssets>,
) {
    for (player, primitive_set) in &players {
        for (index, primitive) in primitive_set.primitives().iter().enumerate() {
            let mesh = render_assets.cache.unit_mesh(primitive.kind, &mut meshes);
            let material = render_assets
                .cache
                .material(primitive.color, &mut materials);
            let visual = commands
                .spawn((
                    FangyuanPlayerPrimitiveVisual {
                        kind: primitive.kind,
                        index,
                        alpha: primitive.alpha,
                    },
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    fangyuan_render_transform_from_primitive(primitive),
                    Visibility::Visible,
                ))
                .id();
            commands.entity(player).add_child(visual);
        }
        commands.entity(player).insert(FangyuanPlayerVisualsSpawned);
    }
}

fn sync_fangyuan_player_transform(
    mut players: Query<
        (
            &FangyuanPlayerPosition,
            &mut FangyuanObjectState,
            &mut Transform,
        ),
        With<FangyuanPlayer>,
    >,
) {
    for (position, mut state, mut transform) in &mut players {
        state.root_translation = position.translation;
        transform.translation = state.root_translation;
        transform.scale = state.root_scale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_runtime_does_not_reference_game_or_network_modules() {
        let source = include_str!("runtime.rs");
        assert!(!source.contains(concat!("crate::", "game")));
        assert!(!source.contains(concat!("My", "Server")));
        assert!(!source.contains(concat!("main", "_world")));
    }

    #[test]
    fn spawn_api_preserves_blueprint_display_name_transform_and_extra_bundle() {
        #[derive(Component)]
        struct Extra;
        let mut app = App::new();
        let primitive_set = FangyuanPrimitiveSet::new();
        let mut commands = app.world_mut().commands();
        let entity = spawn_fangyuan_player(
            &mut commands,
            "bp/test",
            "Test",
            primitive_set,
            Transform::from_xyz(1.0, 2.0, 3.0).with_scale(Vec3::splat(2.0)),
            Extra,
        );
        app.update();
        let avatar = app.world().get::<FangyuanAvatar>(entity).unwrap();
        assert_eq!(avatar.blueprint_id, "bp/test");
        assert_eq!(avatar.display_name, "Test");
        assert!(app.world().get::<Extra>(entity).is_some());
        assert_eq!(
            app.world().get::<Transform>(entity).unwrap().translation,
            Vec3::new(1.0, 2.0, 3.0)
        );
    }

    #[test]
    fn transform_sync_preserves_game_layer_rotation() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, FangyuanPlayerRuntimePlugin));
        let rotation = Quat::from_rotation_y(0.75);
        let entity = app
            .world_mut()
            .spawn((
                FangyuanPlayer,
                FangyuanPlayerPosition::default(),
                FangyuanObjectState::default(),
                FangyuanPrimitiveSet::new(),
                Transform::from_rotation(rotation),
                GlobalTransform::default(),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Transform>(entity).unwrap().rotation,
            rotation
        );
    }
}
