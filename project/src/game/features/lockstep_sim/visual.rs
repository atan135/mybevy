use std::collections::BTreeMap;

use bevy::{gltf::GltfAssetLabel, prelude::*, scene::SceneRoot as BevySceneRoot};
use sim_core::{EntityId, EntityKind, MovementMode, QuantizedDir, SimEntity, SimWorld};

use crate::{
    framework::scene::prelude::{
        SCENE_CAMERA_LOCAL_PLAYER_TARGET_TAG, SceneCameraTarget, SceneOwned, SceneRuntimeRoot,
        SceneSessionId,
    },
    game::authority::AuthoritySession,
};

use super::{
    config::LockstepSimConfig, replay::LockstepSimReplayState, state::LockstepSimSceneState,
};

pub(in crate::game::features::lockstep_sim) const LOCKSTEP_SIM_ENTITY_FOOT_WORLD_Y: f32 = 0.0;
const LOCKSTEP_SIM_MODEL_SCALE: f32 = 5.0;
const LOCKSTEP_SIM_DEAD_MODEL_HEIGHT_SCALE: f32 = 0.6;
const LOCKSTEP_SIM_LOCAL_MODEL_ASSET_PATH: &str = "models/characters/kaykit_adventurers/Knight.glb";
const LOCKSTEP_SIM_REMOTE_MODEL_ASSET_PATHS: [&str; 2] = [
    "models/characters/kaykit_adventurers/Rogue.glb",
    "models/characters/kaykit_adventurers/Mage.glb",
];
const LOCKSTEP_SIM_TRAINING_MODEL_ASSET_PATH: &str =
    "models/characters/kaykit_adventurers/Barbarian.glb";
const LOCKSTEP_SIM_ENEMY_MODEL_ASSET_PATH: &str = "models/characters/kaykit_adventurers/Ranger.glb";

#[derive(Clone, Debug, Default, Resource, PartialEq)]
pub(in crate::game::features::lockstep_sim) struct LockstepSimVisualState {
    pub(in crate::game::features::lockstep_sim) entity_visuals: BTreeMap<EntityId, Entity>,
    pub(in crate::game::features::lockstep_sim) tracked_entity_count: usize,
    pub(in crate::game::features::lockstep_sim) sim_entity_count: usize,
    pub(in crate::game::features::lockstep_sim) last_synced_frame: Option<u32>,
    pub(in crate::game::features::lockstep_sim) debug_entries: Vec<LockstepSimVisualDebugEntry>,
}

impl LockstepSimVisualState {
    pub(in crate::game::features::lockstep_sim) fn clear(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::game::features::lockstep_sim) struct LockstepSimVisualDebugEntry {
    pub(in crate::game::features::lockstep_sim) frame: u32,
    pub(in crate::game::features::lockstep_sim) entity_id: EntityId,
    pub(in crate::game::features::lockstep_sim) role: LockstepSimVisualRole,
    pub(in crate::game::features::lockstep_sim) raw_x: i64,
    pub(in crate::game::features::lockstep_sim) raw_y: i64,
    pub(in crate::game::features::lockstep_sim) render_x: f32,
    pub(in crate::game::features::lockstep_sim) render_z: f32,
    pub(in crate::game::features::lockstep_sim) movement_mode: MovementMode,
    pub(in crate::game::features::lockstep_sim) moving: bool,
    pub(in crate::game::features::lockstep_sim) facing_x: i16,
    pub(in crate::game::features::lockstep_sim) facing_y: i16,
}

#[derive(Clone, Debug, Component, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) struct LockstepSimEntityVisual {
    pub(in crate::game::features::lockstep_sim) entity_id: EntityId,
    pub(in crate::game::features::lockstep_sim) session_id: SceneSessionId,
    pub(in crate::game::features::lockstep_sim) owner_character_id: Option<String>,
    pub(in crate::game::features::lockstep_sim) kind: EntityKind,
    pub(in crate::game::features::lockstep_sim) role: LockstepSimVisualRole,
    pub(in crate::game::features::lockstep_sim) color_index: usize,
    pub(in crate::game::features::lockstep_sim) movement_mode: MovementMode,
    pub(in crate::game::features::lockstep_sim) moving: bool,
    pub(in crate::game::features::lockstep_sim) raw_x: i64,
    pub(in crate::game::features::lockstep_sim) raw_y: i64,
    pub(in crate::game::features::lockstep_sim) facing_dir: QuantizedDir,
    pub(in crate::game::features::lockstep_sim) move_dir: QuantizedDir,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) enum LockstepSimVisualRole {
    LocalPlayer,
    RemotePlayer,
    TrainingTarget,
    EnemyEntity,
}

pub(in crate::game::features::lockstep_sim) fn clear_lockstep_sim_visuals(
    state: &mut LockstepSimVisualState,
) {
    state.clear();
}

pub(in crate::game::features::lockstep_sim) fn despawn_lockstep_sim_visual_entities(
    commands: &mut Commands,
    state: &mut LockstepSimVisualState,
    entities: impl IntoIterator<Item = Entity>,
) {
    for entity in entities {
        commands.entity(entity).despawn();
    }
    state.clear();
}

pub(in crate::game::features::lockstep_sim) fn sync_lockstep_sim_entity_visuals(
    mut commands: Commands,
    asset_server: Option<Res<AssetServer>>,
    config: Res<LockstepSimConfig>,
    scene_state: Res<LockstepSimSceneState>,
    replay_state: Res<LockstepSimReplayState>,
    authority_session: Res<AuthoritySession>,
    mut visual_state: ResMut<LockstepSimVisualState>,
    runtime_roots: Query<(Entity, &SceneRuntimeRoot)>,
    mut entity_visuals: Query<(
        Entity,
        &mut LockstepSimEntityVisual,
        &mut Transform,
        Option<&mut BevySceneRoot>,
    )>,
) {
    if !scene_state.active {
        let entities = entity_visuals
            .iter_mut()
            .map(|(entity, _, _, _)| entity)
            .collect::<Vec<_>>();
        despawn_lockstep_sim_visual_entities(&mut commands, &mut visual_state, entities);
        return;
    }

    let Some(session_id) = scene_state.session_id.as_ref() else {
        return;
    };
    let Some(world) = replay_state.world.as_ref() else {
        let entities = entity_visuals
            .iter_mut()
            .map(|(entity, _, _, _)| entity)
            .collect::<Vec<_>>();
        despawn_lockstep_sim_visual_entities(&mut commands, &mut visual_state, entities);
        return;
    };

    let local_player_id = authority_session
        .local_player_id
        .as_deref()
        .unwrap_or(config.local_player_id.as_str());
    visual_state.sim_entity_count = world.entities_sorted_by_id().len();
    visual_state.last_synced_frame = Some(world.frame.raw());
    visual_state.debug_entries = build_visual_debug_entries(world, local_player_id);

    let mut live_visuals = BTreeMap::new();
    for (bevy_entity, mut visual, mut transform, scene_root) in &mut entity_visuals {
        let should_remove =
            &visual.session_id != session_id || world.entity(visual.entity_id).is_none();
        if should_remove {
            commands.entity(bevy_entity).despawn();
            continue;
        }

        let Some(sim_entity) = world.entity(visual.entity_id) else {
            continue;
        };
        if live_visuals.contains_key(&visual.entity_id) {
            commands.entity(bevy_entity).despawn();
            continue;
        }

        let Some(mut scene_root) = scene_root else {
            commands.entity(bevy_entity).despawn();
            continue;
        };

        let snapshot = visual_snapshot(sim_entity, local_player_id);
        let should_update_model =
            visual.role != snapshot.role || visual.color_index != snapshot.color_index;
        apply_visual_snapshot(&mut visual, snapshot);
        update_lockstep_sim_camera_target(
            &mut commands,
            bevy_entity,
            session_id,
            visual.role == LockstepSimVisualRole::LocalPlayer,
        );
        if should_update_model && let Some(asset_server) = asset_server.as_deref() {
            scene_root.0 =
                lockstep_sim_model_scene_handle(asset_server, visual.role, visual.color_index);
        }
        apply_lockstep_sim_visual_transform(&mut transform, sim_entity);
        live_visuals.insert(visual.entity_id, bevy_entity);
    }
    visual_state.entity_visuals = live_visuals;

    let Some(runtime_root) = find_runtime_root_entity(session_id, runtime_roots.iter()) else {
        if !world.entities_sorted_by_id().is_empty() {
            warn!(
                "skipping lockstep sim entity visuals because session `{}` has no runtime root",
                session_id
            );
        }
        visual_state.tracked_entity_count = visual_state.entity_visuals.len();
        return;
    };

    let Some(asset_server) = asset_server.as_deref() else {
        if !world.entities_sorted_by_id().is_empty() {
            warn!("skipping lockstep sim entity visuals because AssetServer is unavailable");
        }
        visual_state.tracked_entity_count = visual_state.entity_visuals.len();
        return;
    };

    for sim_entity in world.entities_sorted_by_id() {
        if visual_state.entity_visuals.contains_key(&sim_entity.id) {
            continue;
        }
        let snapshot = visual_snapshot(sim_entity, local_player_id);
        let bevy_entity = spawn_lockstep_sim_entity_visual(
            &mut commands,
            asset_server,
            runtime_root,
            session_id,
            sim_entity,
            snapshot,
        );
        visual_state
            .entity_visuals
            .insert(sim_entity.id, bevy_entity);
    }

    visual_state.tracked_entity_count = visual_state.entity_visuals.len();
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LockstepSimVisualSnapshot {
    owner_character_id: Option<String>,
    kind: EntityKind,
    role: LockstepSimVisualRole,
    color_index: usize,
    movement_mode: MovementMode,
    moving: bool,
    raw_x: i64,
    raw_y: i64,
    facing_dir: QuantizedDir,
    move_dir: QuantizedDir,
}

fn spawn_lockstep_sim_entity_visual(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    session_id: &SceneSessionId,
    sim_entity: &SimEntity,
    snapshot: LockstepSimVisualSnapshot,
) -> Entity {
    let scene_handle =
        lockstep_sim_model_scene_handle(asset_server, snapshot.role, snapshot.color_index);
    let is_local_player = snapshot.role == LockstepSimVisualRole::LocalPlayer;
    let entity = commands
        .spawn((
            BevySceneRoot(scene_handle),
            lockstep_sim_entity_transform(sim_entity),
            SceneOwned::new(session_id.clone()),
            LockstepSimEntityVisual {
                entity_id: sim_entity.id,
                session_id: session_id.clone(),
                owner_character_id: snapshot.owner_character_id,
                kind: snapshot.kind,
                role: snapshot.role,
                color_index: snapshot.color_index,
                movement_mode: snapshot.movement_mode,
                moving: snapshot.moving,
                raw_x: snapshot.raw_x,
                raw_y: snapshot.raw_y,
                facing_dir: snapshot.facing_dir,
                move_dir: snapshot.move_dir,
            },
            Name::new(format!("LockstepSimEntity({})", sim_entity.id.raw())),
        ))
        .id();
    update_lockstep_sim_camera_target(commands, entity, session_id, is_local_player);
    commands.entity(parent).add_child(entity);
    entity
}

fn apply_visual_snapshot(
    visual: &mut LockstepSimEntityVisual,
    snapshot: LockstepSimVisualSnapshot,
) {
    visual.owner_character_id = snapshot.owner_character_id;
    visual.kind = snapshot.kind;
    visual.role = snapshot.role;
    visual.color_index = snapshot.color_index;
    visual.movement_mode = snapshot.movement_mode;
    visual.moving = snapshot.moving;
    visual.raw_x = snapshot.raw_x;
    visual.raw_y = snapshot.raw_y;
    visual.facing_dir = snapshot.facing_dir;
    visual.move_dir = snapshot.move_dir;
}

fn visual_snapshot(sim_entity: &SimEntity, local_player_id: &str) -> LockstepSimVisualSnapshot {
    LockstepSimVisualSnapshot {
        owner_character_id: sim_entity.owner_character_id.clone(),
        kind: sim_entity.kind,
        role: visual_role_for_entity(sim_entity, local_player_id),
        color_index: sim_entity.id.raw() as usize,
        movement_mode: sim_entity.movement.mode,
        moving: is_lockstep_sim_entity_moving(sim_entity),
        raw_x: sim_entity.transform.pos.x.raw(),
        raw_y: sim_entity.transform.pos.y.raw(),
        facing_dir: visual_direction(sim_entity),
        move_dir: sim_entity.movement.move_dir,
    }
}

fn visual_role_for_entity(sim_entity: &SimEntity, local_player_id: &str) -> LockstepSimVisualRole {
    if sim_entity.owner_character_id.as_deref() == Some(local_player_id) {
        return LockstepSimVisualRole::LocalPlayer;
    }

    match sim_entity.kind {
        EntityKind::Player => LockstepSimVisualRole::RemotePlayer,
        EntityKind::Npc | EntityKind::Monster => LockstepSimVisualRole::TrainingTarget,
        EntityKind::Projectile | EntityKind::Summon => LockstepSimVisualRole::EnemyEntity,
    }
}

fn is_lockstep_sim_entity_moving(sim_entity: &SimEntity) -> bool {
    sim_entity.movement.mode == MovementMode::Controlled
        && sim_entity.movement.move_dir != QuantizedDir::ZERO
        && sim_entity.movement.speed_per_second.raw() > 0
}

fn apply_lockstep_sim_visual_transform(transform: &mut Transform, sim_entity: &SimEntity) {
    transform.translation = lockstep_sim_entity_translation(sim_entity);
    transform.scale = lockstep_sim_entity_scale(sim_entity);
    if let Some(rotation) = lockstep_sim_entity_rotation(sim_entity) {
        transform.rotation = rotation;
    }
}

fn lockstep_sim_entity_transform(sim_entity: &SimEntity) -> Transform {
    let mut transform = Transform::from_translation(lockstep_sim_entity_translation(sim_entity))
        .with_scale(lockstep_sim_entity_scale(sim_entity));
    if let Some(rotation) = lockstep_sim_entity_rotation(sim_entity) {
        transform.rotation = rotation;
    }
    transform
}

fn lockstep_sim_entity_scale(sim_entity: &SimEntity) -> Vec3 {
    if sim_entity.alive {
        Vec3::splat(LOCKSTEP_SIM_MODEL_SCALE)
    } else {
        Vec3::new(
            LOCKSTEP_SIM_MODEL_SCALE,
            LOCKSTEP_SIM_DEAD_MODEL_HEIGHT_SCALE,
            LOCKSTEP_SIM_MODEL_SCALE,
        )
    }
}

fn lockstep_sim_entity_translation(sim_entity: &SimEntity) -> Vec3 {
    Vec3::new(
        sim_entity.transform.pos.x.to_f32_for_render(),
        LOCKSTEP_SIM_ENTITY_FOOT_WORLD_Y,
        sim_entity.transform.pos.y.to_f32_for_render(),
    )
}

fn lockstep_sim_entity_rotation(sim_entity: &SimEntity) -> Option<Quat> {
    lockstep_sim_yaw_from_direction(visual_direction(sim_entity)).map(Quat::from_rotation_y)
}

fn visual_direction(sim_entity: &SimEntity) -> QuantizedDir {
    if sim_entity.transform.facing != QuantizedDir::ZERO {
        sim_entity.transform.facing
    } else {
        sim_entity.movement.move_dir
    }
}

fn lockstep_sim_yaw_from_direction(dir: QuantizedDir) -> Option<f32> {
    if dir == QuantizedDir::ZERO {
        None
    } else {
        Some((dir.x() as f32).atan2(dir.y() as f32))
    }
}

fn update_lockstep_sim_camera_target(
    commands: &mut Commands,
    entity: Entity,
    session_id: &SceneSessionId,
    is_local_player: bool,
) {
    if is_local_player {
        commands.entity(entity).insert(
            SceneCameraTarget::new(session_id.clone())
                .with_tag(SCENE_CAMERA_LOCAL_PLAYER_TARGET_TAG)
                .with_priority(100),
        );
    } else {
        commands.entity(entity).remove::<SceneCameraTarget>();
    }
}

fn lockstep_sim_model_scene_handle(
    asset_server: &AssetServer,
    role: LockstepSimVisualRole,
    color_index: usize,
) -> Handle<bevy::scene::Scene> {
    asset_server
        .load(GltfAssetLabel::Scene(0).from_asset(lockstep_sim_model_asset_path(role, color_index)))
}

fn lockstep_sim_model_asset_path(role: LockstepSimVisualRole, color_index: usize) -> &'static str {
    match role {
        LockstepSimVisualRole::LocalPlayer => LOCKSTEP_SIM_LOCAL_MODEL_ASSET_PATH,
        LockstepSimVisualRole::RemotePlayer => {
            LOCKSTEP_SIM_REMOTE_MODEL_ASSET_PATHS
                [color_index % LOCKSTEP_SIM_REMOTE_MODEL_ASSET_PATHS.len()]
        }
        LockstepSimVisualRole::TrainingTarget => LOCKSTEP_SIM_TRAINING_MODEL_ASSET_PATH,
        LockstepSimVisualRole::EnemyEntity => LOCKSTEP_SIM_ENEMY_MODEL_ASSET_PATH,
    }
}

fn build_visual_debug_entries(
    world: &SimWorld,
    local_player_id: &str,
) -> Vec<LockstepSimVisualDebugEntry> {
    world
        .entities_sorted_by_id()
        .iter()
        .map(|entity| {
            let direction = visual_direction(entity);
            LockstepSimVisualDebugEntry {
                frame: world.frame.raw(),
                entity_id: entity.id,
                role: visual_role_for_entity(entity, local_player_id),
                raw_x: entity.transform.pos.x.raw(),
                raw_y: entity.transform.pos.y.raw(),
                render_x: entity.transform.pos.x.to_f32_for_render(),
                render_z: entity.transform.pos.y.to_f32_for_render(),
                movement_mode: entity.movement.mode,
                moving: is_lockstep_sim_entity_moving(entity),
                facing_x: direction.x(),
                facing_y: direction.y(),
            }
        })
        .collect()
}

fn find_runtime_root_entity<'runtime>(
    session_id: &SceneSessionId,
    runtime_roots: impl IntoIterator<Item = (Entity, &'runtime SceneRuntimeRoot)>,
) -> Option<Entity> {
    runtime_roots
        .into_iter()
        .find(|(_, root)| root.is_session(session_id))
        .map(|(entity, _)| entity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        framework::scene::prelude::{SceneId, spawn_scene_root, spawn_scene_runtime_root},
        game::scenes::LOCKSTEP_SIM_ARENA_SCENE_ID,
    };
    use sim_core::{
        CombatState, Fp, FrameId, MovementState, SimRngState, SimTransform, SimWorld, TeamId,
        Vec2Fp, hash_world,
    };

    const LOCAL_PLAYER_ID: &str = "player-local";
    const REMOTE_PLAYER_ID: &str = "player-remote";

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin))
            .init_asset::<bevy::scene::Scene>()
            .init_resource::<LockstepSimConfig>()
            .init_resource::<LockstepSimSceneState>()
            .init_resource::<LockstepSimReplayState>()
            .init_resource::<LockstepSimVisualState>()
            .init_resource::<AuthoritySession>()
            .add_systems(Update, sync_lockstep_sim_entity_visuals);
        app
    }

    fn activate_scene_with_runtime_root(app: &mut App) -> (SceneSessionId, Entity) {
        let session_id = SceneSessionId::from("lockstep-sim-session");
        app.world_mut()
            .resource_mut::<LockstepSimSceneState>()
            .activate(
                SceneId::from(LOCKSTEP_SIM_ARENA_SCENE_ID),
                session_id.clone(),
            );
        let scene_root = spawn_scene_root(
            &mut app.world_mut().commands(),
            &SceneId::from(LOCKSTEP_SIM_ARENA_SCENE_ID),
            &session_id,
        );
        let runtime_root =
            spawn_scene_runtime_root(&mut app.world_mut().commands(), scene_root, &session_id);
        app.update();
        (session_id, runtime_root)
    }

    fn set_replay_world(app: &mut App, world: SimWorld) {
        app.world_mut()
            .resource_mut::<LockstepSimReplayState>()
            .world = Some(world);
    }

    fn sim_entity(
        id: u32,
        kind: EntityKind,
        owner: Option<&str>,
        pos: Vec2Fp,
        facing: QuantizedDir,
    ) -> SimEntity {
        SimEntity {
            id: EntityId::new(id),
            kind,
            owner_character_id: owner.map(str::to_string),
            team_id: TeamId::new(if owner == Some(LOCAL_PLAYER_ID) { 1 } else { 2 }),
            transform: SimTransform {
                pos,
                facing,
                radius: Fp::from_milli(500),
            },
            movement: MovementState::default(),
            combat: CombatState::default(),
            alive: true,
        }
    }

    fn moving_entity(
        id: u32,
        owner: Option<&str>,
        pos: Vec2Fp,
        facing: QuantizedDir,
        move_dir: QuantizedDir,
    ) -> SimEntity {
        let mut entity = sim_entity(id, EntityKind::Player, owner, pos, facing);
        entity.movement = MovementState {
            mode: MovementMode::Controlled,
            move_dir,
            speed_per_second: Fp::from_i32(6),
        };
        entity
    }

    fn world_fixture(entities: Vec<SimEntity>) -> SimWorld {
        SimWorld::with_rng(
            FrameId::new(7),
            SimRngState {
                seed: 10,
                counter: 20,
            },
            entities,
        )
        .unwrap()
    }

    fn visual_entries(
        app: &mut App,
    ) -> Vec<(
        Entity,
        EntityId,
        LockstepSimVisualRole,
        EntityKind,
        Option<String>,
        Vec3,
        Vec3,
        String,
        String,
    )> {
        let mut query = app.world_mut().query::<(
            Entity,
            &LockstepSimEntityVisual,
            &Transform,
            &BevySceneRoot,
            &Name,
        )>();
        let asset_server = app.world().resource::<AssetServer>().clone();
        query
            .iter(app.world())
            .map(|(entity, visual, transform, scene_root, name)| {
                (
                    entity,
                    visual.entity_id,
                    visual.role,
                    visual.kind,
                    visual.owner_character_id.clone(),
                    transform.translation,
                    transform.scale,
                    scene_asset_path(&asset_server, scene_root),
                    name.as_str().to_string(),
                )
            })
            .collect()
    }

    fn scene_asset_path(asset_server: &AssetServer, scene_root: &BevySceneRoot) -> String {
        asset_server
            .get_path(scene_root.0.id().untyped())
            .expect("lockstep sim scene handle should have an asset path")
            .to_string()
    }

    fn visual_entity_for(app: &App, entity_id: EntityId) -> Entity {
        *app.world()
            .resource::<LockstepSimVisualState>()
            .entity_visuals
            .get(&entity_id)
            .expect("lockstep visual should be tracked")
    }

    fn visual_for(app: &App, entity_id: EntityId) -> LockstepSimEntityVisual {
        app.world()
            .get::<LockstepSimEntityVisual>(visual_entity_for(app, entity_id))
            .expect("lockstep visual component should exist")
            .clone()
    }

    fn transform_for(app: &App, entity_id: EntityId) -> Transform {
        *app.world()
            .get::<Transform>(visual_entity_for(app, entity_id))
            .expect("lockstep visual should have transform")
    }

    fn assert_same_rotation(actual: Quat, expected: Quat) {
        let same_rotation = actual.dot(expected).abs();
        assert!(
            (1.0 - same_rotation) < 0.000_001,
            "expected {actual:?} to represent the same rotation as {expected:?}"
        );
    }

    #[test]
    fn lockstep_sim_visuals_spawn_local_remote_and_training_entities() {
        let mut app = test_app();
        let (_, runtime_root) = activate_scene_with_runtime_root(&mut app);
        app.world_mut()
            .resource_mut::<AuthoritySession>()
            .local_player_id = Some(LOCAL_PLAYER_ID.to_string());
        set_replay_world(
            &mut app,
            world_fixture(vec![
                sim_entity(
                    1000,
                    EntityKind::Player,
                    Some(LOCAL_PLAYER_ID),
                    Vec2Fp::zero(),
                    QuantizedDir::RIGHT,
                ),
                sim_entity(
                    1001,
                    EntityKind::Player,
                    Some(REMOTE_PLAYER_ID),
                    Vec2Fp::new(Fp::from_i32(3), Fp::ZERO),
                    QuantizedDir::LEFT,
                ),
                sim_entity(
                    9000,
                    EntityKind::Monster,
                    None,
                    Vec2Fp::new(Fp::from_i32(8), Fp::ZERO),
                    QuantizedDir::LEFT,
                ),
            ]),
        );

        app.update();

        let visuals = visual_entries(&mut app);
        assert_eq!(visuals.len(), 3);
        let local = visuals
            .iter()
            .find(|(_, entity_id, _, _, _, _, _, _, _)| *entity_id == EntityId::new(1000))
            .unwrap();
        let remote = visuals
            .iter()
            .find(|(_, entity_id, _, _, _, _, _, _, _)| *entity_id == EntityId::new(1001))
            .unwrap();
        let training = visuals
            .iter()
            .find(|(_, entity_id, _, _, _, _, _, _, _)| *entity_id == EntityId::new(9000))
            .unwrap();

        assert_eq!(local.2, LockstepSimVisualRole::LocalPlayer);
        assert_eq!(remote.2, LockstepSimVisualRole::RemotePlayer);
        assert_eq!(training.2, LockstepSimVisualRole::TrainingTarget);
        assert_eq!(
            local.7,
            "models/characters/kaykit_adventurers/Knight.glb#Scene0"
        );
        assert_eq!(
            remote.7,
            "models/characters/kaykit_adventurers/Mage.glb#Scene0"
        );
        assert_eq!(
            training.7,
            "models/characters/kaykit_adventurers/Barbarian.glb#Scene0"
        );
        assert_eq!(local.8, "LockstepSimEntity(1000)");
        assert!(app.world().get::<SceneCameraTarget>(local.0).is_some());
        assert!(app.world().get::<SceneCameraTarget>(remote.0).is_none());
        assert!(app.world().get::<SceneCameraTarget>(training.0).is_none());
        for (entity, _, _, _, _, _, _, _, _) in visuals {
            let parent = app.world().get::<ChildOf>(entity).unwrap();
            assert_eq!(parent.parent(), runtime_root);
        }

        let visual_state = app.world().resource::<LockstepSimVisualState>();
        assert_eq!(visual_state.tracked_entity_count, 3);
        assert_eq!(visual_state.sim_entity_count, 3);
        assert_eq!(visual_state.last_synced_frame, Some(7));
    }

    #[test]
    fn lockstep_sim_visuals_convert_fixed_position_to_render_transform() {
        let mut app = test_app();
        activate_scene_with_runtime_root(&mut app);
        let pos = Vec2Fp::new(Fp::from_milli(1_234), Fp::from_milli(-45_000));
        set_replay_world(
            &mut app,
            world_fixture(vec![sim_entity(
                1000,
                EntityKind::Player,
                Some(LOCAL_PLAYER_ID),
                pos,
                QuantizedDir::RIGHT,
            )]),
        );

        app.update();

        let transform = transform_for(&app, EntityId::new(1000));
        assert_eq!(
            transform.translation,
            Vec3::new(
                pos.x.to_f32_for_render(),
                LOCKSTEP_SIM_ENTITY_FOOT_WORLD_Y,
                pos.y.to_f32_for_render(),
            )
        );
        assert_eq!(transform.scale, Vec3::splat(LOCKSTEP_SIM_MODEL_SCALE));

        let debug_entry = &app
            .world()
            .resource::<LockstepSimVisualState>()
            .debug_entries[0];
        assert_eq!(debug_entry.raw_x, pos.x.raw());
        assert_eq!(debug_entry.raw_y, pos.y.raw());
        assert_eq!(debug_entry.render_x, pos.x.to_f32_for_render());
        assert_eq!(debug_entry.render_z, pos.y.to_f32_for_render());
    }

    #[test]
    fn lockstep_sim_visuals_compress_dead_entities_on_the_next_sync() {
        let mut app = test_app();
        activate_scene_with_runtime_root(&mut app);
        set_replay_world(
            &mut app,
            world_fixture(vec![sim_entity(
                9000,
                EntityKind::Monster,
                None,
                Vec2Fp::new(Fp::from_i32(8), Fp::ZERO),
                QuantizedDir::LEFT,
            )]),
        );
        app.update();
        assert_eq!(
            transform_for(&app, EntityId::new(9000)).scale,
            Vec3::splat(LOCKSTEP_SIM_MODEL_SCALE)
        );

        app.world_mut()
            .resource_mut::<LockstepSimReplayState>()
            .world
            .as_mut()
            .unwrap()
            .entities[0]
            .alive = false;
        app.update();

        assert_eq!(
            transform_for(&app, EntityId::new(9000)).scale,
            Vec3::new(
                LOCKSTEP_SIM_MODEL_SCALE,
                LOCKSTEP_SIM_DEAD_MODEL_HEIGHT_SCALE,
                LOCKSTEP_SIM_MODEL_SCALE,
            )
        );
    }

    #[test]
    fn lockstep_sim_visuals_record_facing_and_movement_state() {
        let mut app = test_app();
        activate_scene_with_runtime_root(&mut app);
        set_replay_world(
            &mut app,
            world_fixture(vec![moving_entity(
                1000,
                Some(LOCAL_PLAYER_ID),
                Vec2Fp::zero(),
                QuantizedDir::ZERO,
                QuantizedDir::UP,
            )]),
        );

        app.update();

        let visual = visual_for(&app, EntityId::new(1000));
        assert_eq!(visual.movement_mode, MovementMode::Controlled);
        assert!(visual.moving);
        assert_eq!(visual.facing_dir, QuantizedDir::UP);
        assert_eq!(visual.move_dir, QuantizedDir::UP);
        assert_same_rotation(
            transform_for(&app, EntityId::new(1000)).rotation,
            Quat::from_rotation_y(std::f32::consts::PI),
        );

        let debug_entry = &app
            .world()
            .resource::<LockstepSimVisualState>()
            .debug_entries[0];
        assert_eq!(debug_entry.movement_mode, MovementMode::Controlled);
        assert!(debug_entry.moving);
        assert_eq!(debug_entry.facing_x, QuantizedDir::UP.x());
        assert_eq!(debug_entry.facing_y, QuantizedDir::UP.y());
    }

    #[test]
    fn lockstep_sim_visual_sync_does_not_mutate_replay_world() {
        let mut app = test_app();
        activate_scene_with_runtime_root(&mut app);
        let world = world_fixture(vec![moving_entity(
            1000,
            Some(LOCAL_PLAYER_ID),
            Vec2Fp::new(Fp::from_milli(12_300), Fp::from_milli(-4_500)),
            QuantizedDir::RIGHT,
            QuantizedDir::RIGHT,
        )]);
        let hash_before = hash_world(&world);
        let raw_before = world
            .entity(EntityId::new(1000))
            .unwrap()
            .transform
            .pos
            .raw_tuple();
        set_replay_world(&mut app, world);

        app.update();

        let replay = app.world().resource::<LockstepSimReplayState>();
        let world_after = replay.world.as_ref().unwrap();
        assert_eq!(hash_world(world_after), hash_before);
        assert_eq!(
            world_after
                .entity(EntityId::new(1000))
                .unwrap()
                .transform
                .pos
                .raw_tuple(),
            raw_before
        );
    }

    #[test]
    fn same_frame_visual_position_comes_from_sim_world_not_render_tick() {
        let mut app = test_app();
        activate_scene_with_runtime_root(&mut app);
        let pos = Vec2Fp::new(Fp::from_milli(7_500), Fp::from_milli(2_250));
        set_replay_world(
            &mut app,
            world_fixture(vec![sim_entity(
                1000,
                EntityKind::Player,
                Some(LOCAL_PLAYER_ID),
                pos,
                QuantizedDir::RIGHT,
            )]),
        );
        app.update();
        let expected = Vec3::new(
            pos.x.to_f32_for_render(),
            LOCKSTEP_SIM_ENTITY_FOOT_WORLD_Y,
            pos.y.to_f32_for_render(),
        );
        assert_eq!(
            transform_for(&app, EntityId::new(1000)).translation,
            expected
        );

        let visual_entity = visual_entity_for(&app, EntityId::new(1000));
        app.world_mut()
            .get_mut::<Transform>(visual_entity)
            .unwrap()
            .translation = Vec3::new(999.0, 999.0, 999.0);
        app.update();
        app.update();

        assert_eq!(
            transform_for(&app, EntityId::new(1000)).translation,
            expected
        );
        let replay = app.world().resource::<LockstepSimReplayState>();
        assert_eq!(replay.world.as_ref().unwrap().frame.raw(), 7);
        assert_eq!(
            replay
                .world
                .as_ref()
                .unwrap()
                .entity(EntityId::new(1000))
                .unwrap()
                .transform
                .pos
                .raw_tuple(),
            pos.raw_tuple()
        );
    }

    #[test]
    fn lockstep_sim_visuals_remove_missing_entities_and_clear_when_inactive() {
        let mut app = test_app();
        activate_scene_with_runtime_root(&mut app);
        set_replay_world(
            &mut app,
            world_fixture(vec![
                sim_entity(
                    1000,
                    EntityKind::Player,
                    Some(LOCAL_PLAYER_ID),
                    Vec2Fp::zero(),
                    QuantizedDir::RIGHT,
                ),
                sim_entity(
                    1001,
                    EntityKind::Player,
                    Some(REMOTE_PLAYER_ID),
                    Vec2Fp::new(Fp::from_i32(3), Fp::ZERO),
                    QuantizedDir::LEFT,
                ),
            ]),
        );
        app.update();

        app.world_mut()
            .resource_mut::<LockstepSimReplayState>()
            .world
            .as_mut()
            .unwrap()
            .entities
            .retain(|entity| entity.id == EntityId::new(1000));
        app.update();

        let visuals = visual_entries(&mut app);
        assert_eq!(visuals.len(), 1);
        assert_eq!(visuals[0].1, EntityId::new(1000));
        assert_eq!(
            app.world()
                .resource::<LockstepSimVisualState>()
                .entity_visuals
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            vec![EntityId::new(1000)]
        );

        app.world_mut()
            .resource_mut::<LockstepSimSceneState>()
            .reset();
        app.update();

        let mut visual_query = app.world_mut().query::<&LockstepSimEntityVisual>();
        assert_eq!(visual_query.iter(app.world()).count(), 0);
        let mut scene_roots = app.world_mut().query::<&BevySceneRoot>();
        assert_eq!(scene_roots.iter(app.world()).count(), 0);
        let mut camera_targets = app.world_mut().query::<&SceneCameraTarget>();
        assert_eq!(camera_targets.iter(app.world()).count(), 0);
        assert_eq!(
            *app.world().resource::<LockstepSimVisualState>(),
            LockstepSimVisualState::default()
        );
    }
}
