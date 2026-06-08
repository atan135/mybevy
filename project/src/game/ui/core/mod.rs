pub(in crate::game) mod framework;
pub(in crate::game) mod input;
pub(in crate::game) mod layer;
pub(in crate::game) mod panel;

pub(in crate::game) use framework::UiFrameworkPlugin;
pub(in crate::game) use input::{UiInputState, UiInputSystems};
pub(in crate::game) use layer::{UiLayer, UiLayerRoot};
pub(in crate::game) use panel::{
    UiPanelCommand, UiPanelId, UiPanelKind, UiPanelRequest, UiPanelRoot, UiPanelSystems,
};
