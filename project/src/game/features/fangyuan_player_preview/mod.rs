use bevy::{prelude::*, transform::TransformSystems};

use crate::framework::fangyuan::{
    FANGYUAN_MINIMAL_PLAYER_BLUEPRINT_PATH, FangyuanAvatar, FangyuanObjectState,
    FangyuanPrimitiveKind, FangyuanPrimitiveSet, FangyuanRenderAssetCache,
    fangyuan_render_transform_from_primitive, load_fangyuan_minimal_player_primitive_set_or_log,
};
use crate::game::navigation::AppUiMode;

pub(in crate::game) struct FangyuanPlayerPreviewPlugin;

impl Plugin for FangyuanPlayerPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<FangyuanPlayerPreviewRenderAssets>()
            .add_systems(
                OnEnter(AppUiMode::FangyuanPlayerPreview),
                spawn_fangyuan_preview_player,
            )
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

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::game) struct FangyuanPlayer;

#[derive(Component, Clone, Debug, PartialEq)]
pub(in crate::game) struct FangyuanPlayerState {
    pub active: bool,
}

impl Default for FangyuanPlayerState {
    fn default() -> Self {
        Self { active: true }
    }
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::game) struct FangyuanPlayerPosition {
    pub translation: Vec3,
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
struct FangyuanPlayerVisualsSpawned;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
struct FangyuanPlayerPrimitiveVisual {
    kind: FangyuanPrimitiveKind,
    index: usize,
    alpha: f32,
}

#[derive(Clone, Debug, Resource, Default)]
struct FangyuanPlayerPreviewRenderAssets {
    cache: FangyuanRenderAssetCache,
}

impl FangyuanPlayerPreviewRenderAssets {
    fn unit_mesh(
        &mut self,
        kind: FangyuanPrimitiveKind,
        meshes: &mut Assets<Mesh>,
    ) -> Handle<Mesh> {
        self.cache.unit_mesh(kind, meshes)
    }

    fn material(
        &mut self,
        color: Color,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        self.cache.material(color, materials)
    }

    #[cfg(test)]
    fn material_count(&self) -> usize {
        self.cache.material_count()
    }

    #[cfg(test)]
    fn unit_cube_mesh(&self) -> Option<&Handle<Mesh>> {
        self.cache.unit_cube_mesh()
    }

    #[cfg(test)]
    fn unit_sphere_mesh(&self) -> Option<&Handle<Mesh>> {
        self.cache.unit_sphere_mesh()
    }
}

fn spawn_fangyuan_preview_player(mut commands: Commands, players: Query<(), With<FangyuanPlayer>>) {
    if !players.is_empty() {
        return;
    }

    let Some(primitive_set) = load_fangyuan_minimal_player_primitive_set_or_log() else {
        return;
    };

    let position = FangyuanPlayerPosition::default();
    let object_state = FangyuanObjectState::from_translation(position.translation);
    commands.spawn((
        DespawnOnExit(AppUiMode::FangyuanPlayerPreview),
        FangyuanPlayer,
        FangyuanPlayerState::default(),
        position,
        object_state,
        Transform::from_translation(object_state.root_translation)
            .with_scale(object_state.root_scale),
        GlobalTransform::default(),
        FangyuanAvatar::new(
            FANGYUAN_MINIMAL_PLAYER_BLUEPRINT_PATH,
            "Minimal Fangyuan Player",
            primitive_set.clone(),
        ),
        primitive_set,
    ));
}

fn spawn_fangyuan_player_primitive_visuals(
    mut commands: Commands,
    players: Query<
        (Entity, &FangyuanPrimitiveSet),
        (With<FangyuanPlayer>, Without<FangyuanPlayerVisualsSpawned>),
    >,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut render_assets: ResMut<FangyuanPlayerPreviewRenderAssets>,
) {
    for (player, primitive_set) in &players {
        for (index, primitive) in primitive_set.primitives().iter().enumerate() {
            let mesh = render_assets.unit_mesh(primitive.kind, &mut meshes);
            let material = render_assets.material(primitive.color, &mut materials);
            let transform = fangyuan_render_transform_from_primitive(primitive);
            let visual = commands
                .spawn((
                    FangyuanPlayerPrimitiveVisual {
                        kind: primitive.kind,
                        index,
                        alpha: primitive.alpha,
                    },
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    transform,
                    Visibility::Visible,
                    Name::new(format!(
                        "FangyuanPlayerPrimitiveVisual({}:{index})",
                        primitive.kind.as_str()
                    )),
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
    for (position, mut object_state, mut transform) in &mut players {
        object_state.root_translation = position.translation;
        transform.translation = object_state.root_translation;
        transform.scale = object_state.root_scale;
        transform.rotation = Quat::IDENTITY;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::{
        FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT, FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE,
        FangyuanPrimitive, FangyuanPrimitiveLifecycle, FangyuanPrimitiveRole,
    };

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::state::app::StatesPlugin,
            TransformPlugin,
        ))
        .init_state::<AppUiMode>()
        .add_plugins(FangyuanPlayerPreviewPlugin);
        app
    }

    fn enter_preview_mode(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<AppUiMode>>()
            .set(AppUiMode::FangyuanPlayerPreview);
        app.update();
    }

    #[test]
    fn fangyuan_preview_plugin_spawns_one_player_entity() {
        let mut app = test_app();
        enter_preview_mode(&mut app);

        let players = fangyuan_player_entities(&mut app);
        assert_eq!(players.len(), 1);
    }

    #[test]
    fn fangyuan_preview_player_spawn_is_idempotent() {
        let mut app = test_app();
        enter_preview_mode(&mut app);
        app.update();

        let players = fangyuan_player_entities(&mut app);
        assert_eq!(players.len(), 1);
    }

    #[test]
    fn fangyuan_preview_player_only_spawns_after_entering_preview_mode() {
        let mut app = test_app();
        app.update();

        assert!(fangyuan_player_entities(&mut app).is_empty());

        enter_preview_mode(&mut app);

        assert_eq!(fangyuan_player_entities(&mut app).len(), 1);
    }

    #[test]
    fn fangyuan_preview_player_has_required_components() {
        let mut app = test_app();
        enter_preview_mode(&mut app);

        let mut players = app.world_mut().query::<(
            &FangyuanPlayer,
            &FangyuanPlayerState,
            &FangyuanPlayerPosition,
            &FangyuanObjectState,
            &FangyuanAvatar,
            &FangyuanPrimitiveSet,
            &Transform,
        )>();
        let (_, state, position, object_state, avatar, primitive_set, transform) =
            players.single(app.world()).unwrap();

        assert!(state.active);
        assert!(object_state.active);
        assert!(object_state.visible);
        assert_eq!(position.translation, Vec3::ZERO);
        assert_eq!(object_state.root_translation, position.translation);
        assert_eq!(object_state.root_scale, Vec3::ONE);
        assert_eq!(transform.translation, position.translation);
        assert_eq!(transform.scale, object_state.root_scale);
        assert_eq!(transform.rotation, Quat::IDENTITY);
        assert_eq!(avatar.blueprint_id, FANGYUAN_MINIMAL_PLAYER_BLUEPRINT_PATH);
        assert_eq!(avatar.display_name, "Minimal Fangyuan Player");
        assert_eq!(
            avatar.primitives.len(),
            FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT
        );
        assert_eq!(primitive_set.len(), FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT);
        assert_eq!(&avatar.primitives, primitive_set);
    }

    #[test]
    fn fangyuan_preview_player_uses_minimal_runtime_primitive_defaults() {
        let mut app = test_app();
        enter_preview_mode(&mut app);

        let player = fangyuan_player_entities(&mut app)[0];
        let primitive_set = app.world().get::<FangyuanPrimitiveSet>(player).unwrap();
        let primitives = primitive_set.primitives();

        assert_eq!(primitives.len(), FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT);
        assert_eq!(primitives[0].kind, FangyuanPrimitiveKind::Cube);
        assert_eq!(primitives[0].role, FangyuanPrimitiveRole::Structure);
        assert_eq!(primitives[0].local_position, Vec3::new(0.0, 0.75, 0.0));
        assert_eq!(primitives[0].scale, Vec3::new(0.9, 1.5, 0.6));
        assert_eq!(
            primitives[0].color.to_srgba(),
            Color::srgba(0.25, 0.45, 0.95, 1.0).to_srgba()
        );

        assert_eq!(primitives[1].kind, FangyuanPrimitiveKind::Sphere);
        assert_eq!(primitives[1].role, FangyuanPrimitiveRole::Core);
        assert_eq!(primitives[1].local_position, Vec3::new(0.0, 1.75, 0.0));
        assert_eq!(primitives[1].scale, Vec3::splat(0.7));
        assert_eq!(
            primitives[1].color.to_srgba(),
            Color::srgba(0.95, 0.78, 0.55, 1.0).to_srgba()
        );

        for primitive in primitives {
            let color = primitive.color.to_srgba();
            assert_eq!(primitive.alpha, color.alpha);
            assert_eq!(primitive.emissive, FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE);
            assert_eq!(primitive.material_profile_id, None);
            assert_eq!(primitive.lifecycle, FangyuanPrimitiveLifecycle::empty());
        }
    }

    #[test]
    fn fangyuan_player_position_only_exposes_translation() {
        let position = FangyuanPlayerPosition {
            translation: Vec3::new(2.0, 0.0, -3.0),
        };

        assert_eq!(position.translation, Vec3::new(2.0, 0.0, -3.0));
    }

    #[test]
    fn moving_player_position_updates_root_state_and_transform_without_rotation() {
        let mut app = test_app();
        enter_preview_mode(&mut app);
        let player = fangyuan_player_entities(&mut app)[0];

        app.world_mut()
            .get_mut::<FangyuanPlayerPosition>(player)
            .unwrap()
            .translation = Vec3::new(4.0, 0.0, -2.0);
        app.world_mut()
            .get_mut::<FangyuanObjectState>(player)
            .unwrap()
            .root_scale = Vec3::new(1.25, 1.5, 0.75);
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .rotation = Quat::from_rotation_y(1.0);

        app.update();

        let object_state = app.world().get::<FangyuanObjectState>(player).unwrap();
        let transform = app.world().get::<Transform>(player).unwrap();
        assert_eq!(object_state.root_translation, Vec3::new(4.0, 0.0, -2.0));
        assert_eq!(object_state.root_scale, Vec3::new(1.25, 1.5, 0.75));
        assert_eq!(transform.translation, Vec3::new(4.0, 0.0, -2.0));
        assert_eq!(transform.scale, Vec3::new(1.25, 1.5, 0.75));
        assert_eq!(transform.rotation, Quat::IDENTITY);
    }

    #[test]
    fn primitives_remain_data_on_player_entity() {
        let mut app = test_app();
        enter_preview_mode(&mut app);

        let players = fangyuan_player_entities(&mut app);
        let mut primitive_sets = app.world_mut().query::<&FangyuanPrimitiveSet>();

        assert_eq!(players.len(), 1);
        assert_eq!(primitive_sets.iter(app.world()).count(), 1);
        assert_eq!(
            primitive_sets.single(app.world()).unwrap().len(),
            FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT
        );
    }

    #[test]
    fn fangyuan_preview_player_spawns_render_only_visual_children() {
        let mut app = test_app();
        enter_preview_mode(&mut app);

        let player = fangyuan_player_entities(&mut app)[0];
        let records = primitive_visual_records(&mut app);

        assert_eq!(records.len(), FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT);
        assert!(records.iter().any(|record| {
            record.parent == player && record.kind == FangyuanPrimitiveKind::Cube
        }));
        assert!(records.iter().any(|record| {
            record.parent == player && record.kind == FangyuanPrimitiveKind::Sphere
        }));
    }

    #[test]
    fn fangyuan_preview_visual_spawn_is_idempotent() {
        let mut app = test_app();
        enter_preview_mode(&mut app);
        app.update();

        assert_eq!(
            primitive_visual_records(&mut app).len(),
            FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT
        );
    }

    #[test]
    fn fangyuan_preview_visuals_use_cached_unit_meshes_by_kind() {
        let mut app = test_app();
        let color = Color::srgb(0.2, 0.4, 0.6);
        spawn_custom_player_for_test(
            &mut app,
            FangyuanPrimitiveSet::from_primitives(vec![
                FangyuanPrimitive::new(
                    FangyuanPrimitiveKind::Cube,
                    Vec3::new(-1.0, 0.5, 0.0),
                    Vec3::splat(1.0),
                    color,
                ),
                FangyuanPrimitive::new(
                    FangyuanPrimitiveKind::Cube,
                    Vec3::new(0.0, 0.5, 0.0),
                    Vec3::splat(0.75),
                    color,
                ),
                FangyuanPrimitive::new(
                    FangyuanPrimitiveKind::Sphere,
                    Vec3::new(1.0, 0.5, 0.0),
                    Vec3::splat(0.5),
                    color,
                ),
                FangyuanPrimitive::new(
                    FangyuanPrimitiveKind::Sphere,
                    Vec3::new(2.0, 0.5, 0.0),
                    Vec3::splat(0.25),
                    color,
                ),
            ]),
        );

        app.update();

        let mut records = primitive_visual_records(&mut app);
        records.sort_by_key(|record| record.index);
        let render_assets = app.world().resource::<FangyuanPlayerPreviewRenderAssets>();
        let cubes: Vec<_> = records
            .iter()
            .filter(|record| record.kind == FangyuanPrimitiveKind::Cube)
            .collect();
        let spheres: Vec<_> = records
            .iter()
            .filter(|record| record.kind == FangyuanPrimitiveKind::Sphere)
            .collect();

        assert_eq!(records.len(), 4);
        assert_eq!(cubes.len(), 2);
        assert_eq!(spheres.len(), 2);
        assert_eq!(
            Some(&cubes[0].mesh),
            render_assets.unit_cube_mesh(),
            "cube visual should reuse the cached unit cube mesh handle"
        );
        assert_eq!(cubes[0].mesh, cubes[1].mesh);
        assert_eq!(
            Some(&spheres[0].mesh),
            render_assets.unit_sphere_mesh(),
            "sphere visual should reuse the cached unit sphere mesh handle"
        );
        assert_eq!(spheres[0].mesh, spheres[1].mesh);
        assert_ne!(cubes[0].mesh, spheres[0].mesh);
    }

    #[test]
    fn fangyuan_preview_visuals_reuse_materials_by_color() {
        let mut app = test_app();
        let color = Color::srgb(0.2, 0.4, 0.6);
        spawn_custom_player_for_test(
            &mut app,
            FangyuanPrimitiveSet::from_primitives(vec![
                FangyuanPrimitive::new(
                    FangyuanPrimitiveKind::Cube,
                    Vec3::new(-1.0, 0.5, 0.0),
                    Vec3::splat(1.0),
                    color,
                ),
                FangyuanPrimitive::new(
                    FangyuanPrimitiveKind::Sphere,
                    Vec3::new(1.0, 0.5, 0.0),
                    Vec3::splat(0.5),
                    color,
                ),
            ]),
        );

        app.update();

        let mut records = primitive_visual_records(&mut app);
        records.sort_by_key(|record| record.index);
        let render_assets = app.world().resource::<FangyuanPlayerPreviewRenderAssets>();

        assert_eq!(records.len(), 2);
        assert_eq!(render_assets.material_count(), 1);
        assert_eq!(records[0].material, records[1].material);
    }

    #[test]
    fn fangyuan_preview_visual_material_uses_color_alpha_default() {
        let mut app = test_app();
        let color = Color::srgba(0.2, 0.4, 0.6, 0.35);
        spawn_custom_player_for_test(
            &mut app,
            FangyuanPrimitiveSet::from_primitives(vec![FangyuanPrimitive::new(
                FangyuanPrimitiveKind::Cube,
                Vec3::new(0.0, 0.5, 0.0),
                Vec3::splat(1.0),
                color,
            )]),
        );

        app.update();

        let records = primitive_visual_records(&mut app);
        let material = app
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(&records[0].material)
            .unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].alpha, color.to_srgba().alpha);
        assert_eq!(material.base_color, color);
        assert!(matches!(material.alpha_mode.clone(), AlphaMode::Blend));
    }

    #[test]
    fn fangyuan_preview_material_cache_ignores_reserved_runtime_metadata() {
        let mut app = test_app();
        let color = Color::srgb(0.2, 0.4, 0.6);
        spawn_custom_player_for_test(
            &mut app,
            FangyuanPrimitiveSet::from_primitives(vec![
                FangyuanPrimitive::with_runtime_metadata(
                    FangyuanPrimitiveKind::Cube,
                    Vec3::new(-1.0, 0.5, 0.0),
                    Vec3::splat(1.0),
                    color,
                    FangyuanPrimitiveRole::Structure,
                    0.25,
                    0.0,
                    None,
                    FangyuanPrimitiveLifecycle::empty(),
                ),
                FangyuanPrimitive::with_runtime_metadata(
                    FangyuanPrimitiveKind::Sphere,
                    Vec3::new(1.0, 0.5, 0.0),
                    Vec3::splat(0.5),
                    color,
                    FangyuanPrimitiveRole::Decoration,
                    0.75,
                    4.0,
                    Some("preview_reserved_profile".to_string()),
                    FangyuanPrimitiveLifecycle::new(Some(20), Some(2), Some(22)),
                ),
            ]),
        );

        app.update();

        let mut records = primitive_visual_records(&mut app);
        records.sort_by_key(|record| record.index);
        let render_assets = app.world().resource::<FangyuanPlayerPreviewRenderAssets>();

        assert_eq!(records.len(), 2);
        assert_eq!(render_assets.material_count(), 1);
        assert_eq!(records[0].alpha, 0.25);
        assert_eq!(records[1].alpha, 0.75);
        assert_eq!(records[0].material, records[1].material);

        let material = app
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(&records[0].material)
            .unwrap();
        assert_eq!(material.base_color, color);
        assert!(matches!(material.alpha_mode.clone(), AlphaMode::Opaque));
    }

    #[test]
    fn fangyuan_preview_visual_transform_and_material_follow_primitive_data() {
        let mut app = test_app();
        enter_preview_mode(&mut app);

        let primitive_set = {
            let mut primitive_sets = app
                .world_mut()
                .query_filtered::<&FangyuanPrimitiveSet, With<FangyuanPlayer>>();
            primitive_sets.single(app.world()).unwrap().clone()
        };
        let mut records = primitive_visual_records(&mut app);
        records.sort_by_key(|record| record.index);

        for (primitive, record) in primitive_set.primitives().iter().zip(records.iter()) {
            let material = app
                .world()
                .resource::<Assets<StandardMaterial>>()
                .get(&record.material)
                .unwrap();

            assert_eq!(record.kind, primitive.kind);
            assert_eq!(record.translation, primitive.local_position);
            assert_eq!(record.scale, primitive.scale);
            assert_eq!(record.rotation, Quat::IDENTITY);
            assert_eq!(record.alpha, primitive.alpha);
            assert_eq!(material.base_color, primitive.color);
        }
    }

    #[test]
    fn fangyuan_preview_visual_children_do_not_get_gameplay_components() {
        let mut app = test_app();
        enter_preview_mode(&mut app);

        for record in primitive_visual_records(&mut app) {
            let entity = app.world().entity(record.entity);
            assert!(!entity.contains::<FangyuanPlayer>());
            assert!(!entity.contains::<FangyuanPlayerState>());
            assert!(!entity.contains::<FangyuanPlayerPosition>());
            assert!(!entity.contains::<FangyuanObjectState>());
            assert!(!entity.contains::<FangyuanAvatar>());
            assert!(!entity.contains::<FangyuanPrimitiveSet>());
            assert!(!entity.contains::<FangyuanPlayerVisualsSpawned>());
            assert!(entity.contains::<FangyuanPlayerPrimitiveVisual>());
            assert!(entity.contains::<Mesh3d>());
            assert!(entity.contains::<MeshMaterial3d<StandardMaterial>>());
            assert!(entity.contains::<Transform>());
            assert!(entity.contains::<Visibility>());
        }
    }

    #[test]
    fn moving_player_root_preserves_visual_local_transforms_and_parenting() {
        let mut app = test_app();
        enter_preview_mode(&mut app);
        let player = fangyuan_player_entities(&mut app)[0];
        let before = primitive_visual_records(&mut app);

        app.world_mut()
            .get_mut::<FangyuanPlayerPosition>(player)
            .unwrap()
            .translation = Vec3::new(3.0, 0.0, -4.0);
        app.update();

        let after = primitive_visual_records(&mut app);
        assert_eq!(after.len(), before.len());
        for before_record in before {
            let after_record = after
                .iter()
                .find(|record| record.entity == before_record.entity)
                .unwrap();
            assert_eq!(after_record.parent, player);
            assert_eq!(after_record.translation, before_record.translation);
            assert_eq!(after_record.scale, before_record.scale);
            assert_eq!(after_record.rotation, Quat::IDENTITY);
        }
    }

    fn fangyuan_player_entities(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<FangyuanPlayer>>()
            .iter(app.world())
            .collect()
    }

    fn spawn_custom_player_for_test(app: &mut App, primitive_set: FangyuanPrimitiveSet) -> Entity {
        app.world_mut()
            .spawn((
                FangyuanPlayer,
                FangyuanPlayerState::default(),
                FangyuanPlayerPosition::default(),
                FangyuanObjectState::default(),
                Transform::default(),
                GlobalTransform::default(),
                FangyuanAvatar::new("test", "Test Fangyuan Player", primitive_set.clone()),
                primitive_set,
            ))
            .id()
    }

    #[derive(Clone, Debug)]
    struct PrimitiveVisualRecord {
        entity: Entity,
        parent: Entity,
        kind: FangyuanPrimitiveKind,
        index: usize,
        alpha: f32,
        mesh: Handle<Mesh>,
        material: Handle<StandardMaterial>,
        translation: Vec3,
        rotation: Quat,
        scale: Vec3,
    }

    fn primitive_visual_records(app: &mut App) -> Vec<PrimitiveVisualRecord> {
        let mut visuals = app.world_mut().query::<(
            Entity,
            &ChildOf,
            &FangyuanPlayerPrimitiveVisual,
            &Mesh3d,
            &MeshMaterial3d<StandardMaterial>,
            &Transform,
        )>();
        visuals
            .iter(app.world())
            .map(
                |(entity, parent, visual, mesh, material, transform)| PrimitiveVisualRecord {
                    entity,
                    parent: parent.parent(),
                    kind: visual.kind,
                    index: visual.index,
                    alpha: visual.alpha,
                    mesh: mesh.0.clone(),
                    material: material.0.clone(),
                    translation: transform.translation,
                    rotation: transform.rotation,
                    scale: transform.scale,
                },
            )
            .collect()
    }
}
