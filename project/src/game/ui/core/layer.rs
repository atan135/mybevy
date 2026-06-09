use bevy::prelude::*;

pub(in crate::game) struct UiLayerPlugin;

impl Plugin for UiLayerPlugin {
    fn build(&self, _app: &mut App) {}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(in crate::game) enum UiLayer {
    Page,
    Floating,
    Modal,
    Loading,
    Toast,
    Debug,
}

#[derive(Component)]
#[allow(dead_code)]
pub(in crate::game) struct UiLayerRoot {
    pub layer: UiLayer,
}
