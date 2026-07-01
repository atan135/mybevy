use bevy::prelude::*;

use super::FangyuanPrimitiveSet;

#[derive(Component, Clone, Debug, PartialEq)]
pub struct FangyuanAvatar {
    pub blueprint_id: String,
    pub display_name: String,
    pub primitives: FangyuanPrimitiveSet,
}

impl FangyuanAvatar {
    pub fn new(
        blueprint_id: impl Into<String>,
        display_name: impl Into<String>,
        primitives: FangyuanPrimitiveSet,
    ) -> Self {
        Self {
            blueprint_id: blueprint_id.into(),
            display_name: display_name.into(),
            primitives,
        }
    }
}
