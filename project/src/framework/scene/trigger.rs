use std::collections::HashSet;

use bevy::prelude::*;
use serde::{Deserialize, Deserializer};

use super::{
    id::{SceneSessionId, SceneTriggerId},
    root::SceneOwned,
    spawn::transform_from_position_rotation,
};

#[derive(Clone, Debug, Component, PartialEq)]
pub struct SceneTrigger {
    pub trigger_id: SceneTriggerId,
    pub shape: SceneTriggerShape,
    pub event: String,
    pub enabled: bool,
    pub session_id: SceneSessionId,
}

impl SceneTrigger {
    pub fn new(
        session_id: impl Into<SceneSessionId>,
        trigger_id: impl Into<SceneTriggerId>,
        shape: SceneTriggerShape,
        event: impl Into<String>,
    ) -> Self {
        Self {
            trigger_id: trigger_id.into(),
            shape,
            event: event.into(),
            enabled: true,
            session_id: session_id.into(),
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn enable(&mut self) {
        self.set_enabled(true);
    }

    pub fn disable(&mut self) {
        self.set_enabled(false);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SceneTriggerShape {
    Circle2d { radius: f32 },
    Box2d { half_extents: Vec2 },
    Box3d { half_extents: Vec3 },
}

impl SceneTriggerShape {
    pub fn contains_point(&self, trigger_transform: &GlobalTransform, point: Vec3) -> bool {
        let local_point = trigger_transform.affine().inverse().transform_point3(point);

        match self {
            Self::Circle2d { radius } => local_point.xy().length_squared() <= radius * radius,
            Self::Box2d { half_extents } => {
                local_point.x.abs() <= half_extents.x && local_point.y.abs() <= half_extents.y
            }
            Self::Box3d { half_extents } => {
                local_point.x.abs() <= half_extents.x
                    && local_point.y.abs() <= half_extents.y
                    && local_point.z.abs() <= half_extents.z
            }
        }
    }
}

#[derive(Clone, Debug, Message, PartialEq)]
pub struct SceneTriggerEvent {
    pub trigger_id: SceneTriggerId,
    pub activator: Option<Entity>,
    pub action: SceneTriggerAction,
    pub session_id: SceneSessionId,
}

impl SceneTriggerEvent {
    pub fn new(
        trigger_id: impl Into<SceneTriggerId>,
        activator: Option<Entity>,
        action: SceneTriggerAction,
        session_id: impl Into<SceneSessionId>,
    ) -> Self {
        Self {
            trigger_id: trigger_id.into(),
            activator,
            action,
            session_id: session_id.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SceneTriggerAction {
    Enter,
    Exit,
    Stay,
    Interact,
}

#[derive(Clone, Debug, Component, PartialEq, Eq)]
pub struct SceneTriggerActivator {
    pub session_id: Option<SceneSessionId>,
}

impl SceneTriggerActivator {
    pub fn any_session() -> Self {
        Self { session_id: None }
    }

    pub fn for_session(session_id: impl Into<SceneSessionId>) -> Self {
        Self {
            session_id: Some(session_id.into()),
        }
    }
}

#[derive(Clone, Debug, Component, Default, PartialEq, Eq)]
pub struct SceneTriggerContactState {
    activators_inside: HashSet<Entity>,
}

impl SceneTriggerContactState {
    pub fn activators_inside(&self) -> impl Iterator<Item = Entity> + '_ {
        self.activators_inside.iter().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.activators_inside.is_empty()
    }
}

#[derive(Clone, Debug, Component, PartialEq)]
pub struct SceneTriggerDebugShape {
    pub trigger_id: SceneTriggerId,
    pub shape: SceneTriggerShape,
    pub label: String,
    pub session_id: SceneSessionId,
}

impl SceneTriggerDebugShape {
    pub fn new(
        trigger_id: impl Into<SceneTriggerId>,
        shape: SceneTriggerShape,
        label: impl Into<String>,
        session_id: impl Into<SceneSessionId>,
    ) -> Self {
        Self {
            trigger_id: trigger_id.into(),
            shape,
            label: label.into(),
            session_id: session_id.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneTriggerDebugItem<'a> {
    pub entity: Entity,
    pub trigger_id: &'a SceneTriggerId,
    pub session_id: &'a SceneSessionId,
    pub shape: &'a SceneTriggerShape,
    pub label: &'a str,
    pub transform: &'a GlobalTransform,
    pub enabled: bool,
}

#[derive(Clone, Debug, Message, PartialEq, Eq)]
pub enum SceneTriggerCommand {
    SetEnabled {
        session_id: Option<SceneSessionId>,
        trigger_id: SceneTriggerId,
        enabled: bool,
    },
    Interact {
        session_id: Option<SceneSessionId>,
        trigger_id: SceneTriggerId,
        activator: Option<Entity>,
    },
}

impl SceneTriggerCommand {
    pub fn set_enabled(
        trigger_id: impl Into<SceneTriggerId>,
        enabled: bool,
        session_id: Option<SceneSessionId>,
    ) -> Self {
        Self::SetEnabled {
            session_id,
            trigger_id: trigger_id.into(),
            enabled,
        }
    }

    pub fn enable(
        trigger_id: impl Into<SceneTriggerId>,
        session_id: Option<SceneSessionId>,
    ) -> Self {
        Self::set_enabled(trigger_id, true, session_id)
    }

    pub fn disable(
        trigger_id: impl Into<SceneTriggerId>,
        session_id: Option<SceneSessionId>,
    ) -> Self {
        Self::set_enabled(trigger_id, false, session_id)
    }

    pub fn interact(
        trigger_id: impl Into<SceneTriggerId>,
        activator: Option<Entity>,
        session_id: Option<SceneSessionId>,
    ) -> Self {
        Self::Interact {
            session_id,
            trigger_id: trigger_id.into(),
            activator,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct SceneTriggerManifest {
    pub id: SceneTriggerId,
    pub shape: SceneTriggerShapeManifest,
    pub position: [f32; 3],
    #[serde(alias = "rotation")]
    pub rotation_degrees: [f32; 3],
    pub event: String,
}

impl Default for SceneTriggerManifest {
    fn default() -> Self {
        Self::new(
            SceneTriggerId::from(""),
            SceneTriggerShapeManifest::default(),
            "",
        )
    }
}

impl SceneTriggerManifest {
    pub fn new(
        id: impl Into<SceneTriggerId>,
        shape: SceneTriggerShapeManifest,
        event: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            shape,
            position: [0.0, 0.0, 0.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            event: event.into(),
        }
    }

    pub fn with_position(mut self, position: [f32; 3]) -> Self {
        self.position = position;
        self
    }

    pub fn with_rotation_degrees(mut self, rotation_degrees: [f32; 3]) -> Self {
        self.rotation_degrees = rotation_degrees;
        self
    }

    pub fn transform(&self) -> Transform {
        transform_from_position_rotation(self.position, self.rotation_degrees)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SceneTriggerShapeManifest {
    Circle2d { radius: f32 },
    Box2d { half_extents: [f32; 2] },
    Box3d { half_extents: [f32; 3] },
}

impl Default for SceneTriggerShapeManifest {
    fn default() -> Self {
        Self::Box2d {
            half_extents: [0.0, 0.0],
        }
    }
}

impl<'de> Deserialize<'de> for SceneTriggerShapeManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        enum Shape {
            Circle2d { radius: f32 },
            Circle { radius: f32 },
            Box2d { half_extents: [f32; 2] },
            Box3d { half_extents: [f32; 3] },
            Box { half_extents: [f32; 3] },
        }

        Ok(match Shape::deserialize(deserializer)? {
            Shape::Circle2d { radius } | Shape::Circle { radius } => Self::Circle2d { radius },
            Shape::Box2d { half_extents } => Self::Box2d { half_extents },
            Shape::Box3d { half_extents } | Shape::Box { half_extents } => {
                Self::Box3d { half_extents }
            }
        })
    }
}

impl SceneTriggerShapeManifest {
    pub fn circle2d(radius: f32) -> Self {
        Self::Circle2d { radius }
    }

    pub fn box2d(half_extents: [f32; 2]) -> Self {
        Self::Box2d { half_extents }
    }

    pub fn box3d(half_extents: [f32; 3]) -> Self {
        Self::Box3d { half_extents }
    }

    pub fn shape(&self) -> SceneTriggerShape {
        match self {
            Self::Circle2d { radius } => SceneTriggerShape::Circle2d { radius: *radius },
            Self::Box2d { half_extents } => SceneTriggerShape::Box2d {
                half_extents: Vec2::from_array(*half_extents),
            },
            Self::Box3d { half_extents } => SceneTriggerShape::Box3d {
                half_extents: Vec3::from_array(*half_extents),
            },
        }
    }
}

pub fn scene_trigger_bundle(
    session_id: impl Into<SceneSessionId>,
    manifest: &SceneTriggerManifest,
) -> impl Bundle {
    let session_id = session_id.into();
    let trigger_id = manifest.id.clone();
    let shape = manifest.shape.shape();
    let label = manifest.event.clone();
    let name = format!("SceneTrigger({trigger_id})");

    (
        SceneTrigger::new(
            session_id.clone(),
            trigger_id.clone(),
            shape.clone(),
            manifest.event.clone(),
        ),
        SceneTriggerContactState::default(),
        SceneTriggerDebugShape::new(trigger_id, shape, label, session_id.clone()),
        SceneOwned::new(session_id),
        manifest.transform(),
        GlobalTransform::default(),
        Name::new(name),
    )
}

pub fn spawn_scene_trigger(
    commands: &mut Commands,
    session_id: &SceneSessionId,
    manifest: &SceneTriggerManifest,
) -> Entity {
    commands
        .spawn(scene_trigger_bundle(session_id.clone(), manifest))
        .id()
}

pub fn spawn_scene_triggers_from_manifest(
    commands: &mut Commands,
    session_id: &SceneSessionId,
    triggers: &[SceneTriggerManifest],
) -> Vec<Entity> {
    triggers
        .iter()
        .map(|trigger| spawn_scene_trigger(commands, session_id, trigger))
        .collect()
}

pub fn process_scene_trigger_commands(
    mut trigger_commands: MessageReader<SceneTriggerCommand>,
    mut triggers: Query<(&mut SceneTrigger, &mut SceneTriggerContactState)>,
    mut trigger_events: MessageWriter<SceneTriggerEvent>,
) {
    for command in trigger_commands.read() {
        match command {
            SceneTriggerCommand::SetEnabled {
                session_id,
                trigger_id,
                enabled,
            } => {
                for (mut trigger, mut state) in &mut triggers {
                    if trigger_matches(&trigger, session_id.as_ref(), trigger_id) {
                        trigger.set_enabled(*enabled);

                        if !enabled {
                            state.activators_inside.clear();
                        }
                    }
                }
            }
            SceneTriggerCommand::Interact {
                session_id,
                trigger_id,
                activator,
            } => {
                for (trigger, _) in &mut triggers {
                    if trigger.enabled && trigger_matches(&trigger, session_id.as_ref(), trigger_id)
                    {
                        trigger_events.write(SceneTriggerEvent::new(
                            trigger.trigger_id.clone(),
                            *activator,
                            SceneTriggerAction::Interact,
                            trigger.session_id.clone(),
                        ));
                    }
                }
            }
        }
    }
}

pub fn detect_scene_triggers(
    mut triggers: Query<(
        &SceneTrigger,
        &GlobalTransform,
        &mut SceneTriggerContactState,
    )>,
    activators: Query<(
        Entity,
        &GlobalTransform,
        &SceneTriggerActivator,
        Option<&SceneOwned>,
    )>,
    mut trigger_events: MessageWriter<SceneTriggerEvent>,
) {
    for (trigger, trigger_transform, mut state) in &mut triggers {
        if !trigger.enabled {
            state.activators_inside.clear();
            continue;
        }

        let mut current_activators = HashSet::new();

        for (activator_entity, activator_transform, activator, owned) in &activators {
            if !activator_matches_trigger_session(activator, owned, &trigger.session_id) {
                continue;
            }

            if trigger
                .shape
                .contains_point(trigger_transform, activator_transform.translation())
            {
                current_activators.insert(activator_entity);

                let action = if state.activators_inside.contains(&activator_entity) {
                    SceneTriggerAction::Stay
                } else {
                    SceneTriggerAction::Enter
                };

                trigger_events.write(SceneTriggerEvent::new(
                    trigger.trigger_id.clone(),
                    Some(activator_entity),
                    action,
                    trigger.session_id.clone(),
                ));
            }
        }

        for previous_activator in state.activators_inside.difference(&current_activators) {
            trigger_events.write(SceneTriggerEvent::new(
                trigger.trigger_id.clone(),
                Some(*previous_activator),
                SceneTriggerAction::Exit,
                trigger.session_id.clone(),
            ));
        }

        state.activators_inside = current_activators;
    }
}

pub fn scene_trigger_debug_items<'world, 'state>(
    triggers: &'state Query<
        'world,
        'state,
        (
            Entity,
            &'world SceneTrigger,
            &'world SceneTriggerDebugShape,
            &'world GlobalTransform,
        ),
    >,
) -> Vec<SceneTriggerDebugItem<'state>> {
    triggers
        .iter()
        .map(
            |(entity, trigger, debug_shape, transform)| SceneTriggerDebugItem {
                entity,
                trigger_id: &trigger.trigger_id,
                session_id: &trigger.session_id,
                shape: &debug_shape.shape,
                label: &debug_shape.label,
                transform,
                enabled: trigger.enabled,
            },
        )
        .collect()
}

fn trigger_matches(
    trigger: &SceneTrigger,
    session_id: Option<&SceneSessionId>,
    trigger_id: &SceneTriggerId,
) -> bool {
    trigger.trigger_id == *trigger_id
        && session_id.is_none_or(|session_id| &trigger.session_id == session_id)
}

fn activator_matches_trigger_session(
    activator: &SceneTriggerActivator,
    owned: Option<&SceneOwned>,
    trigger_session_id: &SceneSessionId,
) -> bool {
    if let Some(session_id) = &activator.session_id {
        return session_id == trigger_session_id;
    }

    owned.is_none_or(|owned| owned.is_session(trigger_session_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_shapes_contain_expected_points() {
        let transform = GlobalTransform::from(Transform::from_xyz(10.0, 20.0, 1.0));

        assert!(
            SceneTriggerShape::Circle2d { radius: 5.0 }
                .contains_point(&transform, Vec3::new(13.0, 24.0, 100.0))
        );
        assert!(
            !SceneTriggerShape::Circle2d { radius: 5.0 }
                .contains_point(&transform, Vec3::new(16.0, 24.0, 1.0))
        );

        assert!(
            SceneTriggerShape::Box2d {
                half_extents: Vec2::new(2.0, 3.0)
            }
            .contains_point(&transform, Vec3::new(12.0, 23.0, 99.0))
        );
        assert!(
            !SceneTriggerShape::Box2d {
                half_extents: Vec2::new(2.0, 3.0)
            }
            .contains_point(&transform, Vec3::new(12.1, 23.0, 1.0))
        );

        assert!(
            SceneTriggerShape::Box3d {
                half_extents: Vec3::new(2.0, 3.0, 4.0)
            }
            .contains_point(&transform, Vec3::new(12.0, 23.0, 5.0))
        );
        assert!(
            !SceneTriggerShape::Box3d {
                half_extents: Vec3::new(2.0, 3.0, 4.0)
            }
            .contains_point(&transform, Vec3::new(12.0, 23.0, 5.1))
        );
    }

    #[test]
    fn trigger_manifest_builds_transform_and_shape() {
        let manifest = SceneTriggerManifest::new(
            "trigger",
            SceneTriggerShapeManifest::box2d([2.0, 3.0]),
            "scene.trigger",
        )
        .with_position([1.0, 2.0, 3.0]);

        assert_eq!(manifest.transform().translation, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(
            manifest.shape.shape(),
            SceneTriggerShape::Box2d {
                half_extents: Vec2::new(2.0, 3.0)
            }
        );
    }
}
