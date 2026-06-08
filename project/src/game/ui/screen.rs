use bevy::prelude::*;

pub(in crate::game) struct UiScreenPlugin;

impl Plugin for UiScreenPlugin {
    fn build(&self, _app: &mut App) {}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(in crate::game) enum UiScreenId {
    LoginPage,
    GameListPage,
    TouchRippleHud,
}

#[derive(Component)]
#[allow(dead_code)]
pub(in crate::game) struct UiScreenRoot {
    pub id: UiScreenId,
}
