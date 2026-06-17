use bevy::prelude::*;

use crate::framework::ui::{
    core::{
        animation::UiAnimationPlugin, binding::UiBindingPlugin, focus::UiFocusPlugin,
        input::UiInputPlugin, layer::UiLayerPlugin, panel::UiPanelPlugin, stats::UiStatsPlugin,
        viewport::UiViewportPlugin,
    },
    debug::UiDebugPlugin,
    i18n::UiI18nPlugin,
    overlays::UiRouterPlugin,
    style::{UiFontPlugin, UiThemePlugin},
    widgets::UiWidgetsPlugin,
};

pub(crate) struct UiFrameworkPlugin;

impl Plugin for UiFrameworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            UiFontPlugin,
            UiI18nPlugin,
            UiThemePlugin,
            UiViewportPlugin,
            UiWidgetsPlugin,
            UiLayerPlugin,
            UiRouterPlugin,
            UiPanelPlugin,
            UiInputPlugin,
            UiFocusPlugin,
            UiBindingPlugin,
            UiAnimationPlugin,
            UiStatsPlugin,
            UiDebugPlugin,
        ));
    }
}
