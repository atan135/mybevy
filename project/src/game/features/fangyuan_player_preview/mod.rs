use bevy::{
    ecs::component::{Mutable, StorageType},
    prelude::*,
    transform::TransformSystems,
};

use crate::framework::fangyuan::{
    FANGYUAN_MINIMAL_PLAYER_BLUEPRINT_PATH, FangyuanAvatar, FangyuanPrimitiveSet,
    load_fangyuan_minimal_player_primitive_set_or_log,
};

pub(in crate::game) struct FangyuanPlayerPreviewPlugin;

impl Plugin for FangyuanPlayerPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_fangyuan_preview_player)
            .add_systems(
                PostUpdate,
                sync_fangyuan_player_transform.before(TransformSystems::Propagate),
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

impl Component for FangyuanPrimitiveSet {
    const STORAGE_TYPE: StorageType = StorageType::Table;
    type Mutability = Mutable;
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

fn sync_fangyuan_player_transform(
    mut players: Query<(&FangyuanPlayerPosition, &mut Transform), With<FangyuanPlayer>>,
) {
    for (position, mut transform) in &mut players {
        transform.translation = position.translation;
        transform.rotation = Quat::IDENTITY;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin))
            .add_plugins(FangyuanPlayerPreviewPlugin);
        app
    }

    #[test]
    fn fangyuan_preview_plugin_spawns_one_player_entity() {
        let mut app = test_app();
        app.update();

        let players = fangyuan_player_entities(&mut app);
        assert_eq!(players.len(), 1);
    }

    #[test]
    fn fangyuan_preview_player_spawn_is_idempotent() {
        let mut app = test_app();
        app.update();
        app.update();

        let players = fangyuan_player_entities(&mut app);
        assert_eq!(players.len(), 1);
    }

    #[test]
    fn fangyuan_preview_player_has_required_components() {
        let mut app = test_app();
        app.update();

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
        app.update();
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
        app.update();

        let players = fangyuan_player_entities(&mut app);
        let mut primitive_sets = app.world_mut().query::<&FangyuanPrimitiveSet>();

        assert_eq!(players.len(), 1);
        assert_eq!(primitive_sets.iter(app.world()).count(), 1);
        assert_eq!(
            primitive_sets.single(app.world()).unwrap().len(),
            FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT
        );
    }

    fn fangyuan_player_entities(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<FangyuanPlayer>>()
            .iter(app.world())
            .collect()
    }
}
