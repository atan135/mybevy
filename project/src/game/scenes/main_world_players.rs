use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, HashSet},
    time::Duration,
};

use crate::framework::fangyuan::{
    FANGYUAN_MINIMAL_PLAYER_BLUEPRINT_PATH, FangyuanBlueprintBounds, FangyuanObjectState,
    FangyuanPlayerPosition, load_fangyuan_minimal_player_blueprint, spawn_fangyuan_player,
};
use crate::framework::scene::prelude::{
    SCENE_CAMERA_LOCAL_PLAYER_TARGET_TAG, SceneCameraTarget, SceneOwned, SceneRuntimeRoot,
    SceneSessionId,
};
use crate::game::myserver::MyServerEvent;
use crate::game::myserver::protocol::pb;

use super::main_world_contract::{
    MAIN_WORLD_ROOM_SNAPSHOT_REASON, MAIN_WORLD_SERVER_SCENE_ID,
    main_world_movement_snapshot_contains_complete_room_entities,
};
use super::main_world_snapshot::{
    MainWorldSnapshotEvent, MainWorldSnapshotSource, install_main_world_snapshot_bus,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum MainWorldPlayerOwnership {
    Local,
    Remote,
}

#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub(in crate::game) struct MainWorldPlayer {
    pub character_id: String,
    pub server_entity_id: i64,
    pub ownership: MainWorldPlayerOwnership,
    pub scene_session_id: SceneSessionId,
    pub last_authoritative_frame: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::game) struct MainWorldPlayerRegistration {
    pub character_id: String,
    pub server_entity_id: i64,
    pub server_scene_id: i32,
    pub generation: u64,
    pub authoritative_frame: u32,
    pub transform: Transform,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum MainWorldPlayerRegistrationResult {
    Created(Entity),
    Updated(Entity),
    Replaced { stale: Entity, current: Entity },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game) enum MainWorldPlayerRegistrationError {
    EmptyCharacterId,
    UnexpectedScene { actual: i32 },
    NonFiniteTransform,
    StaleGeneration { expected: u64, actual: u64 },
    StaleFrame { current: u32, actual: u32 },
    BlueprintLoadFailed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game) enum MainWorldPlayerSnapshotError {
    NotReady,
    WrongRoom,
    WrongScene { actual: i32 },
    InvalidPosition,
    Registration(MainWorldPlayerRegistrationError),
}

#[derive(Resource, Default)]
struct MainWorldPlayerRuntime {
    cached_snapshots: BTreeMap<u32, MainWorldPlayerSnapshotBatch>,
    cached_generation: u64,
    snapshot_epoch: u64,
    active_generation: u64,
    last_applied_frame: Option<u32>,
    last_applied_envelope_count: usize,
    last_error: Option<MainWorldPlayerSnapshotError>,
    was_recovering: bool,
    registry: Option<MainWorldPlayerRegistry>,
    remote_offline_deadlines: HashMap<String, Duration>,
}

const MAIN_WORLD_REMOTE_OFFLINE_GRACE: Duration = Duration::from_secs(5);
// Covers the 20-second entry watchdog at the current 20 Hz authority rate,
// with headroom for bursts while scene/content readiness settles.
const MAIN_WORLD_PLAYER_SNAPSHOT_CACHE_FRAMES: usize = 512;

#[derive(Clone, Debug)]
struct MainWorldPlayerSnapshotBatch {
    frame_id: u32,
    envelopes: Vec<MainWorldSnapshotEvent>,
}

#[derive(Clone, Debug)]
struct MainWorldMergedPlayerSnapshot {
    push: pb::MovementSnapshotPush,
    complete_visible_character_ids: Option<HashSet<String>>,
}

impl MainWorldPlayerSnapshotBatch {
    fn new(snapshot: MainWorldSnapshotEvent) -> Self {
        Self {
            frame_id: snapshot.push.frame_id,
            envelopes: vec![snapshot],
        }
    }

    fn merge(&self) -> MainWorldMergedPlayerSnapshot {
        let mut ordered: Vec<_> = self.envelopes.iter().collect();
        ordered.sort_by(|left, right| compare_main_world_snapshot_envelopes(left, right));

        let strongest = *ordered
            .last()
            .expect("a cached main-world snapshot batch is never empty");
        let mut push = strongest.push.clone();
        let complete_envelope = ordered
            .iter()
            .rev()
            .find(|envelope| {
                envelope.complete_room_entities && envelope.push.target_character_ids.is_empty()
            })
            .copied();
        let complete_visible_character_ids = complete_envelope.map(|envelope| {
            envelope
                .push
                .entities
                .iter()
                .map(|entity| entity.character_id.clone())
                .collect()
        });

        let mut entities = BTreeMap::new();
        for envelope in ordered {
            let mut ordered_entities: Vec<_> = envelope.push.entities.iter().collect();
            ordered_entities.sort_by(|left, right| compare_entity_transforms(left, right));
            for entity in ordered_entities {
                entities.insert(entity.character_id.clone(), entity.clone());
            }
        }
        push.entities = entities.into_values().collect();
        if let Some(complete) = complete_envelope {
            push.full_sync = complete.push.full_sync;
            push.reason = complete.push.reason.clone();
            push.target_character_ids = complete.push.target_character_ids.clone();
        } else {
            // A targeted strong/recovery correction sets full_sync on the wire,
            // but it is not proof that its entity list is the whole roster.
            push.full_sync = false;
            if push.reason == MAIN_WORLD_ROOM_SNAPSHOT_REASON {
                push.reason.clear();
            }
        }

        MainWorldMergedPlayerSnapshot {
            push,
            complete_visible_character_ids,
        }
    }
}

fn main_world_snapshot_correction_priority(snapshot: &MainWorldSnapshotEvent) -> u8 {
    match pb::MovementCorrectionKind::try_from(snapshot.push.correction_kind).ok() {
        Some(pb::MovementCorrectionKind::Recovery) => 4,
        Some(pb::MovementCorrectionKind::Strong) => 3,
        Some(pb::MovementCorrectionKind::FullSync) => 2,
        Some(pb::MovementCorrectionKind::Incremental) => 1,
        None => 0,
    }
}

fn compare_main_world_snapshot_envelopes(
    left: &MainWorldSnapshotEvent,
    right: &MainWorldSnapshotEvent,
) -> Ordering {
    main_world_snapshot_correction_priority(left)
        .cmp(&main_world_snapshot_correction_priority(right))
        .then_with(|| {
            left.complete_room_entities
                .cmp(&right.complete_room_entities)
        })
        .then_with(|| left.push.full_sync.cmp(&right.push.full_sync))
        .then_with(|| {
            left.push
                .reference_frame_id
                .cmp(&right.push.reference_frame_id)
        })
        .then_with(|| left.push.reason_code.cmp(&right.push.reason_code))
        .then_with(|| left.push.reason.cmp(&right.push.reason))
        .then_with(|| {
            left.push
                .target_character_ids
                .cmp(&right.push.target_character_ids)
        })
        .then_with(|| compare_entity_transform_slices(&left.push.entities, &right.push.entities))
}

fn compare_entity_transform_slices(
    left: &[pb::EntityTransform],
    right: &[pb::EntityTransform],
) -> Ordering {
    let mut left = left.iter().collect::<Vec<_>>();
    let mut right = right.iter().collect::<Vec<_>>();
    left.sort_by(|a, b| compare_entity_transforms(a, b));
    right.sort_by(|a, b| compare_entity_transforms(a, b));
    left.iter()
        .zip(&right)
        .map(|(a, b)| compare_entity_transforms(a, b))
        .find(|ordering| *ordering != Ordering::Equal)
        .unwrap_or_else(|| left.len().cmp(&right.len()))
}

fn compare_entity_transforms(left: &pb::EntityTransform, right: &pb::EntityTransform) -> Ordering {
    left.character_id
        .cmp(&right.character_id)
        .then_with(|| left.entity_id.cmp(&right.entity_id))
        .then_with(|| left.scene_id.cmp(&right.scene_id))
        .then_with(|| left.x.total_cmp(&right.x))
        .then_with(|| left.y.total_cmp(&right.y))
        .then_with(|| left.dir_x.total_cmp(&right.dir_x))
        .then_with(|| left.dir_y.total_cmp(&right.dir_y))
        .then_with(|| left.moving.cmp(&right.moving))
        .then_with(|| left.last_input_frame.cmp(&right.last_input_frame))
}

fn main_world_snapshot_targets_character(
    push: &pb::MovementSnapshotPush,
    character_id: Option<&str>,
) -> bool {
    push.target_character_ids.is_empty()
        || character_id.is_some_and(|character_id| {
            push.target_character_ids
                .iter()
                .any(|target| target == character_id)
        })
}

fn main_world_player_snapshot_contains_complete_roster(push: &pb::MovementSnapshotPush) -> bool {
    (main_world_movement_snapshot_contains_complete_room_entities(push)
        && push.target_character_ids.is_empty())
        || (push.full_sync && push.target_character_ids.is_empty())
}

fn cache_main_world_player_snapshot(
    runtime: &mut MainWorldPlayerRuntime,
    snapshot: MainWorldSnapshotEvent,
    generation: u64,
) {
    if snapshot.epoch < runtime.snapshot_epoch {
        return;
    }
    if snapshot.epoch > runtime.snapshot_epoch {
        runtime.cached_snapshots.clear();
        runtime.last_applied_frame = None;
        runtime.last_applied_envelope_count = 0;
        runtime.snapshot_epoch = snapshot.epoch;
    }
    let push = &snapshot.push;
    if runtime
        .last_applied_frame
        .is_some_and(|applied| push.frame_id < applied)
    {
        return;
    }
    match runtime.cached_snapshots.get_mut(&push.frame_id) {
        Some(cached) => {
            if !cached
                .envelopes
                .iter()
                .any(|cached| same_main_world_snapshot_envelope(cached, &snapshot))
            {
                cached.envelopes.push(snapshot);
            }
        }
        None => {
            runtime
                .cached_snapshots
                .insert(push.frame_id, MainWorldPlayerSnapshotBatch::new(snapshot));
        }
    }
    while runtime.cached_snapshots.len() > MAIN_WORLD_PLAYER_SNAPSHOT_CACHE_FRAMES {
        runtime.cached_snapshots.pop_first();
    }
    runtime.cached_generation = generation;
}

fn same_main_world_snapshot_envelope(
    left: &MainWorldSnapshotEvent,
    right: &MainWorldSnapshotEvent,
) -> bool {
    left.epoch == right.epoch
        && left.source == right.source
        && left.complete_room_entities == right.complete_room_entities
        && left.push.frame_id == right.push.frame_id
        && left.push.full_sync == right.push.full_sync
        && left.push.correction_kind == right.push.correction_kind
        && left.push.reason_code == right.push.reason_code
        && left.push.reason == right.push.reason
        && left.push.target_character_ids == right.push.target_character_ids
        && left.push.reference_frame_id == right.push.reference_frame_id
        && compare_entity_transform_slices(&left.push.entities, &right.push.entities)
            == Ordering::Equal
}

fn clear_main_world_player_snapshot_cache(runtime: &mut MainWorldPlayerRuntime) {
    runtime.cached_snapshots.clear();
    runtime.snapshot_epoch = 0;
    runtime.last_applied_frame = None;
    runtime.last_applied_envelope_count = 0;
}

pub(in crate::game) struct MainWorldPlayersPlugin;

impl Plugin for MainWorldPlayersPlugin {
    fn build(&self, app: &mut App) {
        install_main_world_snapshot_bus(app);
        app.add_message::<MyServerEvent>()
            .init_resource::<MainWorldPlayerRuntime>()
            .init_resource::<MainWorldPlayersFixture>()
            .add_systems(Startup, setup_main_world_players_fixture)
            .add_systems(
                Update,
                (
                    maintain_main_world_players
                        .after(super::main_world_entry::MainWorldEntryUpdateSet::Coordinator),
                    drive_main_world_players_fixture,
                )
                    .chain(),
            );
    }
}

#[derive(Resource, Default)]
struct MainWorldPlayersFixture {
    screenshot_path: Option<std::path::PathBuf>,
    frames: u32,
    requested: bool,
}

fn setup_main_world_players_fixture(
    mut commands: Commands,
    mut fixture: ResMut<MainWorldPlayersFixture>,
    mut entry: ResMut<super::main_world_entry::MainWorldEntryState>,
    mut snapshots: MessageWriter<MyServerEvent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    next_mode: Option<ResMut<NextState<crate::game::navigation::AppUiMode>>>,
) {
    let Some(path) = std::env::var_os("PROJECT_MAIN_WORLD_PLAYERS_FIXTURE_SCREENSHOT") else {
        return;
    };
    let path = std::path::PathBuf::from(path);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    fixture.screenshot_path = Some(path.clone());
    if let Some(mut next_mode) = next_mode {
        next_mode.set(crate::game::navigation::AppUiMode::MainWorld);
    }
    let session_id = SceneSessionId::from("main-world-players-fixture");
    entry.generation = 1;
    entry.phase = super::main_world_entry::MainWorldEntryPhase::Active;
    entry.character_id = Some("fixture-local".into());
    entry.scene_session_id = Some(session_id.clone());
    entry.scene_ready = true;
    entry.room_ready_acknowledged = true;
    commands.spawn((
        SceneRuntimeRoot::new(session_id),
        Transform::default(),
        GlobalTransform::default(),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(8.0, 8.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.18, 0.32, 0.18),
            perceptual_roughness: 0.9,
            ..default()
        })),
        Transform::from_xyz(8.0, 0.0, 8.0),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 18_000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, -0.5, 0.0)),
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(8.0, 0.75, 9.1).looking_at(Vec3::new(8.0, 0.15, 8.0), Vec3::Y),
    ));
    snapshots.write(MyServerEvent::MovementSnapshotPush(
        pb::MovementSnapshotPush {
            room_id: super::main_world_contract::MAIN_WORLD_PUBLIC_ROOM_ID.into(),
            frame_id: 1,
            full_sync: true,
            entities: vec![
                pb::EntityTransform {
                    entity_id: 1,
                    character_id: "fixture-local".into(),
                    scene_id: MAIN_WORLD_SERVER_SCENE_ID,
                    x: 2007.7,
                    y: 2008.0,
                    ..Default::default()
                },
                pb::EntityTransform {
                    entity_id: 2,
                    character_id: "fixture-remote".into(),
                    scene_id: MAIN_WORLD_SERVER_SCENE_ID,
                    x: 2008.3,
                    y: 2008.0,
                    ..Default::default()
                },
            ],
            ..Default::default()
        },
    ));
    info!(path = %path.display(), local = "fixture-local", remote = "fixture-remote", "offline main world players fixture started");
}

fn drive_main_world_players_fixture(
    mut commands: Commands,
    mut fixture: ResMut<MainWorldPlayersFixture>,
    players: Query<&MainWorldPlayer>,
) {
    let Some(path) = fixture.screenshot_path.clone() else {
        return;
    };
    fixture.frames += 1;
    if fixture.requested || fixture.frames < 60 || players.iter().count() != 2 {
        return;
    }
    fixture.requested = true;
    commands.spawn(Screenshot::primary_window()).observe(
        move |captured: On<ScreenshotCaptured>, mut exits: MessageWriter<AppExit>| {
            save_to_disk(path.clone())(captured);
            exits.write(AppExit::Success);
        },
    );
}

fn maintain_main_world_players(
    mut commands: Commands,
    mut myserver_events: MessageReader<MyServerEvent>,
    mut snapshot_events: MessageReader<MainWorldSnapshotEvent>,
    entry: Res<super::main_world_entry::MainWorldEntryState>,
    roots: Query<(Entity, &crate::framework::scene::prelude::SceneRuntimeRoot)>,
    time: Res<Time>,
    mut runtime: ResMut<MainWorldPlayerRuntime>,
) {
    if runtime.active_generation != entry.generation {
        if let Some(registry) = runtime.registry.as_mut() {
            registry.clear(&mut commands);
        }
        runtime.registry = None;
        clear_main_world_player_snapshot_cache(&mut runtime);
        runtime.last_error = None;
        runtime.snapshot_epoch = 0;
        runtime.was_recovering = false;
        runtime.remote_offline_deadlines.clear();
        runtime.active_generation = entry.generation;
    }
    let owns_visual_session = matches!(
        entry.phase,
        super::main_world_entry::MainWorldEntryPhase::JoiningRoom
            | super::main_world_entry::MainWorldEntryPhase::LoadingScene
            | super::main_world_entry::MainWorldEntryPhase::WaitingSceneReady
            | super::main_world_entry::MainWorldEntryPhase::Active
            | super::main_world_entry::MainWorldEntryPhase::Recovering
    );
    if !owns_visual_session {
        if let Some(registry) = runtime.registry.as_mut() {
            registry.clear(&mut commands);
        }
        runtime.registry = None;
        clear_main_world_player_snapshot_cache(&mut runtime);
        runtime.last_error = None;
        runtime.was_recovering = false;
        runtime.remote_offline_deadlines.clear();
        return;
    }
    // A recovered room starts a new authority-frame epoch. Reset before
    // handling recovery messages so the initial low frame replaces the old
    // epoch even when entry later passes through WaitingSceneReady.
    if !runtime.was_recovering
        && entry.phase == super::main_world_entry::MainWorldEntryPhase::Recovering
    {
        clear_main_world_player_snapshot_cache(&mut runtime);
        if let Some(registry) = runtime.registry.as_mut() {
            for player in registry.players.values_mut() {
                player.last_authoritative_frame = 0;
            }
        }
    }
    for event in myserver_events.read() {
        let MyServerEvent::RoomMemberOfflinePush(push) = event else {
            continue;
        };
        if push.room_id == super::main_world_contract::MAIN_WORLD_PUBLIC_ROOM_ID
            && push.character_id != entry.character_id.as_deref().unwrap_or_default()
        {
            if push.offline {
                runtime.remote_offline_deadlines.insert(
                    push.character_id.clone(),
                    time.elapsed() + MAIN_WORLD_REMOTE_OFFLINE_GRACE,
                );
            } else {
                runtime.remote_offline_deadlines.remove(&push.character_id);
            }
        }
    }
    for snapshot in snapshot_events.read() {
        let push = &snapshot.push;
        if push.room_id != super::main_world_contract::MAIN_WORLD_PUBLIC_ROOM_ID {
            continue;
        }
        if !main_world_snapshot_targets_character(push, entry.character_id.as_deref()) {
            continue;
        }
        cache_main_world_player_snapshot(&mut runtime, snapshot.clone(), entry.generation);
    }
    runtime.was_recovering =
        entry.phase == super::main_world_entry::MainWorldEntryPhase::Recovering;
    let Some(session_id) = entry.scene_session_id.as_ref() else {
        return;
    };
    let Some(character_id) = entry.character_id.as_ref() else {
        return;
    };
    let Some((root, _)) = roots.iter().find(|(_, root)| root.is_session(session_id)) else {
        return;
    };
    let replace_registry = runtime
        .registry
        .as_ref()
        .is_none_or(|registry| registry.session_id() != session_id);
    if replace_registry {
        if let Some(registry) = runtime.registry.as_mut() {
            registry.clear(&mut commands);
        }
        runtime.registry = Some(MainWorldPlayerRegistry::new(
            session_id.clone(),
            entry.generation,
            character_id.clone(),
            root,
        ));
        runtime.last_applied_frame = None;
        runtime.last_applied_envelope_count = 0;
    }
    let mut offline_deadlines = std::mem::take(&mut runtime.remote_offline_deadlines);
    if let Some(registry) = runtime.registry.as_mut() {
        remove_expired_remote_offline_players(
            registry,
            &mut commands,
            &mut offline_deadlines,
            time.elapsed(),
        );
    }
    runtime.remote_offline_deadlines = offline_deadlines;
    if entry.phase == super::main_world_entry::MainWorldEntryPhase::Recovering {
        if let Some(registry) = runtime.registry.as_ref() {
            for entry in registry.players.values() {
                commands.entity(entry.entity).remove::<SceneCameraTarget>();
            }
        }
        return;
    }
    if !entry.scene_ready || !entry.room_ready_acknowledged {
        return;
    }
    if runtime.cached_generation != entry.generation {
        return;
    }
    let pending_batches: Vec<_> = runtime
        .cached_snapshots
        .values()
        .filter(|batch| {
            runtime
                .last_applied_frame
                .is_none_or(|applied| batch.frame_id > applied)
                || (runtime.last_applied_frame == Some(batch.frame_id)
                    && batch.envelopes.len() > runtime.last_applied_envelope_count)
        })
        .cloned()
        .collect();
    for batch in pending_batches {
        let mut merged = batch.merge();
        suppress_expired_remote_offline_entities(
            &mut merged.push,
            &runtime.remote_offline_deadlines,
            time.elapsed(),
        );
        let snapshot = &merged.push;
        let result = runtime.registry.as_mut().map(|registry| {
            apply_main_world_snapshot(registry, &mut commands, snapshot, true, true)
        });
        match result {
            Some(Ok(())) => {
                runtime.last_applied_frame = Some(snapshot.frame_id);
                runtime.last_applied_envelope_count = batch.envelopes.len();
                runtime.last_error = None;
                if let Some(visible) = merged.complete_visible_character_ids.as_ref() {
                    let mut offline_deadlines =
                        std::mem::take(&mut runtime.remote_offline_deadlines);
                    if let Some(registry) = runtime.registry.as_mut() {
                        let offline: Vec<_> = offline_deadlines.keys().cloned().collect();
                        for character_id in offline {
                            if !visible.contains(&character_id) {
                                registry.remove_remote(&mut commands, &character_id);
                                // Preserve an expired tombstone until an explicit
                                // online notice proves that snapshots may spawn it again.
                                offline_deadlines.insert(character_id, time.elapsed());
                            }
                        }
                    }
                    runtime.remote_offline_deadlines = offline_deadlines;
                }
            }
            Some(Err(error)) => {
                warn!(
                    ?error,
                    frame_id = snapshot.frame_id,
                    "main world player snapshot rejected"
                );
                runtime.last_applied_frame = Some(snapshot.frame_id);
                runtime.last_applied_envelope_count = batch.envelopes.len();
                runtime.last_error = Some(error);
            }
            None => break,
        }
    }
    if entry.phase == super::main_world_entry::MainWorldEntryPhase::Active {
        if let Some(registry) = runtime.registry.as_ref() {
            for (character_id, player) in &registry.players {
                if character_id == &registry.local_character_id {
                    commands.entity(player.entity).insert(
                        SceneCameraTarget::new(registry.session_id.clone())
                            .with_tag(SCENE_CAMERA_LOCAL_PLAYER_TARGET_TAG),
                    );
                } else {
                    commands.entity(player.entity).remove::<SceneCameraTarget>();
                }
            }
        }
    }
}

fn remove_expired_remote_offline_players(
    registry: &mut MainWorldPlayerRegistry,
    commands: &mut Commands,
    deadlines: &mut HashMap<String, Duration>,
    now: Duration,
) {
    let expired: Vec<_> = deadlines
        .iter()
        .filter_map(|(character_id, deadline)| (*deadline <= now).then_some(character_id.clone()))
        .collect();
    for character_id in expired {
        registry.remove_remote(commands, &character_id);
    }
}

fn suppress_expired_remote_offline_entities(
    push: &mut pb::MovementSnapshotPush,
    deadlines: &HashMap<String, Duration>,
    now: Duration,
) {
    push.entities.retain(|entity| {
        !deadlines
            .get(&entity.character_id)
            .is_some_and(|deadline| *deadline <= now)
    });
}

#[derive(Resource, Clone, Debug)]
pub(in crate::game) struct MainWorldPlayerRegistry {
    session_id: SceneSessionId,
    generation: u64,
    local_character_id: String,
    runtime_root: Entity,
    players: HashMap<String, MainWorldPlayerRegistryEntry>,
    player_blueprint: Option<MainWorldPlayerBlueprint>,
}

#[derive(Clone, Debug)]
struct MainWorldPlayerBlueprint {
    bounds: FangyuanBlueprintBounds,
    primitive_set: crate::framework::fangyuan::FangyuanPrimitiveSet,
}

#[derive(Clone, Copy, Debug)]
struct MainWorldPlayerRegistryEntry {
    entity: Entity,
    server_entity_id: i64,
    last_authoritative_frame: u32,
}

impl MainWorldPlayerRegistry {
    pub fn new(
        session_id: impl Into<SceneSessionId>,
        generation: u64,
        local_character_id: impl Into<String>,
        runtime_root: Entity,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            generation,
            local_character_id: local_character_id.into(),
            runtime_root,
            players: HashMap::new(),
            player_blueprint: None,
        }
    }

    pub fn session_id(&self) -> &SceneSessionId {
        &self.session_id
    }

    pub fn get(&self, character_id: &str) -> Option<Entity> {
        self.players.get(character_id).map(|entry| entry.entity)
    }

    pub fn len(&self) -> usize {
        self.players.len()
    }

    pub fn clear(&mut self, commands: &mut Commands) {
        for entry in self.players.drain().map(|(_, entry)| entry) {
            commands.entity(entry.entity).despawn();
        }
    }

    fn remove_missing(
        &mut self,
        commands: &mut Commands,
        visible: &std::collections::HashSet<String>,
    ) {
        let stale: Vec<_> = self
            .players
            .keys()
            .filter(|id| !visible.contains(*id))
            .cloned()
            .collect();
        for character_id in stale {
            if let Some(entry) = self.players.remove(&character_id) {
                commands.entity(entry.entity).despawn();
            }
        }
    }

    /// An offline notice concerns presence, not the ticket owner.  The local
    /// player remains visible until entry/session orchestration tears it down.
    fn remove_remote(&mut self, commands: &mut Commands, character_id: &str) {
        if character_id == self.local_character_id {
            return;
        }
        if let Some(entry) = self.players.remove(character_id) {
            commands.entity(entry.entity).despawn();
        }
    }

    pub fn register(
        &mut self,
        commands: &mut Commands,
        registration: MainWorldPlayerRegistration,
    ) -> Result<MainWorldPlayerRegistrationResult, MainWorldPlayerRegistrationError> {
        self.validate(&registration)?;
        let ownership = if registration.character_id == self.local_character_id {
            MainWorldPlayerOwnership::Local
        } else {
            MainWorldPlayerOwnership::Remote
        };
        let player = MainWorldPlayer {
            character_id: registration.character_id.clone(),
            server_entity_id: registration.server_entity_id,
            ownership,
            scene_session_id: self.session_id.clone(),
            last_authoritative_frame: registration.authoritative_frame,
        };

        if let Some(existing) = self.players.get(&registration.character_id).copied() {
            if registration.authoritative_frame < existing.last_authoritative_frame {
                return Err(MainWorldPlayerRegistrationError::StaleFrame {
                    current: existing.last_authoritative_frame,
                    actual: registration.authoritative_frame,
                });
            }
            if existing.server_entity_id == registration.server_entity_id {
                if ownership == MainWorldPlayerOwnership::Remote {
                    // Remote spatial presentation is owned exclusively by the
                    // interpolation system. Re-inserting authoritative spatial
                    // components here would snap to each low-frequency sample
                    // before interpolation writes the delayed visual position,
                    // producing a visible back-and-forth jitter every snapshot.
                    commands.entity(existing.entity).insert(player);
                } else {
                    let root_transform = main_world_player_root_transform(
                        registration.transform,
                        self.player_blueprint()?.bounds,
                    );
                    commands.entity(existing.entity).insert((
                        player,
                        FangyuanPlayerPosition {
                            translation: root_transform.translation,
                        },
                        FangyuanObjectState::new(root_transform.translation, root_transform.scale),
                        root_transform,
                    ));
                }
                self.players.insert(
                    registration.character_id,
                    MainWorldPlayerRegistryEntry {
                        entity: existing.entity,
                        server_entity_id: existing.server_entity_id,
                        last_authoritative_frame: registration.authoritative_frame,
                    },
                );
                return Ok(MainWorldPlayerRegistrationResult::Updated(existing.entity));
            }
            commands.entity(existing.entity).despawn();
            let player_blueprint = self.player_blueprint()?.clone();
            let root_transform =
                main_world_player_root_transform(registration.transform, player_blueprint.bounds);
            let current = spawn_player_root(
                commands,
                self.runtime_root,
                &self.session_id,
                player,
                player_blueprint.primitive_set,
                root_transform,
            );
            self.players.insert(
                registration.character_id,
                MainWorldPlayerRegistryEntry {
                    entity: current,
                    server_entity_id: registration.server_entity_id,
                    last_authoritative_frame: registration.authoritative_frame,
                },
            );
            return Ok(MainWorldPlayerRegistrationResult::Replaced {
                stale: existing.entity,
                current,
            });
        }

        let character_id = registration.character_id;
        let player_blueprint = self.player_blueprint()?.clone();
        let root_transform =
            main_world_player_root_transform(registration.transform, player_blueprint.bounds);
        let current = spawn_player_root(
            commands,
            self.runtime_root,
            &self.session_id,
            player,
            player_blueprint.primitive_set,
            root_transform,
        );
        self.players.insert(
            character_id,
            MainWorldPlayerRegistryEntry {
                entity: current,
                server_entity_id: registration.server_entity_id,
                last_authoritative_frame: registration.authoritative_frame,
            },
        );
        Ok(MainWorldPlayerRegistrationResult::Created(current))
    }

    fn player_blueprint(
        &mut self,
    ) -> Result<&MainWorldPlayerBlueprint, MainWorldPlayerRegistrationError> {
        if self.player_blueprint.is_none() {
            let blueprint = load_fangyuan_minimal_player_blueprint()
                .map_err(|_| MainWorldPlayerRegistrationError::BlueprintLoadFailed)?;
            let primitive_set = blueprint
                .compile()
                .map_err(|_| MainWorldPlayerRegistrationError::BlueprintLoadFailed)?;
            self.player_blueprint = Some(MainWorldPlayerBlueprint {
                bounds: blueprint.bounds,
                primitive_set,
            });
        }
        Ok(self
            .player_blueprint
            .as_ref()
            .expect("player blueprint was initialized"))
    }

    fn validate(
        &self,
        registration: &MainWorldPlayerRegistration,
    ) -> Result<(), MainWorldPlayerRegistrationError> {
        if registration.character_id.trim().is_empty() {
            return Err(MainWorldPlayerRegistrationError::EmptyCharacterId);
        }
        if registration.server_scene_id != MAIN_WORLD_SERVER_SCENE_ID {
            return Err(MainWorldPlayerRegistrationError::UnexpectedScene {
                actual: registration.server_scene_id,
            });
        }
        if registration.generation != self.generation {
            return Err(MainWorldPlayerRegistrationError::StaleGeneration {
                expected: self.generation,
                actual: registration.generation,
            });
        }
        if !registration.transform.translation.is_finite()
            || !registration.transform.rotation.is_finite()
            || !registration.transform.scale.is_finite()
        {
            return Err(MainWorldPlayerRegistrationError::NonFiniteTransform);
        }
        Ok(())
    }
}

/// Adapter for authoritative snapshots. Entry orchestration supplies the
/// session/generation gate; this runtime owns all-character presentation.
pub(in crate::game) fn apply_main_world_snapshot(
    registry: &mut MainWorldPlayerRegistry,
    commands: &mut Commands,
    push: &pb::MovementSnapshotPush,
    room_ready: bool,
    scene_ready: bool,
) -> Result<(), MainWorldPlayerSnapshotError> {
    if !room_ready || !scene_ready {
        return Err(MainWorldPlayerSnapshotError::NotReady);
    }
    if push.room_id != super::main_world_contract::MAIN_WORLD_PUBLIC_ROOM_ID {
        return Err(MainWorldPlayerSnapshotError::WrongRoom);
    }
    for entity in &push.entities {
        if entity.character_id.trim().is_empty() {
            return Err(MainWorldPlayerSnapshotError::Registration(
                MainWorldPlayerRegistrationError::EmptyCharacterId,
            ));
        }
        if entity.scene_id != MAIN_WORLD_SERVER_SCENE_ID {
            return Err(MainWorldPlayerSnapshotError::WrongScene {
                actual: entity.scene_id,
            });
        }
        super::main_world_entry::main_world_bevy_position(entity.x, entity.y)
            .map_err(|_| MainWorldPlayerSnapshotError::InvalidPosition)?;
    }
    let mut visible = std::collections::HashSet::new();
    for entity in &push.entities {
        let position = super::main_world_entry::main_world_bevy_position(entity.x, entity.y)
            .map_err(|_| MainWorldPlayerSnapshotError::InvalidPosition)?;
        visible.insert(entity.character_id.clone());
        let registration = MainWorldPlayerRegistration {
            character_id: entity.character_id.clone(),
            server_entity_id: entity.entity_id as i64,
            server_scene_id: entity.scene_id,
            generation: registry.generation,
            authoritative_frame: push.frame_id,
            transform: Transform::from_translation(position),
        };
        registry
            .register(commands, registration)
            .map_err(MainWorldPlayerSnapshotError::Registration)?;
    }
    if main_world_player_snapshot_contains_complete_roster(push) {
        registry.remove_missing(commands, &visible);
    }
    Ok(())
}

fn spawn_player_root(
    commands: &mut Commands,
    runtime_root: Entity,
    session_id: &SceneSessionId,
    player: MainWorldPlayer,
    primitive_set: crate::framework::fangyuan::FangyuanPrimitiveSet,
    transform: Transform,
) -> Entity {
    let local = player.ownership == MainWorldPlayerOwnership::Local;
    let entity = spawn_fangyuan_player(
        commands,
        FANGYUAN_MINIMAL_PLAYER_BLUEPRINT_PATH,
        "Minimal Fangyuan Player",
        primitive_set,
        transform,
        (player, SceneOwned::new(session_id.clone())),
    );
    if local {
        commands.entity(entity).insert(
            SceneCameraTarget::new(session_id.clone())
                .with_tag(SCENE_CAMERA_LOCAL_PLAYER_TARGET_TAG),
        );
    }
    commands.entity(runtime_root).add_child(entity);
    entity
}

pub(in crate::game) fn main_world_player_uniform_scale(bounds: FangyuanBlueprintBounds) -> f32 {
    0.25 / bounds.width.max(bounds.depth)
}

pub(in crate::game) fn main_world_player_root_transform(
    authoritative: Transform,
    bounds: FangyuanBlueprintBounds,
) -> Transform {
    let scale = main_world_player_uniform_scale(bounds);
    Transform {
        translation: authoritative.translation,
        rotation: authoritative.rotation,
        scale: Vec3::splat(scale),
    }
}

#[cfg(test)]
mod tests {
    use super::super::main_world_entry::MainWorldEntryState;
    use super::*;
    use crate::framework::fangyuan::{
        FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT, FangyuanPlayerPrimitiveVisual,
        FangyuanPlayerRuntimePlugin,
    };
    use crate::framework::scene::prelude::SceneRuntimeRoot;

    fn registration(character: &str, entity_id: i64, frame: u32) -> MainWorldPlayerRegistration {
        MainWorldPlayerRegistration {
            character_id: character.to_owned(),
            server_entity_id: entity_id,
            server_scene_id: MAIN_WORLD_SERVER_SCENE_ID,
            generation: 3,
            authoritative_frame: frame,
            transform: Transform::from_xyz(1.0, 0.0, 2.0),
        }
    }

    fn register(
        world: &mut World,
        registry: &mut MainWorldPlayerRegistry,
        value: MainWorldPlayerRegistration,
    ) -> Result<MainWorldPlayerRegistrationResult, MainWorldPlayerRegistrationError> {
        let result = registry.register(&mut world.commands(), value);
        world.flush();
        result
    }

    fn registry(world: &mut World, session: &str, local: &str) -> MainWorldPlayerRegistry {
        let root = world.spawn_empty().id();
        MainWorldPlayerRegistry::new(session, 3, local, root)
    }

    #[test]
    fn registry_is_unique_by_character_and_updates_same_entity() {
        let mut world = World::new();
        let mut registry = registry(&mut world, "session-a", "local");
        let first = register(&mut world, &mut registry, registration("remote", 10, 1)).unwrap();
        let blueprint_ptr = registry
            .player_blueprint
            .as_ref()
            .expect("first spawn caches blueprint")
            as *const MainWorldPlayerBlueprint;
        let second = register(&mut world, &mut registry, registration("remote", 10, 2)).unwrap();
        let MainWorldPlayerRegistrationResult::Created(first) = first else {
            panic!()
        };
        assert_eq!(second, MainWorldPlayerRegistrationResult::Updated(first));
        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry
                .player_blueprint
                .as_ref()
                .expect("cached blueprint remains") as *const MainWorldPlayerBlueprint,
            blueprint_ptr
        );
        assert_eq!(
            world
                .get::<MainWorldPlayer>(first)
                .unwrap()
                .last_authoritative_frame,
            2
        );
    }

    #[test]
    fn same_entity_update_preserves_authoritative_rotation_through_runtime_sync() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, FangyuanPlayerRuntimePlugin));
        let root = app.world_mut().spawn_empty().id();
        let mut registry = MainWorldPlayerRegistry::new("session-a", 3, "local", root);
        register(app.world_mut(), &mut registry, registration("local", 10, 1)).unwrap();
        let rotation = Quat::from_rotation_y(1.25);
        let mut updated = registration("local", 10, 2);
        updated.transform.rotation = rotation;
        let result = register(app.world_mut(), &mut registry, updated).unwrap();
        let MainWorldPlayerRegistrationResult::Updated(entity) = result else {
            panic!()
        };
        app.update();
        assert_eq!(
            app.world().get::<Transform>(entity).unwrap().rotation,
            rotation
        );
    }

    #[test]
    fn remote_same_entity_update_preserves_interpolated_spatial_state() {
        let mut world = World::new();
        let mut registry = registry(&mut world, "session-a", "local");
        let created = register(&mut world, &mut registry, registration("remote", 10, 1)).unwrap();
        let MainWorldPlayerRegistrationResult::Created(entity) = created else {
            panic!()
        };
        let interpolated = Transform::from_xyz(7.0, 0.0, 9.0);
        world.entity_mut(entity).insert((
            interpolated,
            FangyuanPlayerPosition {
                translation: interpolated.translation,
            },
            FangyuanObjectState::new(interpolated.translation, interpolated.scale),
        ));

        let mut updated = registration("remote", 10, 2);
        updated.transform.translation = Vec3::new(20.0, 0.0, 30.0);
        assert!(matches!(
            register(&mut world, &mut registry, updated),
            Ok(MainWorldPlayerRegistrationResult::Updated(current)) if current == entity
        ));

        assert_eq!(world.get::<Transform>(entity).unwrap(), &interpolated);
        assert_eq!(
            world
                .get::<FangyuanPlayerPosition>(entity)
                .unwrap()
                .translation,
            interpolated.translation
        );
    }

    #[test]
    fn changed_server_entity_id_replaces_stale_root() {
        let mut world = World::new();
        let mut registry = registry(&mut world, "session-a", "local");
        let first = register(&mut world, &mut registry, registration("remote", 10, 1)).unwrap();
        let MainWorldPlayerRegistrationResult::Created(stale) = first else {
            panic!()
        };
        let replaced = register(&mut world, &mut registry, registration("remote", 11, 2)).unwrap();
        let MainWorldPlayerRegistrationResult::Replaced {
            stale: old,
            current,
        } = replaced
        else {
            panic!()
        };
        assert_eq!(old, stale);
        assert!(world.get_entity(stale).is_err());
        assert_eq!(registry.get("remote"), Some(current));
    }

    #[test]
    fn local_ticket_character_gets_camera_target_but_remote_does_not() {
        let mut world = World::new();
        let mut registry = registry(&mut world, "session-a", "ticket-character");
        let local = register(
            &mut world,
            &mut registry,
            registration("ticket-character", 1, 1),
        )
        .unwrap();
        let remote = register(
            &mut world,
            &mut registry,
            registration("account-player-id", 2, 1),
        )
        .unwrap();
        let MainWorldPlayerRegistrationResult::Created(local) = local else {
            panic!()
        };
        let MainWorldPlayerRegistrationResult::Created(remote) = remote else {
            panic!()
        };
        let target = world.get::<SceneCameraTarget>(local).unwrap();
        assert!(target.has_tag(SCENE_CAMERA_LOCAL_PLAYER_TARGET_TAG));
        assert!(target.is_session(registry.session_id()));
        assert!(world.get::<SceneCameraTarget>(remote).is_none());
        assert_eq!(
            world.get::<MainWorldPlayer>(remote).unwrap().ownership,
            MainWorldPlayerOwnership::Remote
        );
    }

    #[test]
    fn registry_rejects_invalid_or_stale_input_without_mutating_session() {
        let mut world = World::new();
        let mut registry = registry(&mut world, "session-a", "local");
        let mut empty = registration("", 1, 1);
        assert_eq!(
            register(&mut world, &mut registry, empty.clone()),
            Err(MainWorldPlayerRegistrationError::EmptyCharacterId)
        );
        empty.character_id = "remote".to_owned();
        empty.server_scene_id = 99;
        assert_eq!(
            register(&mut world, &mut registry, empty.clone()),
            Err(MainWorldPlayerRegistrationError::UnexpectedScene { actual: 99 })
        );
        empty.server_scene_id = MAIN_WORLD_SERVER_SCENE_ID;
        empty.generation = 2;
        assert_eq!(
            register(&mut world, &mut registry, empty.clone()),
            Err(MainWorldPlayerRegistrationError::StaleGeneration {
                expected: 3,
                actual: 2
            })
        );
        empty.generation = 3;
        empty.transform.translation.x = f32::NAN;
        assert_eq!(
            register(&mut world, &mut registry, empty),
            Err(MainWorldPlayerRegistrationError::NonFiniteTransform)
        );
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn registry_rejects_older_frames_and_clear_only_despawns_its_session() {
        let mut world = World::new();
        let unrelated = world.spawn_empty().id();
        let mut registry = registry(&mut world, "session-a", "local");
        let created = register(&mut world, &mut registry, registration("remote", 1, 5)).unwrap();
        let MainWorldPlayerRegistrationResult::Created(entity) = created else {
            panic!()
        };
        assert_eq!(
            register(&mut world, &mut registry, registration("remote", 1, 4)),
            Err(MainWorldPlayerRegistrationError::StaleFrame {
                current: 5,
                actual: 4
            })
        );
        registry.clear(&mut world.commands());
        world.flush();
        assert!(world.get_entity(entity).is_err());
        assert!(world.get_entity(unrelated).is_ok());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn snapshot_applies_all_characters_incrementally_and_full_sync_removes_missing() {
        let mut world = World::new();
        let root = world.spawn_empty().id();
        let mut registry = MainWorldPlayerRegistry::new("session-a", 3, "local", root);
        let push = pb::MovementSnapshotPush {
            room_id: super::super::main_world_contract::MAIN_WORLD_PUBLIC_ROOM_ID.to_owned(),
            frame_id: 1,
            entities: vec![
                pb::EntityTransform {
                    entity_id: 1,
                    character_id: "local".into(),
                    scene_id: 1,
                    x: 2001.0,
                    y: 2001.0,
                    ..Default::default()
                },
                pb::EntityTransform {
                    entity_id: 2,
                    character_id: "remote".into(),
                    scene_id: 1,
                    x: 2002.0,
                    y: 2002.0,
                    ..Default::default()
                },
            ],
            full_sync: true,
            ..Default::default()
        };
        apply_main_world_snapshot(&mut registry, &mut world.commands(), &push, true, true).unwrap();
        world.flush();
        assert_eq!(registry.len(), 2);
        let incremental = pb::MovementSnapshotPush {
            frame_id: 2,
            full_sync: false,
            entities: vec![pb::EntityTransform {
                entity_id: 1,
                character_id: "local".into(),
                scene_id: 1,
                x: 2003.0,
                y: 2003.0,
                ..Default::default()
            }],
            ..push.clone()
        };
        apply_main_world_snapshot(
            &mut registry,
            &mut world.commands(),
            &incremental,
            true,
            true,
        )
        .unwrap();
        world.flush();
        assert_eq!(registry.len(), 2);
        let full = pb::MovementSnapshotPush {
            frame_id: 3,
            full_sync: true,
            entities: vec![pb::EntityTransform {
                entity_id: 1,
                character_id: "local".into(),
                scene_id: 1,
                x: 2004.0,
                y: 2004.0,
                ..Default::default()
            }],
            ..push
        };
        apply_main_world_snapshot(&mut registry, &mut world.commands(), &full, true, true).unwrap();
        world.flush();
        assert_eq!(registry.len(), 1);
        assert!(registry.get("remote").is_none());
    }

    #[test]
    fn complete_room_snapshot_removes_missing_remote_player_without_full_sync() {
        let mut world = World::new();
        let root = world.spawn_empty().id();
        let mut registry = MainWorldPlayerRegistry::new("session-a", 3, "local", root);
        let initial = pb::MovementSnapshotPush {
            room_id: super::super::main_world_contract::MAIN_WORLD_PUBLIC_ROOM_ID.to_owned(),
            frame_id: 1,
            entities: vec![
                pb::EntityTransform {
                    entity_id: 1,
                    character_id: "local".into(),
                    scene_id: 1,
                    x: 2001.0,
                    y: 2001.0,
                    ..Default::default()
                },
                pb::EntityTransform {
                    entity_id: 2,
                    character_id: "remote".into(),
                    scene_id: 1,
                    x: 2002.0,
                    y: 2002.0,
                    ..Default::default()
                },
            ],
            full_sync: true,
            ..Default::default()
        };
        apply_main_world_snapshot(&mut registry, &mut world.commands(), &initial, true, true)
            .unwrap();
        world.flush();
        assert_eq!(registry.len(), 2);

        let room_snapshot = pb::MovementSnapshotPush {
            frame_id: 2,
            full_sync: false,
            reason: super::super::main_world_contract::MAIN_WORLD_ROOM_SNAPSHOT_REASON.to_owned(),
            entities: vec![pb::EntityTransform {
                entity_id: 1,
                character_id: "local".into(),
                scene_id: 1,
                x: 2003.0,
                y: 2003.0,
                ..Default::default()
            }],
            ..initial
        };
        apply_main_world_snapshot(
            &mut registry,
            &mut world.commands(),
            &room_snapshot,
            true,
            true,
        )
        .unwrap();
        world.flush();

        assert_eq!(registry.len(), 1);
        assert!(registry.get("local").is_some());
        assert!(registry.get("remote").is_none());
    }

    #[test]
    fn targeted_full_snapshot_does_not_remove_players_outside_its_entity_subset() {
        let mut world = World::new();
        let root = world.spawn_empty().id();
        let mut registry = MainWorldPlayerRegistry::new("session-a", 3, "local", root);
        let initial = pb::MovementSnapshotPush {
            room_id: super::super::main_world_contract::MAIN_WORLD_PUBLIC_ROOM_ID.to_owned(),
            frame_id: 1,
            full_sync: true,
            entities: vec![
                pb::EntityTransform {
                    entity_id: 1,
                    character_id: "local".into(),
                    scene_id: MAIN_WORLD_SERVER_SCENE_ID,
                    x: 2001.0,
                    y: 2001.0,
                    ..Default::default()
                },
                pb::EntityTransform {
                    entity_id: 2,
                    character_id: "remote".into(),
                    scene_id: MAIN_WORLD_SERVER_SCENE_ID,
                    x: 2002.0,
                    y: 2002.0,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        apply_main_world_snapshot(&mut registry, &mut world.commands(), &initial, true, true)
            .unwrap();
        world.flush();

        let targeted = pb::MovementSnapshotPush {
            frame_id: 2,
            entities: vec![pb::EntityTransform {
                entity_id: 1,
                character_id: "local".into(),
                scene_id: MAIN_WORLD_SERVER_SCENE_ID,
                x: 2003.0,
                y: 2003.0,
                ..Default::default()
            }],
            target_character_ids: vec!["local".into()],
            correction_kind: pb::MovementCorrectionKind::Strong as i32,
            ..initial
        };
        apply_main_world_snapshot(&mut registry, &mut world.commands(), &targeted, true, true)
            .unwrap();
        world.flush();

        assert_eq!(registry.len(), 2);
        assert!(registry.get("remote").is_some());
    }

    #[test]
    fn snapshot_before_ready_is_rejected_without_spawning() {
        let mut world = World::new();
        let root = world.spawn_empty().id();
        let mut registry = MainWorldPlayerRegistry::new("session-a", 3, "local", root);
        let push = pb::MovementSnapshotPush {
            room_id: super::super::main_world_contract::MAIN_WORLD_PUBLIC_ROOM_ID.into(),
            frame_id: 1,
            ..Default::default()
        };
        assert_eq!(
            apply_main_world_snapshot(&mut registry, &mut world.commands(), &push, false, false),
            Err(MainWorldPlayerSnapshotError::NotReady)
        );
        world.flush();
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn invalid_snapshot_is_atomic_and_does_not_partially_spawn() {
        let mut world = World::new();
        let root = world.spawn_empty().id();
        let mut registry = MainWorldPlayerRegistry::new("session-a", 3, "local", root);
        let push = pb::MovementSnapshotPush {
            room_id: super::super::main_world_contract::MAIN_WORLD_PUBLIC_ROOM_ID.into(),
            frame_id: 1,
            entities: vec![
                pb::EntityTransform {
                    entity_id: 1,
                    character_id: "valid".into(),
                    scene_id: 1,
                    x: 2001.0,
                    y: 2001.0,
                    ..Default::default()
                },
                pb::EntityTransform {
                    entity_id: 2,
                    character_id: "invalid".into(),
                    scene_id: 99,
                    x: 2002.0,
                    y: 2002.0,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(
            apply_main_world_snapshot(&mut registry, &mut world.commands(), &push, true, true),
            Err(MainWorldPlayerSnapshotError::WrongScene { actual: 99 })
        );
        world.flush();
        assert_eq!(registry.len(), 0);
    }

    fn runtime_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, FangyuanPlayerRuntimePlugin))
            .add_message::<MyServerEvent>()
            .init_resource::<MainWorldEntryState>()
            .add_plugins(MainWorldPlayersPlugin);
        app
    }

    /// Builds an authority snapshot from concise centred-Bevy test positions.
    /// Production snapshots are always already in server coordinates.
    fn snapshot(
        frame: u32,
        full_sync: bool,
        entities: &[(&str, u64, f32, f32)],
    ) -> pb::MovementSnapshotPush {
        pb::MovementSnapshotPush {
            room_id: super::super::main_world_contract::MAIN_WORLD_PUBLIC_ROOM_ID.into(),
            frame_id: frame,
            full_sync,
            entities: entities
                .iter()
                .map(|(character, id, x, y)| pb::EntityTransform {
                    entity_id: *id,
                    character_id: (*character).into(),
                    scene_id: MAIN_WORLD_SERVER_SCENE_ID,
                    x: *x
                        + super::super::main_world_contract::MAIN_WORLD_WORLD_CENTRE_OFFSET_METRES,
                    y: *y
                        + super::super::main_world_contract::MAIN_WORLD_WORLD_CENTRE_OFFSET_METRES,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }

    fn player_count(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<MainWorldPlayer>>()
            .iter(app.world())
            .count()
    }

    fn activate_runtime_app(app: &mut App) {
        app.world_mut().spawn(SceneRuntimeRoot::new("session-1"));
        let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
        entry.generation = 1;
        entry.phase = super::super::main_world_entry::MainWorldEntryPhase::Active;
        entry.character_id = Some("local".into());
        entry.scene_session_id = Some(SceneSessionId::from("session-1"));
        entry.scene_ready = true;
        entry.room_ready_acknowledged = true;
    }

    #[test]
    fn plugin_merges_same_frame_envelopes_with_deterministic_correction_priority() {
        let mut app = runtime_app();
        activate_runtime_app(&mut app);
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                1,
                true,
                &[("local", 1, 1.0, 1.0), ("remote", 2, 2.0, 2.0)],
            )));
        app.update();

        let mut strong = snapshot(2, true, &[("local", 1, 8.0, 8.0)]);
        strong.correction_kind = pb::MovementCorrectionKind::Strong as i32;
        strong.target_character_ids = vec!["local".into()];
        let mut incremental = snapshot(2, false, &[("new-remote", 3, 4.0, 4.0)]);
        incremental.correction_kind = pb::MovementCorrectionKind::Incremental as i32;
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(strong));
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(incremental));
        app.update();

        let runtime = app.world().resource::<MainWorldPlayerRuntime>();
        let registry = runtime.registry.as_ref().unwrap();
        let local = registry.get("local").unwrap();
        assert!(registry.get("remote").is_some());
        assert!(registry.get("new-remote").is_some());
        assert_eq!(runtime.last_applied_frame, Some(2));
        assert_eq!(runtime.last_applied_envelope_count, 2);
        assert_eq!(
            app.world().get::<Transform>(local).unwrap().translation,
            Vec3::new(8.0, 0.0, 8.0)
        );
    }

    #[test]
    fn cache_keeps_same_frame_same_metadata_envelopes_with_different_entities() {
        let mut runtime = MainWorldPlayerRuntime::default();
        cache_main_world_player_snapshot(
            &mut runtime,
            MainWorldSnapshotEvent {
                epoch: 1,
                source: MainWorldSnapshotSource::Movement,
                complete_room_entities: false,
                push: snapshot(4, false, &[("local", 1, 1.0, 1.0)]),
            },
            1,
        );
        cache_main_world_player_snapshot(
            &mut runtime,
            MainWorldSnapshotEvent {
                epoch: 1,
                source: MainWorldSnapshotSource::Movement,
                complete_room_entities: false,
                push: snapshot(4, false, &[("remote", 2, 2.0, 2.0)]),
            },
            1,
        );

        assert_eq!(runtime.cached_snapshots[&4].envelopes.len(), 2);
        let merged = runtime.cached_snapshots[&4].merge();
        assert_eq!(merged.push.entities.len(), 2);
    }

    #[test]
    fn plugin_ignores_snapshot_targeted_to_another_recipient() {
        let mut app = runtime_app();
        activate_runtime_app(&mut app);
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                1,
                true,
                &[("local", 1, 1.0, 1.0), ("remote", 2, 2.0, 2.0)],
            )));
        app.update();

        let mut not_for_local = snapshot(2, true, &[("intruder", 3, 9.0, 9.0)]);
        not_for_local.target_character_ids = vec!["someone-else".into()];
        not_for_local.correction_kind = pb::MovementCorrectionKind::Recovery as i32;
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(not_for_local));
        app.update();

        let runtime = app.world().resource::<MainWorldPlayerRuntime>();
        assert_eq!(runtime.last_applied_frame, Some(1));
        assert_eq!(runtime.registry.as_ref().unwrap().len(), 2);
        assert!(runtime.registry.as_ref().unwrap().get("intruder").is_none());
    }

    #[test]
    fn plugin_replays_earlier_complete_roster_before_later_incremental_after_ready() {
        let mut app = runtime_app();
        app.world_mut().spawn(SceneRuntimeRoot::new("session-1"));
        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.generation = 1;
            entry.phase = super::super::main_world_entry::MainWorldEntryPhase::WaitingSceneReady;
            entry.character_id = Some("local".into());
            entry.scene_session_id = Some(SceneSessionId::from("session-1"));
        }
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                10,
                true,
                &[("local", 1, 1.0, 1.0), ("remote", 2, 2.0, 2.0)],
            )));
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                11,
                false,
                &[("local", 1, 3.0, 3.0)],
            )));
        app.update();
        assert_eq!(player_count(&mut app), 0);

        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.phase = super::super::main_world_entry::MainWorldEntryPhase::Active;
            entry.scene_ready = true;
            entry.room_ready_acknowledged = true;
        }
        app.update();

        let runtime = app.world().resource::<MainWorldPlayerRuntime>();
        let registry = runtime.registry.as_ref().unwrap();
        assert!(registry.get("remote").is_some());
        assert_eq!(runtime.last_applied_frame, Some(11));
        assert_eq!(
            app.world()
                .get::<Transform>(registry.get("local").unwrap())
                .unwrap()
                .translation,
            Vec3::new(3.0, 0.0, 3.0)
        );
    }

    fn activate_entry_in_coordinator(mut entry: ResMut<MainWorldEntryState>) {
        entry.phase = super::super::main_world_entry::MainWorldEntryPhase::Active;
        entry.scene_ready = true;
        entry.room_ready_acknowledged = true;
    }

    #[test]
    fn plugin_observes_entry_state_after_coordinator_in_the_same_update() {
        let mut app = runtime_app();
        app.add_systems(
            Update,
            activate_entry_in_coordinator
                .in_set(super::super::main_world_entry::MainWorldEntryUpdateSet::Coordinator),
        );
        app.world_mut().spawn(SceneRuntimeRoot::new("session-1"));
        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.generation = 1;
            entry.phase = super::super::main_world_entry::MainWorldEntryPhase::WaitingSceneReady;
            entry.character_id = Some("local".into());
            entry.scene_session_id = Some(SceneSessionId::from("session-1"));
        }
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                1,
                true,
                &[("local", 1, 1.0, 1.0)],
            )));

        app.update();

        assert_eq!(player_count(&mut app), 1);
    }

    #[test]
    fn plugin_caches_early_snapshot_then_handles_repeat_incremental_full_and_stale() {
        let mut app = runtime_app();
        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.generation = 1;
            entry.phase = super::super::main_world_entry::MainWorldEntryPhase::WaitingSceneReady;
            entry.character_id = Some("local".into());
            entry.scene_session_id = Some(SceneSessionId::from("session-1"));
        }
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                5,
                true,
                &[("local", 1, 1.0, 1.0), ("remote", 2, 2.0, 2.0)],
            )));
        app.update();
        assert_eq!(player_count(&mut app), 0);

        let root = app
            .world_mut()
            .spawn(SceneRuntimeRoot::new("session-1"))
            .id();
        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.scene_ready = true;
            entry.room_ready_acknowledged = true;
            entry.phase = super::super::main_world_entry::MainWorldEntryPhase::Active;
        }
        app.update();
        assert_eq!(player_count(&mut app), 2);
        let visual_count = app
            .world_mut()
            .query_filtered::<Entity, With<FangyuanPlayerPrimitiveVisual>>()
            .iter(app.world())
            .count();
        assert_eq!(visual_count, 4);

        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                5,
                true,
                &[("local", 1, 9.0, 9.0), ("remote", 2, 9.0, 9.0)],
            )));
        app.update();
        assert_eq!(player_count(&mut app), 2);
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<FangyuanPlayerPrimitiveVisual>>()
                .iter(app.world())
                .count(),
            4
        );

        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                6,
                false,
                &[("local", 1, 3.0, 3.0), ("new", 3, 4.0, 4.0)],
            )));
        app.update();
        assert_eq!(player_count(&mut app), 3);
        let local = app
            .world()
            .resource::<MainWorldPlayerRuntime>()
            .registry
            .as_ref()
            .unwrap()
            .get("local")
            .unwrap();
        assert_eq!(
            app.world().get::<Transform>(local).unwrap().translation,
            Vec3::new(3.0, 0.0, 3.0)
        );

        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                4,
                false,
                &[("local", 1, 1.0, 1.0)],
            )));
        app.update();
        assert_eq!(
            app.world().get::<Transform>(local).unwrap().translation,
            Vec3::new(3.0, 0.0, 3.0)
        );

        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                7,
                true,
                &[("local", 1, 5.0, 5.0)],
            )));
        app.update();
        assert_eq!(player_count(&mut app), 1);
        assert!(app.world().get_entity(root).is_ok());
    }

    #[test]
    fn plugin_generation_session_switch_clears_old_players_and_cached_snapshot() {
        let mut app = runtime_app();
        let old_root = app
            .world_mut()
            .spawn(SceneRuntimeRoot::new("session-1"))
            .id();
        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.generation = 1;
            entry.phase = super::super::main_world_entry::MainWorldEntryPhase::Active;
            entry.character_id = Some("local".into());
            entry.scene_session_id = Some(SceneSessionId::from("session-1"));
            entry.scene_ready = true;
            entry.room_ready_acknowledged = true;
        }
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                8,
                true,
                &[("local", 1, 2.0, 2.0)],
            )));
        app.update();
        assert_eq!(player_count(&mut app), 1);

        app.world_mut().spawn(SceneRuntimeRoot::new("session-2"));
        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.generation = 2;
            entry.phase = super::super::main_world_entry::MainWorldEntryPhase::Active;
            entry.scene_session_id = Some(SceneSessionId::from("session-2"));
            entry.scene_ready = true;
            entry.room_ready_acknowledged = true;
        }
        app.update();
        assert_eq!(player_count(&mut app), 0);
        assert!(app.world().get_entity(old_root).is_ok());
        assert!(
            app.world()
                .resource::<MainWorldPlayerRuntime>()
                .cached_snapshots
                .is_empty()
        );
    }

    #[test]
    fn plugin_recovery_preserves_visuals_freezes_camera_then_restores_unique_target() {
        let mut app = runtime_app();
        app.world_mut().spawn(SceneRuntimeRoot::new("session-1"));
        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.generation = 1;
            entry.phase = super::super::main_world_entry::MainWorldEntryPhase::Active;
            entry.character_id = Some("local".into());
            entry.scene_session_id = Some(SceneSessionId::from("session-1"));
            entry.scene_ready = true;
            entry.room_ready_acknowledged = true;
        }
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                10,
                true,
                &[("local", 1, 2.0, 2.0), ("remote", 2, 3.0, 3.0)],
            )));
        app.update();
        let local = app
            .world()
            .resource::<MainWorldPlayerRuntime>()
            .registry
            .as_ref()
            .unwrap()
            .get("local")
            .unwrap();
        assert!(app.world().get::<SceneCameraTarget>(local).is_some());

        app.world_mut().resource_mut::<MainWorldEntryState>().phase =
            super::super::main_world_entry::MainWorldEntryPhase::Recovering;
        app.update();
        assert_eq!(player_count(&mut app), 2);
        assert!(app.world().get::<SceneCameraTarget>(local).is_none());

        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.phase = super::super::main_world_entry::MainWorldEntryPhase::WaitingSceneReady;
            entry.room_ready_acknowledged = false;
        }
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                1,
                true,
                &[("local", 1, 4.0, 4.0)],
            )));
        app.update();
        assert_eq!(player_count(&mut app), 2);

        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.phase = super::super::main_world_entry::MainWorldEntryPhase::Active;
            entry.room_ready_acknowledged = true;
        }
        app.update();
        assert_eq!(player_count(&mut app), 1);
        assert!(app.world().get::<SceneCameraTarget>(local).is_some());
    }

    fn offline_push(character_id: &str, offline: bool) -> MyServerEvent {
        MyServerEvent::RoomMemberOfflinePush(pb::RoomMemberOfflinePush {
            room_id: super::super::main_world_contract::MAIN_WORLD_PUBLIC_ROOM_ID.into(),
            character_id: character_id.into(),
            offline,
        })
    }

    #[test]
    fn plugin_complete_snapshot_presence_does_not_override_offline_notice() {
        let mut app = runtime_app();
        app.world_mut().spawn(SceneRuntimeRoot::new("session-1"));
        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.generation = 1;
            entry.phase = super::super::main_world_entry::MainWorldEntryPhase::Active;
            entry.character_id = Some("local".into());
            entry.scene_session_id = Some(SceneSessionId::from("session-1"));
            entry.scene_ready = true;
            entry.room_ready_acknowledged = true;
        }
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                1,
                true,
                &[("local", 1, 2.0, 2.0), ("remote", 2, 3.0, 3.0)],
            )));
        app.update();
        app.world_mut().write_message(offline_push("remote", true));
        app.update();
        assert_eq!(player_count(&mut app), 2);
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                2,
                true,
                &[("local", 1, 4.0, 4.0), ("remote", 2, 5.0, 5.0)],
            )));
        app.update();
        assert!(
            app.world()
                .resource::<MainWorldPlayerRuntime>()
                .remote_offline_deadlines
                .contains_key("remote")
        );
        assert_eq!(player_count(&mut app), 2);
    }

    #[test]
    fn plugin_incremental_presence_does_not_clear_remote_offline_grace() {
        let mut app = runtime_app();
        activate_runtime_app(&mut app);
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                1,
                true,
                &[("local", 1, 2.0, 2.0), ("remote", 2, 3.0, 3.0)],
            )));
        app.update();
        app.world_mut().write_message(offline_push("remote", true));
        app.update();

        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                2,
                false,
                &[("remote", 2, 4.0, 4.0)],
            )));
        app.update();

        let runtime = app.world().resource::<MainWorldPlayerRuntime>();
        assert!(runtime.remote_offline_deadlines.contains_key("remote"));
        assert!(runtime.registry.as_ref().unwrap().get("remote").is_some());
    }

    #[test]
    fn plugin_online_notice_clears_remote_offline_grace() {
        let mut app = runtime_app();
        app.world_mut().spawn(SceneRuntimeRoot::new("session-1"));
        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.generation = 1;
            entry.phase = super::super::main_world_entry::MainWorldEntryPhase::Active;
            entry.character_id = Some("local".into());
            entry.scene_session_id = Some(SceneSessionId::from("session-1"));
            entry.scene_ready = true;
            entry.room_ready_acknowledged = true;
        }
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                1,
                true,
                &[("local", 1, 2.0, 2.0), ("remote", 2, 3.0, 3.0)],
            )));
        app.update();
        app.world_mut().write_message(offline_push("remote", true));
        app.update();
        app.world_mut().write_message(offline_push("remote", false));
        app.update();
        assert!(
            app.world()
                .resource::<MainWorldPlayerRuntime>()
                .remote_offline_deadlines
                .is_empty()
        );
        assert_eq!(player_count(&mut app), 2);
    }

    #[test]
    fn plugin_offline_remote_is_removed_when_complete_snapshot_omits_it() {
        let mut app = runtime_app();
        app.world_mut().spawn(SceneRuntimeRoot::new("session-1"));
        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.generation = 1;
            entry.phase = super::super::main_world_entry::MainWorldEntryPhase::Active;
            entry.character_id = Some("local".into());
            entry.scene_session_id = Some(SceneSessionId::from("session-1"));
            entry.scene_ready = true;
            entry.room_ready_acknowledged = true;
        }
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                1,
                true,
                &[("local", 1, 2.0, 2.0), ("remote", 2, 3.0, 3.0)],
            )));
        app.update();
        app.world_mut().write_message(offline_push("remote", true));
        app.update();
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                2,
                true,
                &[("local", 1, 4.0, 4.0)],
            )));
        app.update();
        assert_eq!(player_count(&mut app), 1);
        assert!(
            app.world()
                .resource::<MainWorldPlayerRuntime>()
                .remote_offline_deadlines
                .contains_key("remote")
        );
    }

    #[test]
    fn expired_remote_offline_grace_removes_only_remote_player() {
        let mut world = World::new();
        let mut registry = registry(&mut world, "session-a", "local");
        register(&mut world, &mut registry, registration("local", 1, 1)).unwrap();
        register(&mut world, &mut registry, registration("remote", 2, 1)).unwrap();
        let mut deadlines = HashMap::from([("remote".to_owned(), MAIN_WORLD_REMOTE_OFFLINE_GRACE)]);
        remove_expired_remote_offline_players(
            &mut registry,
            &mut world.commands(),
            &mut deadlines,
            MAIN_WORLD_REMOTE_OFFLINE_GRACE,
        );
        world.flush();
        assert_eq!(registry.get("remote"), None);
        assert!(registry.get("local").is_some());
        assert!(deadlines.contains_key("remote"));
    }

    #[test]
    fn plugin_scene_exit_or_lobby_teardown_clears_players_visuals_and_targets() {
        let mut app = runtime_app();
        app.world_mut().spawn(SceneRuntimeRoot::new("session-1"));
        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.generation = 1;
            entry.phase = super::super::main_world_entry::MainWorldEntryPhase::Active;
            entry.character_id = Some("local".into());
            entry.scene_session_id = Some(SceneSessionId::from("session-1"));
            entry.scene_ready = true;
            entry.room_ready_acknowledged = true;
        }
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                1,
                true,
                &[("local", 1, 2.0, 2.0)],
            )));
        app.update();
        app.update();
        assert_eq!(player_count(&mut app), 1);
        app.world_mut().resource_mut::<MainWorldEntryState>().phase =
            super::super::main_world_entry::MainWorldEntryPhase::LobbyIdle;
        app.update();
        assert_eq!(player_count(&mut app), 0);
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<FangyuanPlayerPrimitiveVisual>>()
                .iter(app.world())
                .count(),
            0
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<SceneCameraTarget>>()
                .iter(app.world())
                .count(),
            0
        );
    }

    #[test]
    fn plugin_same_generation_reentry_after_teardown_uses_fresh_snapshot_and_target() {
        let mut app = runtime_app();
        app.world_mut().spawn(SceneRuntimeRoot::new("session-1"));
        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.generation = 1;
            entry.phase = super::super::main_world_entry::MainWorldEntryPhase::Active;
            entry.character_id = Some("local".into());
            entry.scene_session_id = Some(SceneSessionId::from("session-1"));
            entry.scene_ready = true;
            entry.room_ready_acknowledged = true;
        }
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                9,
                true,
                &[("local", 1, 2.0, 2.0)],
            )));
        app.update();
        app.world_mut().resource_mut::<MainWorldEntryState>().phase =
            super::super::main_world_entry::MainWorldEntryPhase::LobbyIdle;
        app.update();
        assert_eq!(player_count(&mut app), 0);
        assert!(
            !app.world()
                .resource::<MainWorldPlayerRuntime>()
                .was_recovering
        );

        app.world_mut().spawn(SceneRuntimeRoot::new("session-2"));
        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.phase = super::super::main_world_entry::MainWorldEntryPhase::Active;
            entry.scene_session_id = Some(SceneSessionId::from("session-2"));
            entry.scene_ready = true;
            entry.room_ready_acknowledged = true;
        }
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(snapshot(
                1,
                true,
                &[("local", 2, 4.0, 4.0)],
            )));
        app.update();
        assert_eq!(player_count(&mut app), 1);
        let local = app
            .world()
            .resource::<MainWorldPlayerRuntime>()
            .registry
            .as_ref()
            .unwrap()
            .get("local")
            .unwrap();
        assert_eq!(
            app.world().get::<Transform>(local).unwrap().translation,
            Vec3::new(4.0, 0.0, 4.0)
        );
        assert!(
            app.world()
                .get::<SceneCameraTarget>(local)
                .unwrap()
                .has_tag(SCENE_CAMERA_LOCAL_PLAYER_TARGET_TAG)
        );
    }

    #[test]
    fn minimal_blueprint_scales_to_quarter_meter_footprint_and_grounded_height() {
        let blueprint = load_fangyuan_minimal_player_blueprint().unwrap();
        let scale = main_world_player_uniform_scale(blueprint.bounds);
        let transform =
            main_world_player_root_transform(Transform::from_xyz(4.0, 0.0, -2.0), blueprint.bounds);
        assert_eq!(
            blueprint.bounds,
            FangyuanBlueprintBounds::new(2.0, 2.0, 3.0)
        );
        assert_eq!(scale, 0.125);
        assert_eq!(
            Vec3::new(
                blueprint.bounds.width,
                blueprint.bounds.height,
                blueprint.bounds.depth,
            ) * scale,
            Vec3::new(0.25, 0.375, 0.25)
        );
        assert_eq!(transform.translation, Vec3::new(4.0, 0.0, -2.0));
        assert_eq!(transform.scale, Vec3::splat(0.125));
    }

    #[test]
    fn multiple_players_parent_to_runtime_root_and_share_visual_assets() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, FangyuanPlayerRuntimePlugin));
        let root = app.world_mut().spawn_empty().id();
        let mut registry = MainWorldPlayerRegistry::new("session-a", 3, "local", root);
        register(app.world_mut(), &mut registry, registration("local", 1, 1)).unwrap();
        register(app.world_mut(), &mut registry, registration("remote", 2, 1)).unwrap();
        app.update();

        let mut visuals = app.world_mut().query::<(
            &ChildOf,
            &FangyuanPlayerPrimitiveVisual,
            &Mesh3d,
            &MeshMaterial3d<StandardMaterial>,
        )>();
        let records: Vec<_> = visuals
            .iter(app.world())
            .map(|(parent, visual, mesh, material)| {
                (
                    parent.parent(),
                    visual.kind,
                    mesh.0.clone(),
                    material.0.clone(),
                )
            })
            .collect();
        assert_eq!(records.len(), FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT * 2);
        for character in ["local", "remote"] {
            let player = registry.get(character).unwrap();
            assert_eq!(app.world().get::<ChildOf>(player).unwrap().parent(), root);
            assert_eq!(
                records.iter().filter(|record| record.0 == player).count(),
                2
            );
        }
        for kind in [
            crate::framework::fangyuan::FangyuanPrimitiveKind::Cube,
            crate::framework::fangyuan::FangyuanPrimitiveKind::Sphere,
        ] {
            let matching: Vec<_> = records.iter().filter(|record| record.1 == kind).collect();
            assert_eq!(matching.len(), 2);
            assert_eq!(matching[0].2, matching[1].2);
            assert_eq!(matching[0].3, matching[1].3);
        }
    }

    #[test]
    fn player_generation_measurement_reports_stable_entity_and_asset_deltas() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, FangyuanPlayerRuntimePlugin));
        let root = app.world_mut().spawn_empty().id();
        let mut registry = MainWorldPlayerRegistry::new("measure", 3, "local", root);
        let base_meshes = app.world().resource::<Assets<Mesh>>().len();
        let base_materials = app.world().resource::<Assets<StandardMaterial>>().len();

        let single_started = std::time::Instant::now();
        register(app.world_mut(), &mut registry, registration("local", 1, 1)).unwrap();
        app.update();
        let single_ms = single_started.elapsed().as_secs_f64() * 1000.0;
        let single_meshes = app.world().resource::<Assets<Mesh>>().len();
        let single_materials = app.world().resource::<Assets<StandardMaterial>>().len();
        assert_eq!(registry.len(), 1);
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<FangyuanPlayerPrimitiveVisual>>()
                .iter(app.world())
                .count(),
            2
        );
        assert_eq!(single_meshes - base_meshes, 2);
        assert_eq!(single_materials - base_materials, 2);

        let double_started = std::time::Instant::now();
        register(app.world_mut(), &mut registry, registration("remote", 2, 1)).unwrap();
        app.update();
        let double_ms = double_started.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(registry.len(), 2);
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<FangyuanPlayerPrimitiveVisual>>()
                .iter(app.world())
                .count(),
            4
        );
        assert_eq!(
            app.world().resource::<Assets<Mesh>>().len() - single_meshes,
            0
        );
        assert_eq!(
            app.world().resource::<Assets<StandardMaterial>>().len() - single_materials,
            0
        );
        println!(
            "main-world-player-measurement empty_to_single: roots=+1 visuals=+2 meshes=+2 materials=+2 elapsed_ms={single_ms:.3}; single_to_double: roots=+1 visuals=+2 meshes=+0 materials=+0 elapsed_ms={double_ms:.3}"
        );
    }
}
