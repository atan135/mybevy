use bevy::prelude::*;

const UI_FONT_REGULAR_PATH: &str = "ui/fonts/MyBevyUiCjk-Regular.otf";

pub(in crate::game) struct UiFontPlugin;

impl Plugin for UiFontPlugin {
    fn build(&self, app: &mut App) {
        let regular = app
            .world()
            .resource::<AssetServer>()
            .load(UI_FONT_REGULAR_PATH);
        app.insert_resource(UiFontAssets { regular });
        info!(path = UI_FONT_REGULAR_PATH, "loaded ui font asset");
    }
}

#[derive(Clone, Debug, Resource)]
pub(in crate::game) struct UiFontAssets {
    pub regular: Handle<Font>,
}
