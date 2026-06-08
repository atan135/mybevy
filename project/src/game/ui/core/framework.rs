use bevy::prelude::*;

use crate::game::ui::{
    core::{input::UiInputPlugin, layer::UiLayerPlugin, screen::UiScreenPlugin},
    overlays::UiRouterPlugin,
    style::UiThemePlugin,
    widgets::UiWidgetsPlugin,
};

pub(in crate::game) struct UiFrameworkPlugin;

impl Plugin for UiFrameworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            UiThemePlugin,
            UiWidgetsPlugin,
            UiScreenPlugin,
            UiLayerPlugin,
            UiRouterPlugin,
            UiInputPlugin,
        ));
    }
}
