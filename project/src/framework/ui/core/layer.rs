use bevy::prelude::*;

pub(crate) struct UiLayerPlugin;

impl Plugin for UiLayerPlugin {
    fn build(&self, _app: &mut App) {}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) enum UiLayer {
    Page,
    Floating,
    Modal,
    Loading,
    Toast,
    Debug,
}

#[derive(Component)]
#[allow(dead_code)]
pub(crate) struct UiLayerRoot {
    pub layer: UiLayer,
}
