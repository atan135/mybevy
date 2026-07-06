use std::collections::VecDeque;

use bevy::prelude::*;
use sim_core::{BuffId, EntityId, SimEvent, SimWorld, SkillId};

use crate::framework::scene::prelude::{SceneOwned, SceneRuntimeRoot, SceneSessionId};

use super::{replay::LockstepSimReplayState, state::LockstepSimSceneState};

const COMBAT_EVENT_DISPLAY_HISTORY_LIMIT: usize = 128;
const COMBAT_EVENT_VISUAL_LIMIT: usize = 128;
const DEFAULT_SKILL_PREVIEW_RADIUS: f32 = 1.5;

#[derive(Clone, Debug, Default, Resource, PartialEq)]
pub(in crate::game::features::lockstep_sim) struct LockstepSimCombatEventState {
    pub(in crate::game::features::lockstep_sim) last_consumed_frame: Option<u32>,
    pub(in crate::game::features::lockstep_sim) entries: Vec<LockstepSimCombatEventEntry>,
    pub(in crate::game::features::lockstep_sim) next_visual_entry_index: usize,
    pub(in crate::game::features::lockstep_sim) visual_entities: VecDeque<Entity>,
}

impl LockstepSimCombatEventState {
    pub(in crate::game::features::lockstep_sim) fn clear(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::game::features::lockstep_sim) struct LockstepSimCombatEventEntry {
    pub(in crate::game::features::lockstep_sim) frame: u32,
    pub(in crate::game::features::lockstep_sim) sequence: u32,
    pub(in crate::game::features::lockstep_sim) order_index: usize,
    pub(in crate::game::features::lockstep_sim) kind: LockstepSimCombatEventKind,
    pub(in crate::game::features::lockstep_sim) label: String,
    pub(in crate::game::features::lockstep_sim) source_entity: EntityId,
    pub(in crate::game::features::lockstep_sim) target_entity: Option<EntityId>,
    pub(in crate::game::features::lockstep_sim) skill_id: Option<SkillId>,
    pub(in crate::game::features::lockstep_sim) buff_id: Option<BuffId>,
    pub(in crate::game::features::lockstep_sim) value: i32,
    pub(in crate::game::features::lockstep_sim) source_position: Option<Vec3>,
    pub(in crate::game::features::lockstep_sim) target_position: Option<Vec3>,
    pub(in crate::game::features::lockstep_sim) display_position: Option<Vec3>,
    pub(in crate::game::features::lockstep_sim) range_preview: Option<LockstepSimSkillRangePreview>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) enum LockstepSimCombatEventKind {
    SkillCast,
    Hit,
    DamageNumber,
    HealNumber,
    BuffApplied,
    BuffTick,
    BuffExpired,
    DeathState,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::game::features::lockstep_sim) struct LockstepSimSkillRangePreview {
    pub(in crate::game::features::lockstep_sim) center: Vec3,
    pub(in crate::game::features::lockstep_sim) radius: f32,
    pub(in crate::game::features::lockstep_sim) target: Option<Vec3>,
}

#[derive(Clone, Debug, Component, PartialEq)]
pub(in crate::game::features::lockstep_sim) struct LockstepSimCombatEventVisual {
    pub(in crate::game::features::lockstep_sim) session_id: SceneSessionId,
    pub(in crate::game::features::lockstep_sim) kind: LockstepSimCombatEventKind,
    pub(in crate::game::features::lockstep_sim) label: String,
    pub(in crate::game::features::lockstep_sim) frame: u32,
    pub(in crate::game::features::lockstep_sim) sequence: u32,
    pub(in crate::game::features::lockstep_sim) order_index: usize,
    pub(in crate::game::features::lockstep_sim) source_entity: EntityId,
    pub(in crate::game::features::lockstep_sim) target_entity: Option<EntityId>,
    pub(in crate::game::features::lockstep_sim) value: i32,
    pub(in crate::game::features::lockstep_sim) range_preview: bool,
}

pub(in crate::game::features::lockstep_sim) fn despawn_lockstep_sim_combat_event_visuals(
    commands: &mut Commands,
    state: &mut LockstepSimCombatEventState,
    entities: impl IntoIterator<Item = Entity>,
) {
    for entity in entities {
        commands.entity(entity).despawn();
    }
    state.clear();
}

pub(in crate::game::features::lockstep_sim) fn update_lockstep_sim_combat_events(
    scene_state: Res<LockstepSimSceneState>,
    replay_state: Res<LockstepSimReplayState>,
    mut event_state: ResMut<LockstepSimCombatEventState>,
) {
    if !scene_state.active {
        event_state.clear();
        return;
    }

    let Some(world) = replay_state.world.as_ref() else {
        event_state.clear();
        return;
    };

    let mut last_consumed_frame = event_state.last_consumed_frame;
    let mut entries = replay_state
        .event_history
        .iter()
        .filter(|frame_events| {
            event_state
                .last_consumed_frame
                .is_none_or(|frame| frame_events.frame > frame)
        })
        .flat_map(|frame_events| {
            last_consumed_frame = Some(frame_events.frame);
            frame_events
                .events
                .iter()
                .enumerate()
                .flat_map(|(order_index, event)| entries_from_sim_event(world, event, order_index))
        })
        .collect::<Vec<_>>();
    if entries.is_empty() {
        return;
    }

    event_state.entries.append(&mut entries);
    if event_state.entries.len() > COMBAT_EVENT_DISPLAY_HISTORY_LIMIT {
        let remove_count = event_state
            .entries
            .len()
            .saturating_sub(COMBAT_EVENT_DISPLAY_HISTORY_LIMIT);
        event_state.entries.drain(0..remove_count);
        event_state.next_visual_entry_index = event_state
            .next_visual_entry_index
            .saturating_sub(remove_count);
    }
    event_state.last_consumed_frame = last_consumed_frame;
}

pub(in crate::game::features::lockstep_sim) fn sync_lockstep_sim_combat_event_visuals(
    mut commands: Commands,
    scene_state: Res<LockstepSimSceneState>,
    mut event_state: ResMut<LockstepSimCombatEventState>,
    runtime_roots: Query<(Entity, &SceneRuntimeRoot)>,
    visuals: Query<(Entity, &LockstepSimCombatEventVisual)>,
) {
    if !scene_state.active {
        let entities = visuals.iter().map(|(entity, _)| entity).collect::<Vec<_>>();
        despawn_lockstep_sim_combat_event_visuals(&mut commands, &mut event_state, entities);
        return;
    }

    let Some(session_id) = scene_state.session_id.as_ref() else {
        return;
    };
    let runtime_root = find_runtime_root_entity(session_id, runtime_roots.iter());
    for (entity, visual) in &visuals {
        if &visual.session_id != session_id {
            commands.entity(entity).despawn();
        }
    }

    let start_index = event_state
        .next_visual_entry_index
        .min(event_state.entries.len());
    let entries_to_spawn = event_state.entries[start_index..].to_vec();
    event_state.next_visual_entry_index = event_state.entries.len();
    for entry in entries_to_spawn {
        let entity = spawn_lockstep_sim_combat_event_visual(
            &mut commands,
            session_id,
            runtime_root,
            &entry,
            false,
        );
        event_state.visual_entities.push_back(entity);

        if let Some(preview) = entry.range_preview {
            let entity = spawn_lockstep_sim_combat_event_visual(
                &mut commands,
                session_id,
                runtime_root,
                &entry,
                true,
            );
            event_state.visual_entities.push_back(entity);
            commands
                .entity(entity)
                .insert(
                    Transform::from_translation(preview.center).with_scale(Vec3::new(
                        preview.radius.max(0.1) * 2.0,
                        preview.radius.max(0.1) * 2.0,
                        1.0,
                    )),
                );
        }
    }

    while event_state.visual_entities.len() > COMBAT_EVENT_VISUAL_LIMIT {
        if let Some(entity) = event_state.visual_entities.pop_front() {
            commands.entity(entity).despawn();
        }
    }
}

fn entries_from_sim_event(
    world: &SimWorld,
    event: &SimEvent,
    order_index: usize,
) -> Vec<LockstepSimCombatEventEntry> {
    match event {
        SimEvent::SkillCast {
            frame,
            source_entity,
            target_entity,
            skill_id,
            value,
            sequence,
        } => vec![LockstepSimCombatEventEntry {
            frame: frame.raw(),
            sequence: *sequence,
            order_index,
            kind: LockstepSimCombatEventKind::SkillCast,
            label: format!("Skill {}", skill_id.raw()),
            source_entity: *source_entity,
            target_entity: *target_entity,
            skill_id: Some(*skill_id),
            buff_id: None,
            value: *value,
            source_position: entity_position(world, *source_entity),
            target_position: target_entity.and_then(|entity_id| entity_position(world, entity_id)),
            display_position: entity_position(world, *source_entity),
            range_preview: skill_range_preview(world, *source_entity, *target_entity, *skill_id),
        }],
        SimEvent::DamageApplied {
            frame,
            source_entity,
            target_entity,
            skill_id,
            buff_id,
            value,
            sequence,
        } => vec![
            LockstepSimCombatEventEntry {
                frame: frame.raw(),
                sequence: *sequence,
                order_index,
                kind: LockstepSimCombatEventKind::Hit,
                label: "Hit".to_string(),
                source_entity: *source_entity,
                target_entity: Some(*target_entity),
                skill_id: *skill_id,
                buff_id: *buff_id,
                value: *value,
                source_position: entity_position(world, *source_entity),
                target_position: entity_position(world, *target_entity),
                display_position: entity_position(world, *target_entity),
                range_preview: None,
            },
            LockstepSimCombatEventEntry {
                frame: frame.raw(),
                sequence: *sequence,
                order_index,
                kind: LockstepSimCombatEventKind::DamageNumber,
                label: format!("-{value}"),
                source_entity: *source_entity,
                target_entity: Some(*target_entity),
                skill_id: *skill_id,
                buff_id: *buff_id,
                value: *value,
                source_position: entity_position(world, *source_entity),
                target_position: entity_position(world, *target_entity),
                display_position: floating_number_position(world, *target_entity),
                range_preview: None,
            },
        ],
        SimEvent::HealApplied {
            frame,
            source_entity,
            target_entity,
            skill_id,
            buff_id,
            value,
            sequence,
        } => vec![LockstepSimCombatEventEntry {
            frame: frame.raw(),
            sequence: *sequence,
            order_index,
            kind: LockstepSimCombatEventKind::HealNumber,
            label: format!("+{value}"),
            source_entity: *source_entity,
            target_entity: Some(*target_entity),
            skill_id: *skill_id,
            buff_id: *buff_id,
            value: *value,
            source_position: entity_position(world, *source_entity),
            target_position: entity_position(world, *target_entity),
            display_position: floating_number_position(world, *target_entity),
            range_preview: None,
        }],
        SimEvent::BuffApplied {
            frame,
            source_entity,
            target_entity,
            buff_id,
            value,
            sequence,
        } => vec![buff_entry(
            world,
            frame.raw(),
            *sequence,
            order_index,
            LockstepSimCombatEventKind::BuffApplied,
            format!("Buff {} applied", buff_id.raw()),
            *source_entity,
            *target_entity,
            *buff_id,
            *value,
        )],
        SimEvent::BuffTick {
            frame,
            source_entity,
            target_entity,
            buff_id,
            value,
            sequence,
        } => vec![buff_entry(
            world,
            frame.raw(),
            *sequence,
            order_index,
            LockstepSimCombatEventKind::BuffTick,
            format!("Buff {} tick", buff_id.raw()),
            *source_entity,
            *target_entity,
            *buff_id,
            *value,
        )],
        SimEvent::BuffExpired {
            frame,
            source_entity,
            target_entity,
            buff_id,
            value,
            sequence,
        } => vec![buff_entry(
            world,
            frame.raw(),
            *sequence,
            order_index,
            LockstepSimCombatEventKind::BuffExpired,
            format!("Buff {} expired", buff_id.raw()),
            *source_entity,
            *target_entity,
            *buff_id,
            *value,
        )],
        SimEvent::EntityDied {
            frame,
            source_entity,
            target_entity,
            skill_id,
            buff_id,
            value,
            sequence,
        } => vec![LockstepSimCombatEventEntry {
            frame: frame.raw(),
            sequence: *sequence,
            order_index,
            kind: LockstepSimCombatEventKind::DeathState,
            label: "Dead".to_string(),
            source_entity: *source_entity,
            target_entity: Some(*target_entity),
            skill_id: *skill_id,
            buff_id: *buff_id,
            value: *value,
            source_position: entity_position(world, *source_entity),
            target_position: entity_position(world, *target_entity),
            display_position: entity_position(world, *target_entity),
            range_preview: None,
        }],
    }
}

fn buff_entry(
    world: &SimWorld,
    frame: u32,
    sequence: u32,
    order_index: usize,
    kind: LockstepSimCombatEventKind,
    label: String,
    source_entity: EntityId,
    target_entity: EntityId,
    buff_id: BuffId,
    value: i32,
) -> LockstepSimCombatEventEntry {
    LockstepSimCombatEventEntry {
        frame,
        sequence,
        order_index,
        kind,
        label,
        source_entity,
        target_entity: Some(target_entity),
        skill_id: None,
        buff_id: Some(buff_id),
        value,
        source_position: entity_position(world, source_entity),
        target_position: entity_position(world, target_entity),
        display_position: entity_position(world, target_entity),
        range_preview: None,
    }
}

fn skill_range_preview(
    world: &SimWorld,
    source_entity: EntityId,
    target_entity: Option<EntityId>,
    _skill_id: SkillId,
) -> Option<LockstepSimSkillRangePreview> {
    let center = entity_position(world, source_entity)?;
    Some(LockstepSimSkillRangePreview {
        center,
        radius: DEFAULT_SKILL_PREVIEW_RADIUS,
        target: target_entity.and_then(|entity_id| entity_position(world, entity_id)),
    })
}

fn entity_position(world: &SimWorld, entity_id: EntityId) -> Option<Vec3> {
    let entity = world.entity(entity_id)?;
    Some(Vec3::new(
        entity.transform.pos.x.to_f32_for_render(),
        0.0,
        entity.transform.pos.y.to_f32_for_render(),
    ))
}

fn floating_number_position(world: &SimWorld, entity_id: EntityId) -> Option<Vec3> {
    entity_position(world, entity_id).map(|position| position + Vec3::Y * 1.6)
}

fn spawn_lockstep_sim_combat_event_visual(
    commands: &mut Commands,
    session_id: &SceneSessionId,
    runtime_root: Option<Entity>,
    entry: &LockstepSimCombatEventEntry,
    range_preview: bool,
) -> Entity {
    let translation = if range_preview {
        entry
            .range_preview
            .map(|preview| preview.center)
            .or(entry.display_position)
    } else {
        entry.display_position.or(entry.source_position)
    }
    .unwrap_or(Vec3::ZERO);
    let transform = Transform::from_translation(translation)
        .with_scale(combat_event_visual_scale(entry.kind, range_preview));
    let entity = commands
        .spawn((
            Sprite::from_color(
                combat_event_visual_color(entry.kind, range_preview),
                Vec2::ONE,
            ),
            transform,
            SceneOwned::new(session_id.clone()),
            LockstepSimCombatEventVisual {
                session_id: session_id.clone(),
                kind: entry.kind,
                label: entry.label.clone(),
                frame: entry.frame,
                sequence: entry.sequence,
                order_index: entry.order_index,
                source_entity: entry.source_entity,
                target_entity: entry.target_entity,
                value: entry.value,
                range_preview,
            },
            Name::new(if range_preview {
                format!(
                    "LockstepSimCombatRange({}:{})",
                    entry.frame, entry.order_index
                )
            } else {
                format!(
                    "LockstepSimCombatEvent({}:{:?}:{})",
                    entry.frame, entry.kind, entry.order_index
                )
            }),
        ))
        .id();
    if let Some(runtime_root) = runtime_root {
        commands.entity(runtime_root).add_child(entity);
    }
    entity
}

fn combat_event_visual_color(kind: LockstepSimCombatEventKind, range_preview: bool) -> Color {
    if range_preview {
        return Color::srgba(0.25, 0.65, 1.0, 0.25);
    }

    match kind {
        LockstepSimCombatEventKind::SkillCast => Color::srgb(0.25, 0.65, 1.0),
        LockstepSimCombatEventKind::Hit => Color::srgb(1.0, 0.9, 0.2),
        LockstepSimCombatEventKind::DamageNumber => Color::srgb(1.0, 0.2, 0.15),
        LockstepSimCombatEventKind::HealNumber => Color::srgb(0.2, 1.0, 0.35),
        LockstepSimCombatEventKind::BuffApplied => Color::srgb(0.75, 0.45, 1.0),
        LockstepSimCombatEventKind::BuffTick => Color::srgb(1.0, 0.55, 0.15),
        LockstepSimCombatEventKind::BuffExpired => Color::srgb(0.55, 0.55, 0.6),
        LockstepSimCombatEventKind::DeathState => Color::srgb(0.05, 0.05, 0.05),
    }
}

fn combat_event_visual_scale(kind: LockstepSimCombatEventKind, range_preview: bool) -> Vec3 {
    if range_preview {
        return Vec3::splat(DEFAULT_SKILL_PREVIEW_RADIUS * 2.0);
    }

    match kind {
        LockstepSimCombatEventKind::DamageNumber | LockstepSimCombatEventKind::HealNumber => {
            Vec3::new(0.45, 0.45, 1.0)
        }
        LockstepSimCombatEventKind::DeathState => Vec3::new(0.8, 0.8, 1.0),
        LockstepSimCombatEventKind::SkillCast => Vec3::new(0.65, 0.65, 1.0),
        _ => Vec3::new(0.35, 0.35, 1.0),
    }
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
    use crate::framework::scene::prelude::{SceneId, spawn_scene_root, spawn_scene_runtime_root};
    use crate::game::scenes::LOCKSTEP_SIM_ARENA_SCENE_ID;
    use sim_core::{
        BuffSlot, CombatState, EntityKind, Fp, FrameId, MovementState, QuantizedDir, SimEntity,
        SimRngState, SimTransform, TeamId, Vec2Fp,
    };

    #[test]
    fn combat_event_entries_cover_all_sim_event_kinds_in_order() {
        let world = world_fixture();
        let events = vec![
            SimEvent::SkillCast {
                frame: FrameId::new(9),
                source_entity: EntityId::new(100),
                target_entity: Some(EntityId::new(200)),
                skill_id: SkillId::new(10),
                value: 0,
                sequence: 0,
            },
            SimEvent::BuffApplied {
                frame: FrameId::new(9),
                source_entity: EntityId::new(100),
                target_entity: EntityId::new(200),
                buff_id: BuffId::new(20),
                value: 1,
                sequence: 1,
            },
            SimEvent::BuffTick {
                frame: FrameId::new(9),
                source_entity: EntityId::new(100),
                target_entity: EntityId::new(200),
                buff_id: BuffId::new(20),
                value: 2,
                sequence: 2,
            },
            SimEvent::DamageApplied {
                frame: FrameId::new(9),
                source_entity: EntityId::new(100),
                target_entity: EntityId::new(200),
                skill_id: Some(SkillId::new(10)),
                buff_id: None,
                value: 15,
                sequence: 3,
            },
            SimEvent::HealApplied {
                frame: FrameId::new(9),
                source_entity: EntityId::new(100),
                target_entity: EntityId::new(100),
                skill_id: Some(SkillId::new(11)),
                buff_id: None,
                value: 5,
                sequence: 4,
            },
            SimEvent::BuffExpired {
                frame: FrameId::new(9),
                source_entity: EntityId::new(100),
                target_entity: EntityId::new(200),
                buff_id: BuffId::new(20),
                value: 0,
                sequence: 5,
            },
            SimEvent::EntityDied {
                frame: FrameId::new(9),
                source_entity: EntityId::new(100),
                target_entity: EntityId::new(200),
                skill_id: Some(SkillId::new(10)),
                buff_id: None,
                value: 15,
                sequence: 6,
            },
        ];

        let entries = events
            .iter()
            .enumerate()
            .flat_map(|(index, event)| entries_from_sim_event(&world, event, index))
            .collect::<Vec<_>>();

        assert_eq!(
            entries.iter().map(|entry| entry.kind).collect::<Vec<_>>(),
            vec![
                LockstepSimCombatEventKind::SkillCast,
                LockstepSimCombatEventKind::BuffApplied,
                LockstepSimCombatEventKind::BuffTick,
                LockstepSimCombatEventKind::Hit,
                LockstepSimCombatEventKind::DamageNumber,
                LockstepSimCombatEventKind::HealNumber,
                LockstepSimCombatEventKind::BuffExpired,
                LockstepSimCombatEventKind::DeathState,
            ]
        );
        assert_eq!(entries[0].label, "Skill 10");
        assert_eq!(entries[4].label, "-15");
        assert_eq!(entries[5].label, "+5");
        assert_eq!(entries[7].label, "Dead");
        assert!(entries[0].range_preview.is_some());
        assert_eq!(entries[4].display_position, Some(Vec3::new(3.0, 1.6, 4.0)));
        assert_eq!(
            entries
                .iter()
                .map(|entry| (entry.frame, entry.sequence, entry.order_index))
                .collect::<Vec<_>>(),
            vec![
                (9, 0, 0),
                (9, 1, 1),
                (9, 2, 2),
                (9, 3, 3),
                (9, 3, 3),
                (9, 4, 4),
                (9, 5, 5),
                (9, 6, 6),
            ]
        );
    }

    #[test]
    fn combat_event_system_consumes_latest_frame_once_and_clears_when_inactive() {
        let mut app = App::new();
        app.init_resource::<LockstepSimSceneState>()
            .init_resource::<LockstepSimReplayState>()
            .init_resource::<LockstepSimCombatEventState>()
            .add_systems(Update, update_lockstep_sim_combat_events);
        app.world_mut()
            .resource_mut::<LockstepSimSceneState>()
            .active = true;
        {
            let mut replay = app.world_mut().resource_mut::<LockstepSimReplayState>();
            replay.world = Some(world_fixture());
            replay
                .event_history
                .push_back(super::super::replay::LockstepSimFrameEvents {
                    frame: 9,
                    events: vec![SimEvent::DamageApplied {
                        frame: FrameId::new(9),
                        source_entity: EntityId::new(100),
                        target_entity: EntityId::new(200),
                        skill_id: Some(SkillId::new(10)),
                        buff_id: None,
                        value: 15,
                        sequence: 3,
                    }],
                });
        }

        app.update();
        app.update();

        let state = app.world().resource::<LockstepSimCombatEventState>();
        assert_eq!(state.last_consumed_frame, Some(9));
        assert_eq!(state.entries.len(), 2);

        app.world_mut()
            .resource_mut::<LockstepSimSceneState>()
            .active = false;
        app.update();

        assert_eq!(
            *app.world().resource::<LockstepSimCombatEventState>(),
            LockstepSimCombatEventState::default()
        );
    }

    #[test]
    fn combat_event_system_consumes_backlog_frames_in_stable_order_without_duplicates() {
        let mut app = App::new();
        app.init_resource::<LockstepSimSceneState>()
            .init_resource::<LockstepSimReplayState>()
            .init_resource::<LockstepSimCombatEventState>()
            .add_systems(Update, update_lockstep_sim_combat_events);
        app.world_mut()
            .resource_mut::<LockstepSimSceneState>()
            .active = true;
        {
            let mut replay = app.world_mut().resource_mut::<LockstepSimReplayState>();
            replay.world = Some(world_fixture());
            replay.event_history.push_back(frame_events(
                8,
                vec![SimEvent::SkillCast {
                    frame: FrameId::new(8),
                    source_entity: EntityId::new(100),
                    target_entity: Some(EntityId::new(200)),
                    skill_id: SkillId::new(10),
                    value: 0,
                    sequence: 0,
                }],
            ));
            replay.event_history.push_back(frame_events(
                9,
                vec![SimEvent::DamageApplied {
                    frame: FrameId::new(9),
                    source_entity: EntityId::new(100),
                    target_entity: EntityId::new(200),
                    skill_id: Some(SkillId::new(10)),
                    buff_id: None,
                    value: 15,
                    sequence: 1,
                }],
            ));
        }

        app.update();
        app.update();

        let state = app.world().resource::<LockstepSimCombatEventState>();
        assert_eq!(state.last_consumed_frame, Some(9));
        assert_eq!(
            state
                .entries
                .iter()
                .map(|entry| (entry.frame, entry.kind, entry.order_index))
                .collect::<Vec<_>>(),
            vec![
                (8, LockstepSimCombatEventKind::SkillCast, 0),
                (9, LockstepSimCombatEventKind::Hit, 0),
                (9, LockstepSimCombatEventKind::DamageNumber, 0),
            ]
        );
    }

    #[test]
    fn combat_event_fixture_melee_hit_displays_skill_hit_and_damage_number() {
        let world = world_fixture();
        let entries = fixture_entries(
            &world,
            vec![
                SimEvent::SkillCast {
                    frame: FrameId::new(10),
                    source_entity: EntityId::new(100),
                    target_entity: Some(EntityId::new(200)),
                    skill_id: SkillId::new(1),
                    value: 0,
                    sequence: 0,
                },
                SimEvent::DamageApplied {
                    frame: FrameId::new(10),
                    source_entity: EntityId::new(100),
                    target_entity: EntityId::new(200),
                    skill_id: Some(SkillId::new(1)),
                    buff_id: None,
                    value: 12,
                    sequence: 1,
                },
            ],
        );

        assert_eq!(
            entries.iter().map(|entry| entry.kind).collect::<Vec<_>>(),
            vec![
                LockstepSimCombatEventKind::SkillCast,
                LockstepSimCombatEventKind::Hit,
                LockstepSimCombatEventKind::DamageNumber,
            ]
        );
        assert_eq!(entries[0].label, "Skill 1");
        assert_eq!(entries[2].label, "-12");
        assert_eq!(entries[2].target_entity, Some(EntityId::new(200)));
        assert!(entries[0].range_preview.is_some());
    }

    #[test]
    fn combat_event_fixture_aoe_hit_displays_multiple_targets_in_stable_order() {
        let world = world_fixture_with_extra_target();
        let entries = fixture_entries(
            &world,
            vec![
                SimEvent::SkillCast {
                    frame: FrameId::new(11),
                    source_entity: EntityId::new(100),
                    target_entity: None,
                    skill_id: SkillId::new(2),
                    value: 0,
                    sequence: 0,
                },
                SimEvent::DamageApplied {
                    frame: FrameId::new(11),
                    source_entity: EntityId::new(100),
                    target_entity: EntityId::new(200),
                    skill_id: Some(SkillId::new(2)),
                    buff_id: None,
                    value: 8,
                    sequence: 1,
                },
                SimEvent::DamageApplied {
                    frame: FrameId::new(11),
                    source_entity: EntityId::new(100),
                    target_entity: EntityId::new(201),
                    skill_id: Some(SkillId::new(2)),
                    buff_id: None,
                    value: 9,
                    sequence: 2,
                },
            ],
        );

        assert_eq!(
            entries
                .iter()
                .map(|entry| (entry.kind, entry.target_entity, entry.label.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (LockstepSimCombatEventKind::SkillCast, None, "Skill 2"),
                (
                    LockstepSimCombatEventKind::Hit,
                    Some(EntityId::new(200)),
                    "Hit",
                ),
                (
                    LockstepSimCombatEventKind::DamageNumber,
                    Some(EntityId::new(200)),
                    "-8",
                ),
                (
                    LockstepSimCombatEventKind::Hit,
                    Some(EntityId::new(201)),
                    "Hit",
                ),
                (
                    LockstepSimCombatEventKind::DamageNumber,
                    Some(EntityId::new(201)),
                    "-9",
                ),
            ]
        );
        assert_eq!(
            entries
                .iter()
                .map(|entry| (entry.sequence, entry.order_index))
                .collect::<Vec<_>>(),
            vec![(0, 0), (1, 1), (1, 1), (2, 2), (2, 2)]
        );
    }

    #[test]
    fn combat_event_fixture_buff_dot_displays_apply_tick_damage_and_expire() {
        let world = world_fixture();
        let entries = fixture_entries(
            &world,
            vec![
                SimEvent::BuffApplied {
                    frame: FrameId::new(12),
                    source_entity: EntityId::new(100),
                    target_entity: EntityId::new(200),
                    buff_id: BuffId::new(20),
                    value: 1,
                    sequence: 0,
                },
                SimEvent::BuffTick {
                    frame: FrameId::new(12),
                    source_entity: EntityId::new(100),
                    target_entity: EntityId::new(200),
                    buff_id: BuffId::new(20),
                    value: 1,
                    sequence: 1,
                },
                SimEvent::DamageApplied {
                    frame: FrameId::new(12),
                    source_entity: EntityId::new(100),
                    target_entity: EntityId::new(200),
                    skill_id: None,
                    buff_id: Some(BuffId::new(20)),
                    value: 3,
                    sequence: 2,
                },
                SimEvent::BuffExpired {
                    frame: FrameId::new(12),
                    source_entity: EntityId::new(100),
                    target_entity: EntityId::new(200),
                    buff_id: BuffId::new(20),
                    value: 0,
                    sequence: 3,
                },
            ],
        );

        assert_eq!(
            entries.iter().map(|entry| entry.kind).collect::<Vec<_>>(),
            vec![
                LockstepSimCombatEventKind::BuffApplied,
                LockstepSimCombatEventKind::BuffTick,
                LockstepSimCombatEventKind::Hit,
                LockstepSimCombatEventKind::DamageNumber,
                LockstepSimCombatEventKind::BuffExpired,
            ]
        );
        assert_eq!(entries[0].label, "Buff 20 applied");
        assert_eq!(entries[1].label, "Buff 20 tick");
        assert_eq!(entries[3].label, "-3");
        assert_eq!(entries[4].label, "Buff 20 expired");
        assert!(
            entries
                .iter()
                .all(|entry| entry.target_entity == Some(EntityId::new(200)))
        );
    }

    #[test]
    fn combat_event_display_does_not_mutate_replay_world() {
        let mut state = LockstepSimCombatEventState::default();
        let replay = LockstepSimReplayState {
            world: Some(world_fixture()),
            event_history: [super::super::replay::LockstepSimFrameEvents {
                frame: 9,
                events: vec![SimEvent::HealApplied {
                    frame: FrameId::new(9),
                    source_entity: EntityId::new(100),
                    target_entity: EntityId::new(100),
                    skill_id: Some(SkillId::new(11)),
                    buff_id: None,
                    value: 5,
                    sequence: 4,
                }],
            }]
            .into(),
            ..Default::default()
        };
        let before = replay.world.clone();
        let mut app = App::new();
        app.init_resource::<LockstepSimSceneState>()
            .insert_resource(replay)
            .insert_resource(state.clone())
            .add_systems(Update, update_lockstep_sim_combat_events);
        app.world_mut()
            .resource_mut::<LockstepSimSceneState>()
            .active = true;

        app.update();
        state = app
            .world()
            .resource::<LockstepSimCombatEventState>()
            .clone();

        assert_eq!(
            app.world().resource::<LockstepSimReplayState>().world,
            before
        );
        assert_eq!(state.entries.len(), 1);
    }

    #[test]
    fn combat_event_visuals_spawn_markers_with_labels_and_range_preview() {
        let mut app = visual_test_app();
        activate_scene_with_runtime_root(&mut app);
        {
            let mut state = app
                .world_mut()
                .resource_mut::<LockstepSimCombatEventState>();
            state.entries = fixture_entries(
                &world_fixture(),
                vec![
                    SimEvent::SkillCast {
                        frame: FrameId::new(20),
                        source_entity: EntityId::new(100),
                        target_entity: Some(EntityId::new(200)),
                        skill_id: SkillId::new(7),
                        value: 0,
                        sequence: 0,
                    },
                    SimEvent::DamageApplied {
                        frame: FrameId::new(20),
                        source_entity: EntityId::new(100),
                        target_entity: EntityId::new(200),
                        skill_id: Some(SkillId::new(7)),
                        buff_id: None,
                        value: 17,
                        sequence: 1,
                    },
                    SimEvent::HealApplied {
                        frame: FrameId::new(20),
                        source_entity: EntityId::new(100),
                        target_entity: EntityId::new(100),
                        skill_id: Some(SkillId::new(8)),
                        buff_id: None,
                        value: 6,
                        sequence: 2,
                    },
                    SimEvent::EntityDied {
                        frame: FrameId::new(20),
                        source_entity: EntityId::new(100),
                        target_entity: EntityId::new(200),
                        skill_id: Some(SkillId::new(7)),
                        buff_id: None,
                        value: 17,
                        sequence: 3,
                    },
                ],
            );
        }

        app.update();

        let visuals = visual_components(&mut app);
        assert_eq!(visuals.len(), 6);
        assert!(visuals.iter().any(|(_, visual, _, _)| {
            visual.kind == LockstepSimCombatEventKind::SkillCast
                && visual.label == "Skill 7"
                && !visual.range_preview
        }));
        assert!(visuals.iter().any(|(_, visual, _, _)| {
            visual.kind == LockstepSimCombatEventKind::SkillCast && visual.range_preview
        }));
        assert!(visuals.iter().any(|(_, visual, _, _)| {
            visual.kind == LockstepSimCombatEventKind::DamageNumber && visual.label == "-17"
        }));
        assert!(visuals.iter().any(|(_, visual, _, _)| {
            visual.kind == LockstepSimCombatEventKind::HealNumber && visual.label == "+6"
        }));
        assert!(visuals.iter().any(|(_, visual, _, _)| {
            visual.kind == LockstepSimCombatEventKind::DeathState && visual.label == "Dead"
        }));

        let range = visuals
            .iter()
            .find(|(_, visual, _, _)| visual.range_preview)
            .unwrap();
        assert_eq!(range.2.translation, Vec3::new(1.0, 0.0, 2.0));
        assert_eq!(
            range.2.scale,
            Vec3::new(
                DEFAULT_SKILL_PREVIEW_RADIUS * 2.0,
                DEFAULT_SKILL_PREVIEW_RADIUS * 2.0,
                1.0
            )
        );
        assert!(range.3.custom_size.is_some());
    }

    #[test]
    fn combat_event_visuals_clear_when_scene_inactive() {
        let mut app = visual_test_app();
        activate_scene_with_runtime_root(&mut app);
        app.world_mut()
            .resource_mut::<LockstepSimCombatEventState>()
            .entries = fixture_entries(
            &world_fixture(),
            vec![SimEvent::DamageApplied {
                frame: FrameId::new(21),
                source_entity: EntityId::new(100),
                target_entity: EntityId::new(200),
                skill_id: Some(SkillId::new(1)),
                buff_id: None,
                value: 5,
                sequence: 0,
            }],
        );
        app.update();
        assert_eq!(visual_components(&mut app).len(), 2);

        app.world_mut()
            .resource_mut::<LockstepSimSceneState>()
            .active = false;
        app.update();

        assert!(visual_components(&mut app).is_empty());
        assert_eq!(
            *app.world().resource::<LockstepSimCombatEventState>(),
            LockstepSimCombatEventState::default()
        );
    }

    #[test]
    fn combat_event_visuals_keep_same_frame_order_fields_stable() {
        let mut app = visual_test_app();
        activate_scene_with_runtime_root(&mut app);
        app.world_mut()
            .resource_mut::<LockstepSimCombatEventState>()
            .entries = fixture_entries(
            &world_fixture_with_extra_target(),
            vec![
                SimEvent::DamageApplied {
                    frame: FrameId::new(22),
                    source_entity: EntityId::new(100),
                    target_entity: EntityId::new(200),
                    skill_id: Some(SkillId::new(2)),
                    buff_id: None,
                    value: 8,
                    sequence: 1,
                },
                SimEvent::DamageApplied {
                    frame: FrameId::new(22),
                    source_entity: EntityId::new(100),
                    target_entity: EntityId::new(201),
                    skill_id: Some(SkillId::new(2)),
                    buff_id: None,
                    value: 9,
                    sequence: 2,
                },
            ],
        );

        app.update();

        let mut visuals = visual_components(&mut app)
            .into_iter()
            .map(|(_, visual, _, _)| visual)
            .collect::<Vec<_>>();
        visuals.sort_by_key(|visual| {
            (
                visual.frame,
                visual.order_index,
                visual.sequence,
                visual.label.clone(),
            )
        });
        assert_eq!(
            visuals
                .iter()
                .map(|visual| {
                    (
                        visual.frame,
                        visual.order_index,
                        visual.sequence,
                        visual.kind,
                        visual.target_entity,
                        visual.label.as_str(),
                    )
                })
                .collect::<Vec<_>>(),
            vec![
                (
                    22,
                    0,
                    1,
                    LockstepSimCombatEventKind::DamageNumber,
                    Some(EntityId::new(200)),
                    "-8",
                ),
                (
                    22,
                    0,
                    1,
                    LockstepSimCombatEventKind::Hit,
                    Some(EntityId::new(200)),
                    "Hit",
                ),
                (
                    22,
                    1,
                    2,
                    LockstepSimCombatEventKind::DamageNumber,
                    Some(EntityId::new(201)),
                    "-9",
                ),
                (
                    22,
                    1,
                    2,
                    LockstepSimCombatEventKind::Hit,
                    Some(EntityId::new(201)),
                    "Hit",
                ),
            ]
        );
    }

    fn world_fixture() -> SimWorld {
        SimWorld::with_rng(
            FrameId::new(9),
            SimRngState {
                seed: 1,
                counter: 2,
            },
            vec![
                entity(100, Vec2Fp::new(Fp::from_i32(1), Fp::from_i32(2))),
                entity(200, Vec2Fp::new(Fp::from_i32(3), Fp::from_i32(4))),
            ],
        )
        .unwrap()
    }

    fn visual_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(TransformPlugin)
            .init_resource::<LockstepSimSceneState>()
            .init_resource::<LockstepSimCombatEventState>()
            .add_systems(Update, sync_lockstep_sim_combat_event_visuals);
        app
    }

    fn activate_scene_with_runtime_root(app: &mut App) -> (SceneSessionId, Entity) {
        let session_id = SceneSessionId::from("lockstep-combat-session");
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

    fn visual_components(
        app: &mut App,
    ) -> Vec<(Entity, LockstepSimCombatEventVisual, Transform, Sprite)> {
        let mut query = app
            .world_mut()
            .query::<(Entity, &LockstepSimCombatEventVisual, &Transform, &Sprite)>();
        query
            .iter(app.world())
            .map(|(entity, visual, transform, sprite)| {
                (entity, visual.clone(), *transform, sprite.clone())
            })
            .collect()
    }

    fn world_fixture_with_extra_target() -> SimWorld {
        SimWorld::with_rng(
            FrameId::new(11),
            SimRngState {
                seed: 1,
                counter: 2,
            },
            vec![
                entity(100, Vec2Fp::new(Fp::from_i32(1), Fp::from_i32(2))),
                entity(200, Vec2Fp::new(Fp::from_i32(3), Fp::from_i32(4))),
                entity(201, Vec2Fp::new(Fp::from_i32(5), Fp::from_i32(6))),
            ],
        )
        .unwrap()
    }

    fn frame_events(
        frame: u32,
        events: Vec<SimEvent>,
    ) -> super::super::replay::LockstepSimFrameEvents {
        super::super::replay::LockstepSimFrameEvents { frame, events }
    }

    fn fixture_entries(
        world: &SimWorld,
        events: Vec<SimEvent>,
    ) -> Vec<LockstepSimCombatEventEntry> {
        events
            .iter()
            .enumerate()
            .flat_map(|(index, event)| entries_from_sim_event(world, event, index))
            .collect()
    }

    fn entity(id: u32, pos: Vec2Fp) -> SimEntity {
        SimEntity {
            id: EntityId::new(id),
            kind: EntityKind::Player,
            owner_character_id: None,
            team_id: TeamId::new(1),
            transform: SimTransform {
                pos,
                facing: QuantizedDir::RIGHT,
                radius: Fp::from_milli(500),
            },
            movement: MovementState::default(),
            combat: CombatState {
                buffs: vec![BuffSlot {
                    buff_id: BuffId::new(20),
                    source_entity: EntityId::new(100),
                    duration_remaining: 10,
                    interval_remaining: 1,
                    stacks: 1,
                }],
                ..Default::default()
            },
            alive: true,
        }
    }
}
