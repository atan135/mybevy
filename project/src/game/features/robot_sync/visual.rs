use std::collections::BTreeMap;

use bevy::{gltf::GltfAssetLabel, prelude::*, scene::SceneRoot as BevySceneRoot};

use crate::{
    framework::scene::prelude::{SceneOwned, SceneRuntimeRoot, SceneSessionId},
    game::authority::AuthoritySession,
};

use super::{
    config::RobotSyncConfig,
    state::RobotSyncSceneState,
    sync::{FIXED_UNIT, RobotState},
};

const ROBOT_LOCAL_MODEL_ASSET_PATH: &str = "models/characters/kaykit_adventurers/Knight.glb";
const ROBOT_REMOTE_MODEL_ASSET_PATHS: [&str; 2] = [
    "models/characters/kaykit_adventurers/Rogue.glb",
    "models/characters/kaykit_adventurers/Mage.glb",
];
const ROBOT_MODEL_SCALE: f32 = 14.0;
const ROBOT_MODEL_Y: f32 = 0.05;

#[derive(Clone, Debug, Default, Resource, PartialEq, Eq)]
pub(in crate::game::features::robot_sync) struct RobotSyncVisualState {
    pub(in crate::game::features::robot_sync) robot_entities: BTreeMap<String, Entity>,
    pub(in crate::game::features::robot_sync) tracked_robot_entities: usize,
}

impl RobotSyncVisualState {
    pub(in crate::game::features::robot_sync) fn clear(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Debug, Component, PartialEq, Eq)]
pub(in crate::game::features::robot_sync) struct RobotSyncRobotVisual {
    pub(in crate::game::features::robot_sync) player_id: String,
    pub(in crate::game::features::robot_sync) session_id: SceneSessionId,
    pub(in crate::game::features::robot_sync) color_index: usize,
    pub(in crate::game::features::robot_sync) is_local_player: bool,
}

pub(in crate::game::features::robot_sync) fn clear_robot_sync_visuals(
    state: &mut RobotSyncVisualState,
) {
    state.clear();
}

pub(in crate::game::features::robot_sync) fn sync_robot_sync_robot_visuals(
    mut commands: Commands,
    asset_server: Option<Res<AssetServer>>,
    config: Res<RobotSyncConfig>,
    scene_state: Res<RobotSyncSceneState>,
    replay_state: Res<super::sync::RobotSyncReplayState>,
    authority_session: Res<AuthoritySession>,
    mut visual_state: ResMut<RobotSyncVisualState>,
    runtime_roots: Query<(Entity, &SceneRuntimeRoot)>,
    mut robot_visuals: Query<(
        Entity,
        &mut RobotSyncRobotVisual,
        &mut Transform,
        Option<&mut BevySceneRoot>,
    )>,
) {
    if !scene_state.active {
        despawn_all_robot_visuals(&mut commands, &mut visual_state, robot_visuals.iter_mut());
        return;
    }

    let Some(session_id) = scene_state.session_id.as_ref() else {
        return;
    };
    let local_player_id = authority_session
        .local_player_id
        .as_deref()
        .unwrap_or(config.local_player_id.as_str());
    let mut live_robot_entities = BTreeMap::new();

    for (entity, mut visual, mut transform, scene_root) in &mut robot_visuals {
        let should_remove = &visual.session_id != session_id
            || !replay_state.robots.contains_key(&visual.player_id);

        if should_remove {
            commands.entity(entity).despawn();
            remove_robot_entity_mapping_if_current(&mut visual_state, &visual.player_id, entity);
            continue;
        }

        let Some(robot) = replay_state.robots.get(&visual.player_id) else {
            continue;
        };
        let Some(mut scene_root) = scene_root else {
            commands.entity(entity).despawn();
            remove_robot_entity_mapping_if_current(&mut visual_state, &visual.player_id, entity);
            continue;
        };
        if live_robot_entities.contains_key(&visual.player_id) {
            commands.entity(entity).despawn();
            remove_robot_entity_mapping_if_current(&mut visual_state, &visual.player_id, entity);
            continue;
        }

        let is_local_player = local_player_id == visual.player_id.as_str();
        let should_update_model =
            visual.color_index != robot.color_index || visual.is_local_player != is_local_player;
        visual.color_index = robot.color_index;
        visual.is_local_player = is_local_player;
        if should_update_model && let Some(asset_server) = asset_server.as_deref() {
            scene_root.0 =
                robot_model_scene_handle(asset_server, robot.color_index, is_local_player);
        }
        apply_robot_visual_state(&mut transform, robot);
        live_robot_entities.insert(visual.player_id.clone(), entity);
    }
    visual_state.robot_entities = live_robot_entities;

    let Some(runtime_root) = find_runtime_root_entity(session_id, runtime_roots.iter()) else {
        if !replay_state.robots.is_empty() {
            warn!(
                "skipping robot sync robot visuals because session `{}` has no runtime root",
                session_id
            );
        }
        visual_state.tracked_robot_entities = visual_state.robot_entities.len();
        return;
    };

    let Some(asset_server) = asset_server.as_deref() else {
        if !replay_state.robots.is_empty() {
            warn!("skipping robot sync robot visuals because AssetServer is unavailable");
        }
        visual_state.tracked_robot_entities = visual_state.robot_entities.len();
        return;
    };

    for (player_id, robot) in &replay_state.robots {
        if visual_state.robot_entities.contains_key(player_id) {
            continue;
        }

        let is_local_player = local_player_id == player_id.as_str();
        let entity = spawn_robot_visual(
            &mut commands,
            &asset_server,
            runtime_root,
            session_id,
            robot,
            is_local_player,
        );
        visual_state
            .robot_entities
            .insert(player_id.clone(), entity);
    }

    visual_state.tracked_robot_entities = visual_state.robot_entities.len();
}

fn remove_robot_entity_mapping_if_current(
    visual_state: &mut RobotSyncVisualState,
    player_id: &str,
    entity: Entity,
) {
    if visual_state.robot_entities.get(player_id) == Some(&entity) {
        visual_state.robot_entities.remove(player_id);
    }
}

fn despawn_all_robot_visuals<'world>(
    commands: &mut Commands,
    visual_state: &mut RobotSyncVisualState,
    robot_visuals: impl IntoIterator<
        Item = (
            Entity,
            Mut<'world, RobotSyncRobotVisual>,
            Mut<'world, Transform>,
            Option<Mut<'world, BevySceneRoot>>,
        ),
    >,
) {
    for (entity, _, _, _) in robot_visuals {
        commands.entity(entity).despawn();
    }
    visual_state.clear();
}

fn spawn_robot_visual(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    session_id: &SceneSessionId,
    robot: &RobotState,
    is_local_player: bool,
) -> Entity {
    let scene_handle = robot_model_scene_handle(asset_server, robot.color_index, is_local_player);
    let entity = commands
        .spawn((
            BevySceneRoot(scene_handle),
            robot_model_transform(robot),
            SceneOwned::new(session_id.clone()),
            RobotSyncRobotVisual {
                player_id: robot.player_id.clone(),
                session_id: session_id.clone(),
                color_index: robot.color_index,
                is_local_player,
            },
            Name::new(format!("RobotSyncRobot({})", robot.player_id)),
        ))
        .id();
    commands.entity(parent).add_child(entity);
    entity
}

fn apply_robot_visual_state(transform: &mut Transform, robot: &RobotState) {
    transform.translation = robot_world_translation(robot);
}

fn robot_model_transform(robot: &RobotState) -> Transform {
    Transform::from_translation(robot_world_translation(robot))
        .with_scale(Vec3::splat(ROBOT_MODEL_SCALE))
}

fn robot_world_translation(robot: &RobotState) -> Vec3 {
    Vec3::new(
        robot.position.x as f32 / FIXED_UNIT as f32,
        ROBOT_MODEL_Y,
        robot.position.y as f32 / FIXED_UNIT as f32,
    )
}

fn robot_model_asset_path(color_index: usize, is_local_player: bool) -> &'static str {
    if is_local_player {
        ROBOT_LOCAL_MODEL_ASSET_PATH
    } else {
        ROBOT_REMOTE_MODEL_ASSET_PATHS[color_index % ROBOT_REMOTE_MODEL_ASSET_PATHS.len()]
    }
}

fn robot_model_scene_handle(
    asset_server: &AssetServer,
    color_index: usize,
    is_local_player: bool,
) -> Handle<bevy::scene::Scene> {
    asset_server.load(
        GltfAssetLabel::Scene(0).from_asset(robot_model_asset_path(color_index, is_local_player)),
    )
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
        game::{
            authority::AuthoritySession,
            features::robot_sync::{
                state::RobotSyncSceneState,
                sync::{FixedPosition, RobotSyncReplayState},
            },
            scenes::ROBOT_SYNC_ARENA_SCENE_ID,
        },
    };

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin))
            .init_asset::<bevy::scene::Scene>()
            .init_resource::<RobotSyncConfig>()
            .init_resource::<RobotSyncSceneState>()
            .init_resource::<RobotSyncReplayState>()
            .init_resource::<RobotSyncVisualState>()
            .init_resource::<AuthoritySession>()
            .add_systems(Update, sync_robot_sync_robot_visuals);
        app
    }

    fn activate_scene_with_runtime_root(app: &mut App) -> (SceneSessionId, Entity) {
        let session_id = SceneSessionId::from("robot-sync-session");
        app.world_mut()
            .resource_mut::<RobotSyncSceneState>()
            .activate(SceneId::from(ROBOT_SYNC_ARENA_SCENE_ID), session_id.clone());
        let scene_root = spawn_scene_root(
            &mut app.world_mut().commands(),
            &SceneId::from(ROBOT_SYNC_ARENA_SCENE_ID),
            &session_id,
        );
        let runtime_root =
            spawn_scene_runtime_root(&mut app.world_mut().commands(), scene_root, &session_id);
        app.update();
        (session_id, runtime_root)
    }

    fn insert_robot(
        app: &mut App,
        player_id: &str,
        position: FixedPosition,
        spawn_index: usize,
        color_index: usize,
    ) {
        app.world_mut()
            .resource_mut::<RobotSyncReplayState>()
            .robots
            .insert(
                player_id.to_string(),
                RobotState {
                    player_id: player_id.to_string(),
                    position,
                    dir_x: 1000,
                    dir_y: 0,
                    speed: 60_000,
                    last_input_seq: None,
                    last_frame: None,
                    spawn_index,
                    color_index,
                },
            );
    }

    fn visual_entries(
        app: &mut App,
    ) -> Vec<(
        Entity,
        String,
        bool,
        usize,
        Entity,
        SceneSessionId,
        Vec3,
        Vec3,
        String,
        String,
    )> {
        let mut query = app.world_mut().query::<(
            Entity,
            &RobotSyncRobotVisual,
            &ChildOf,
            &SceneOwned,
            &Transform,
            &BevySceneRoot,
            &Name,
        )>();
        let asset_server = app.world().resource::<AssetServer>().clone();
        query
            .iter(app.world())
            .map(
                |(entity, visual, parent, owned, transform, scene_root, name)| {
                    (
                        entity,
                        visual.player_id.clone(),
                        visual.is_local_player,
                        visual.color_index,
                        parent.parent(),
                        owned.session_id.clone(),
                        transform.translation,
                        transform.scale,
                        robot_scene_asset_path(&asset_server, scene_root),
                        name.as_str().to_string(),
                    )
                },
            )
            .collect()
    }

    fn robot_scene_asset_path(asset_server: &AssetServer, scene_root: &BevySceneRoot) -> String {
        asset_server
            .get_path(scene_root.0.id().untyped())
            .expect("robot model scene handle should have an asset path")
            .to_string()
    }

    fn visual_entity_for(app: &App, player_id: &str) -> Entity {
        *app.world()
            .resource::<RobotSyncVisualState>()
            .robot_entities
            .get(player_id)
            .expect("robot visual should be tracked")
    }

    #[test]
    fn robot_sync_visuals_spawn_one_glb_root_per_player_under_runtime_root() {
        let mut app = test_app();
        let (session_id, runtime_root) = activate_scene_with_runtime_root(&mut app);
        app.world_mut()
            .resource_mut::<AuthoritySession>()
            .local_player_id = Some("player-a".to_string());
        insert_robot(&mut app, "player-a", FixedPosition { x: 0, y: 0 }, 0, 0);
        insert_robot(&mut app, "player-b", FixedPosition { x: 0, y: 0 }, 1, 1);

        app.update();

        let visuals = visual_entries(&mut app);
        assert_eq!(visuals.len(), 2);

        let local = visuals
            .iter()
            .find(|(_, player_id, _, _, _, _, _, _, _, _)| player_id == "player-a")
            .expect("local robot visual should exist");
        assert_eq!(local.4, runtime_root);
        assert_eq!(local.5, session_id);
        assert!(local.2);
        assert_eq!(local.3, 0);
        assert_eq!(local.6, Vec3::new(0.0, ROBOT_MODEL_Y, 0.0));
        assert_eq!(local.7, Vec3::splat(ROBOT_MODEL_SCALE));
        assert_eq!(
            local.8,
            "models/characters/kaykit_adventurers/Knight.glb#Scene0"
        );
        assert_eq!(local.9, "RobotSyncRobot(player-a)");

        let remote = visuals
            .iter()
            .find(|(_, player_id, _, _, _, _, _, _, _, _)| player_id == "player-b")
            .expect("remote robot visual should exist");
        assert_eq!(remote.4, runtime_root);
        assert_eq!(remote.5, session_id);
        assert!(!remote.2);
        assert_eq!(remote.3, 1);
        assert_eq!(remote.6, Vec3::new(0.0, ROBOT_MODEL_Y, 0.0));
        assert_eq!(remote.7, Vec3::splat(ROBOT_MODEL_SCALE));
        assert_eq!(
            remote.8,
            "models/characters/kaykit_adventurers/Mage.glb#Scene0"
        );
        assert_eq!(remote.9, "RobotSyncRobot(player-b)");

        let visual_state = app.world().resource::<RobotSyncVisualState>();
        assert_eq!(visual_state.tracked_robot_entities, 2);
        assert_eq!(visual_state.robot_entities.len(), 2);
    }

    #[test]
    fn robot_sync_visuals_map_fixed_coordinates_to_3d_xz_plane() {
        let robot = RobotState {
            player_id: "player-a".to_string(),
            position: FixedPosition {
                x: 123_000,
                y: -45_000,
            },
            dir_x: 1000,
            dir_y: 0,
            speed: 60_000,
            last_input_seq: None,
            last_frame: None,
            spawn_index: 0,
            color_index: 0,
        };

        assert_eq!(
            robot_world_translation(&robot),
            Vec3::new(123.0, ROBOT_MODEL_Y, -45.0)
        );
        assert_eq!(
            robot_model_transform(&robot).scale,
            Vec3::splat(ROBOT_MODEL_SCALE)
        );
    }

    #[test]
    fn robot_sync_model_asset_selection_distinguishes_local_and_remote_color_index() {
        assert_eq!(
            robot_model_asset_path(0, true),
            ROBOT_LOCAL_MODEL_ASSET_PATH
        );
        assert_eq!(
            robot_model_asset_path(7, true),
            ROBOT_LOCAL_MODEL_ASSET_PATH
        );
        assert_eq!(
            robot_model_asset_path(0, false),
            "models/characters/kaykit_adventurers/Rogue.glb"
        );
        assert_eq!(
            robot_model_asset_path(1, false),
            "models/characters/kaykit_adventurers/Mage.glb"
        );
        assert_eq!(
            robot_model_asset_path(2, false),
            "models/characters/kaykit_adventurers/Rogue.glb"
        );
    }

    #[test]
    fn robot_sync_visuals_update_transform_from_fixed_position() {
        let mut app = test_app();
        activate_scene_with_runtime_root(&mut app);
        insert_robot(&mut app, "player-a", FixedPosition { x: 0, y: 0 }, 0, 2);
        app.update();

        app.world_mut()
            .resource_mut::<RobotSyncReplayState>()
            .robots
            .get_mut("player-a")
            .unwrap()
            .position = FixedPosition {
            x: 123_000,
            y: -45_000,
        };
        app.update();

        let mut query = app
            .world_mut()
            .query::<(&RobotSyncRobotVisual, &Transform)>();
        let (_, transform) = query
            .iter(app.world())
            .find(|(visual, _)| visual.player_id == "player-a")
            .expect("robot visual should exist");
        assert_eq!(
            transform.translation,
            Vec3::new(123.0, ROBOT_MODEL_Y, -45.0)
        );
        assert_eq!(transform.scale, Vec3::splat(ROBOT_MODEL_SCALE));
    }

    #[test]
    fn robot_sync_visuals_update_global_transform_for_rendering() {
        let mut app = test_app();
        activate_scene_with_runtime_root(&mut app);
        app.world_mut()
            .resource_mut::<AuthoritySession>()
            .local_player_id = Some("player-a".to_string());
        insert_robot(
            &mut app,
            "player-a",
            FixedPosition {
                x: -119_176,
                y: -20_824,
            },
            0,
            0,
        );

        app.update();
        app.update();

        let mut query = app
            .world_mut()
            .query::<(&RobotSyncRobotVisual, &Transform, &GlobalTransform)>();
        let (_, transform, global_transform) = query
            .iter(app.world())
            .find(|(visual, _, _)| visual.player_id == "player-a")
            .expect("robot visual should exist");

        let expected = Vec3::new(-119.176, ROBOT_MODEL_Y, -20.824);
        assert_eq!(transform.translation, expected);
        assert_eq!(global_transform.translation(), expected);
    }

    #[test]
    fn robot_sync_visuals_remove_entity_when_player_disappears() {
        let mut app = test_app();
        activate_scene_with_runtime_root(&mut app);
        insert_robot(&mut app, "player-a", FixedPosition { x: 0, y: 0 }, 0, 0);
        insert_robot(
            &mut app,
            "player-b",
            FixedPosition { x: 10_000, y: 0 },
            1,
            1,
        );
        app.update();

        app.world_mut()
            .resource_mut::<RobotSyncReplayState>()
            .robots
            .remove("player-b");
        app.update();

        let mut query = app.world_mut().query::<&RobotSyncRobotVisual>();
        let player_ids = query
            .iter(app.world())
            .map(|visual| visual.player_id.clone())
            .collect::<Vec<_>>();
        assert_eq!(player_ids, vec!["player-a"]);

        let visual_state = app.world().resource::<RobotSyncVisualState>();
        assert_eq!(visual_state.tracked_robot_entities, 1);
        assert!(visual_state.robot_entities.contains_key("player-a"));
        assert!(!visual_state.robot_entities.contains_key("player-b"));

        let visuals = visual_entries(&mut app);
        assert_eq!(visuals.len(), 1);
        assert_eq!(visuals[0].1, "player-a");
    }

    #[test]
    fn robot_sync_visuals_remove_stale_session_without_losing_current_mapping() {
        let mut app = test_app();
        let (session_id, runtime_root) = activate_scene_with_runtime_root(&mut app);
        insert_robot(&mut app, "player-a", FixedPosition { x: 0, y: 0 }, 0, 0);
        app.update();

        let current_entity = *app
            .world()
            .resource::<RobotSyncVisualState>()
            .robot_entities
            .get("player-a")
            .expect("current visual should be tracked");
        let stale_scene_handle = {
            let asset_server = app.world().resource::<AssetServer>().clone();
            robot_model_scene_handle(&asset_server, 1, false)
        };
        let stale_entity = app
            .world_mut()
            .spawn((
                BevySceneRoot(stale_scene_handle),
                Transform::default(),
                SceneOwned::new(SceneSessionId::from("old-session")),
                RobotSyncRobotVisual {
                    player_id: "player-a".to_string(),
                    session_id: SceneSessionId::from("old-session"),
                    color_index: 0,
                    is_local_player: false,
                },
                Name::new("RobotSyncRobot(stale-player-a)"),
            ))
            .id();
        app.world_mut()
            .entity_mut(runtime_root)
            .add_child(stale_entity);

        app.update();

        let visual_state = app.world().resource::<RobotSyncVisualState>();
        assert_eq!(
            visual_state.robot_entities.get("player-a"),
            Some(&current_entity)
        );

        let mut query = app.world_mut().query::<&RobotSyncRobotVisual>();
        let player_a_count = query
            .iter(app.world())
            .filter(|visual| visual.player_id == "player-a")
            .count();
        assert_eq!(player_a_count, 1);

        let current_visual = app
            .world()
            .get::<RobotSyncRobotVisual>(current_entity)
            .unwrap();
        assert_eq!(current_visual.session_id, session_id);

        let visuals = visual_entries(&mut app);
        assert_eq!(visuals.len(), 1);
        assert_eq!(visuals[0].0, current_entity);
    }

    #[test]
    fn robot_sync_visuals_recover_from_stale_entity_mapping() {
        let mut app = test_app();
        activate_scene_with_runtime_root(&mut app);
        insert_robot(&mut app, "player-a", FixedPosition { x: 0, y: 0 }, 0, 0);
        app.world_mut()
            .resource_mut::<RobotSyncVisualState>()
            .robot_entities
            .insert("player-a".to_string(), Entity::PLACEHOLDER);

        app.update();

        let mut query = app.world_mut().query::<&RobotSyncRobotVisual>();
        let visuals = query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(visuals.len(), 1);
        assert_eq!(visuals[0].player_id, "player-a");

        let visual_state = app.world().resource::<RobotSyncVisualState>();
        assert_ne!(
            visual_state.robot_entities.get("player-a"),
            Some(&Entity::PLACEHOLDER)
        );
        assert_eq!(visual_state.tracked_robot_entities, 1);
    }

    #[test]
    fn robot_sync_visuals_clear_entities_and_state_when_scene_inactive() {
        let mut app = test_app();
        activate_scene_with_runtime_root(&mut app);
        insert_robot(&mut app, "player-a", FixedPosition { x: 0, y: 0 }, 0, 0);
        app.update();

        app.world_mut()
            .resource_mut::<RobotSyncSceneState>()
            .reset();
        app.update();

        let mut query = app.world_mut().query::<&RobotSyncRobotVisual>();
        assert_eq!(query.iter(app.world()).count(), 0);
        let mut scenes = app.world_mut().query::<&BevySceneRoot>();
        assert_eq!(scenes.iter(app.world()).count(), 0);
        assert_eq!(
            *app.world().resource::<RobotSyncVisualState>(),
            RobotSyncVisualState::default()
        );
    }

    #[test]
    fn robot_sync_local_player_visual_distinction_uses_different_glb_assets() {
        let mut app = test_app();
        activate_scene_with_runtime_root(&mut app);
        app.world_mut()
            .resource_mut::<AuthoritySession>()
            .local_player_id = Some("player-local".to_string());
        insert_robot(&mut app, "player-local", FixedPosition { x: 0, y: 0 }, 0, 3);
        insert_robot(
            &mut app,
            "player-remote",
            FixedPosition { x: 50_000, y: 0 },
            1,
            4,
        );

        app.update();

        let visuals = visual_entries(&mut app);
        let local = visuals
            .iter()
            .find(|(_, player_id, _, _, _, _, _, _, _, _)| player_id == "player-local")
            .unwrap();
        let remote = visuals
            .iter()
            .find(|(_, player_id, _, _, _, _, _, _, _, _)| player_id == "player-remote")
            .unwrap();

        assert!(local.2);
        assert!(!remote.2);
        assert_eq!(
            local.8,
            "models/characters/kaykit_adventurers/Knight.glb#Scene0"
        );
        assert_eq!(
            remote.8,
            "models/characters/kaykit_adventurers/Rogue.glb#Scene0"
        );
    }

    #[test]
    fn robot_sync_visuals_use_configured_local_player_when_session_is_pending() {
        let mut app = test_app();
        activate_scene_with_runtime_root(&mut app);
        app.world_mut()
            .resource_mut::<RobotSyncConfig>()
            .local_player_id = "player-local".to_string();
        insert_robot(
            &mut app,
            "player-local",
            FixedPosition { x: 20_000, y: 0 },
            0,
            0,
        );
        insert_robot(
            &mut app,
            "player-remote",
            FixedPosition { x: 0, y: 0 },
            1,
            1,
        );

        app.update();

        let visuals = visual_entries(&mut app);
        let local = visuals
            .iter()
            .find(|(_, player_id, _, _, _, _, _, _, _, _)| player_id == "player-local")
            .unwrap();
        let remote = visuals
            .iter()
            .find(|(_, player_id, _, _, _, _, _, _, _, _)| player_id == "player-remote")
            .unwrap();

        assert!(local.2);
        assert_eq!(local.6, Vec3::new(20.0, ROBOT_MODEL_Y, 0.0));
        assert_eq!(
            local.8,
            "models/characters/kaykit_adventurers/Knight.glb#Scene0"
        );
        assert!(!remote.2);
        assert_eq!(
            remote.8,
            "models/characters/kaykit_adventurers/Mage.glb#Scene0"
        );
    }

    #[test]
    fn robot_sync_visuals_update_model_asset_when_existing_remote_color_index_changes() {
        let mut app = test_app();
        activate_scene_with_runtime_root(&mut app);
        insert_robot(
            &mut app,
            "player-remote",
            FixedPosition { x: 0, y: 0 },
            0,
            0,
        );
        app.update();

        let entity = visual_entity_for(&app, "player-remote");
        {
            let asset_server = app.world().resource::<AssetServer>().clone();
            let scene_root = app.world().get::<BevySceneRoot>(entity).unwrap();
            assert_eq!(
                robot_scene_asset_path(&asset_server, scene_root),
                "models/characters/kaykit_adventurers/Rogue.glb#Scene0"
            );
        }

        app.world_mut()
            .resource_mut::<RobotSyncReplayState>()
            .robots
            .get_mut("player-remote")
            .unwrap()
            .color_index = 1;
        app.update();

        assert_eq!(visual_entity_for(&app, "player-remote"), entity);
        let visual = app.world().get::<RobotSyncRobotVisual>(entity).unwrap();
        assert_eq!(visual.color_index, 1);
        assert!(!visual.is_local_player);
        let asset_server = app.world().resource::<AssetServer>().clone();
        let scene_root = app.world().get::<BevySceneRoot>(entity).unwrap();
        assert_eq!(
            robot_scene_asset_path(&asset_server, scene_root),
            "models/characters/kaykit_adventurers/Mage.glb#Scene0"
        );
    }

    #[test]
    fn robot_sync_visuals_update_model_asset_when_existing_player_becomes_local() {
        let mut app = test_app();
        activate_scene_with_runtime_root(&mut app);
        insert_robot(&mut app, "player-a", FixedPosition { x: 0, y: 0 }, 0, 0);
        app.update();

        let entity = visual_entity_for(&app, "player-a");
        {
            let asset_server = app.world().resource::<AssetServer>().clone();
            let scene_root = app.world().get::<BevySceneRoot>(entity).unwrap();
            assert_eq!(
                robot_scene_asset_path(&asset_server, scene_root),
                "models/characters/kaykit_adventurers/Rogue.glb#Scene0"
            );
        }

        app.world_mut()
            .resource_mut::<AuthoritySession>()
            .local_player_id = Some("player-a".to_string());
        app.world_mut()
            .resource_mut::<RobotSyncReplayState>()
            .robots
            .get_mut("player-a")
            .unwrap()
            .color_index = 1;
        app.update();

        assert_eq!(visual_entity_for(&app, "player-a"), entity);
        let visual = app.world().get::<RobotSyncRobotVisual>(entity).unwrap();
        assert_eq!(visual.color_index, 1);
        assert!(visual.is_local_player);
        let asset_server = app.world().resource::<AssetServer>().clone();
        let scene_root = app.world().get::<BevySceneRoot>(entity).unwrap();
        assert_eq!(
            robot_scene_asset_path(&asset_server, scene_root),
            "models/characters/kaykit_adventurers/Knight.glb#Scene0"
        );
    }
}
