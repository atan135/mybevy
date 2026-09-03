// preview3d module – Stage 1 skeleton only. All types below will be replaced in later stages.
use bevy::prelude::*;

pub struct Preview3dViewportPlugin;

impl Plugin for Preview3dViewportPlugin {
    fn build(&self, _app: &mut App) {
        // Registration point for UI 3D preview viewport capabilities.
        // Systems, components, and resources will be added in stages 2-7.
    }
}

mod config;
mod target;
