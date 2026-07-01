use bevy::{
    mesh::{MeshBuilder, SphereKind, SphereMeshBuilder},
    prelude::*,
    transform::TransformSystems,
};
use std::collections::HashMap;

use crate::framework::fangyuan::{
    FANGYUAN_MINIMAL_PLAYER_BLUEPRINT_PATH, FangyuanAvatar, FangyuanPrimitiveKind,
    FangyuanPrimitiveSet, load_fangyuan_minimal_player_primitive_set_or_log,
};
use crate::game::navigation::AppUiMode;

const FANGYUAN_PREVIEW_SPHERE_SECTORS: u32 = 24;
const FANGYUAN_PREVIEW_SPHERE_STACKS: u32 = 12;

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

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
struct FangyuanPlayerPrimitiveVisual {
    kind: FangyuanPrimitiveKind,
    index: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
struct FangyuanPlayerPreviewColorKey([u8; 4]);

impl FangyuanPlayerPreviewColorKey {
    fn from_color(color: Color) -> Self {
        let color = color.to_srgba();
        Self([
            quantize_color_channel(color.red),
            quantize_color_channel(color.green),
            quantize_color_channel(color.blue),
            quantize_color_channel(color.alpha),
        ])
    }
}

#[derive(Clone, Debug, Resource, Default)]
struct FangyuanPlayerPreviewRenderAssets {
    unit_cube_mesh: Option<Handle<Mesh>>,
    unit_sphere_mesh: Option<Handle<Mesh>>,
    materials_by_color: HashMap<FangyuanPlayerPreviewColorKey, Handle<StandardMaterial>>,
}

impl FangyuanPlayerPreviewRenderAssets {
    fn unit_mesh(
        &mut self,
        kind: FangyuanPrimitiveKind,
        meshes: &mut Assets<Mesh>,
    ) -> Handle<Mesh> {
        match kind {
            FangyuanPrimitiveKind::Cube => self
                .unit_cube_mesh
                .get_or_insert_with(|| meshes.add(Cuboid::from_size(Vec3::ONE)))
                .clone(),
            FangyuanPrimitiveKind::Sphere => self
                .unit_sphere_mesh
                .get_or_insert_with(|| {
                    meshes.add(
                        SphereMeshBuilder::new(
                            0.5,
                            SphereKind::Uv {
                                sectors: FANGYUAN_PREVIEW_SPHERE_SECTORS,
                                stacks: FANGYUAN_PREVIEW_SPHERE_STACKS,
                            },
                        )
                        .build(),
                    )
                })
                .clone(),
        }
    }

    fn material(
        &mut self,
        color: Color,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        self.materials_by_color
            .entry(FangyuanPlayerPreviewColorKey::from_color(color))
            .or_insert_with(|| materials.add(standard_material_from_color(color)))
            .clone()
    }

    #[cfg(test)]
    fn material_count(&self) -> usize {
        self.materials_by_color.len()
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
    commands.spawn((
        DespawnOnExit(AppUiMode::FangyuanPlayerPreview),
        FangyuanPlayer,
        FangyuanPlayerState::default(),
        position,
        Transform::from_translation(position.translation),
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
            let transform =
                Transform::from_translation(primitive.local_position).with_scale(primitive.scale);
            let visual = commands
                .spawn((
                    FangyuanPlayerPrimitiveVisual {
                        kind: primitive.kind,
                        index,
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
    mut players: Query<(&FangyuanPlayerPosition, &mut Transform), With<FangyuanPlayer>>,
) {
    for (position, mut transform) in &mut players {
        transform.translation = position.translation;
        transform.rotation = Quat::IDENTITY;
    }
}

fn quantize_color_channel(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn standard_material_from_color(color: Color) -> StandardMaterial {
    let alpha = color.to_srgba().alpha;
    StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.92,
        alpha_mode: if alpha < 1.0 {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        },
        ..default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::{FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT, FangyuanPrimitive};

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
            &FangyuanAvatar,
            &FangyuanPrimitiveSet,
            &Transform,
        )>();
        let (_, state, position, avatar, primitive_set, transform) =
            players.single(app.world()).unwrap();

        assert!(state.active);
        assert_eq!(position.translation, Vec3::ZERO);
        assert_eq!(transform.translation, position.translation);
        assert_eq!(transform.rotation, Quat::IDENTITY);
        assert_eq!(avatar.blueprint_id, FANGYUAN_MINIMAL_PLAYER_BLUEPRINT_PATH);
        assert_eq!(
            avatar.primitives.len(),
            FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT
        );
        assert_eq!(primitive_set.len(), FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT);
        assert_eq!(&avatar.primitives, primitive_set);
    }

    #[test]
    fn fangyuan_player_position_only_exposes_translation() {
        let position = FangyuanPlayerPosition {
            translation: Vec3::new(2.0, 0.0, -3.0),
        };

        assert_eq!(position.translation, Vec3::new(2.0, 0.0, -3.0));
    }

    #[test]
    fn moving_player_position_updates_root_transform_without_rotation() {
        let mut app = test_app();
        enter_preview_mode(&mut app);
        let player = fangyuan_player_entities(&mut app)[0];

        app.world_mut()
            .get_mut::<FangyuanPlayerPosition>(player)
            .unwrap()
            .translation = Vec3::new(4.0, 0.0, -2.0);
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .rotation = Quat::from_rotation_y(1.0);

        app.update();

        let transform = app.world().get::<Transform>(player).unwrap();
        assert_eq!(transform.translation, Vec3::new(4.0, 0.0, -2.0));
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
        enter_preview_mode(&mut app);

        let records = primitive_visual_records(&mut app);
        let render_assets = app.world().resource::<FangyuanPlayerPreviewRenderAssets>();
        let cube = records
            .iter()
            .find(|record| record.kind == FangyuanPrimitiveKind::Cube)
            .unwrap();
        let sphere = records
            .iter()
            .find(|record| record.kind == FangyuanPrimitiveKind::Sphere)
            .unwrap();

        assert_eq!(
            Some(&cube.mesh),
            render_assets.unit_cube_mesh.as_ref(),
            "cube visual should reuse the cached unit cube mesh handle"
        );
        assert_eq!(
            Some(&sphere.mesh),
            render_assets.unit_sphere_mesh.as_ref(),
            "sphere visual should reuse the cached unit sphere mesh handle"
        );
        assert_ne!(cube.mesh, sphere.mesh);
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

        let records = primitive_visual_records(&mut app);
        let render_assets = app.world().resource::<FangyuanPlayerPreviewRenderAssets>();

        assert_eq!(records.len(), 2);
        assert_eq!(render_assets.material_count(), 1);
        assert_eq!(records[0].material, records[1].material);
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
            assert!(!entity.contains::<FangyuanAvatar>());
            assert!(!entity.contains::<FangyuanPrimitiveSet>());
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
