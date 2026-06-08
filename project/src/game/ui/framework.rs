use bevy::prelude::*;

use super::{
    input::UiInputPlugin, layer::UiLayerPlugin, router::UiRouterPlugin, screen::UiScreenPlugin,
    theme::UiThemePlugin, widgets::UiWidgetsPlugin,
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
