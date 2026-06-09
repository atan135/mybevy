use bevy::prelude::*;

use crate::game::ui::{
    core::{
        focus::UiFocusPlugin, input::UiInputPlugin, layer::UiLayerPlugin, panel::UiPanelPlugin,
    },
    i18n::UiI18nPlugin,
    overlays::UiRouterPlugin,
    style::UiThemePlugin,
    widgets::UiWidgetsPlugin,
};

pub(in crate::game) struct UiFrameworkPlugin;

impl Plugin for UiFrameworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            UiI18nPlugin,
            UiThemePlugin,
            UiWidgetsPlugin,
            UiLayerPlugin,
            UiRouterPlugin,
            UiPanelPlugin,
            UiInputPlugin,
            UiFocusPlugin,
        ));
    }
}
