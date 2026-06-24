use crate::framework::ui::{
    core::{UiOwnerId, UiPanelId},
    overlays::{UiModalActionId, UiModalId},
};

pub(in crate::game) const OWNER_LOGIN: UiOwnerId = UiOwnerId::new("login");
pub(in crate::game) const OWNER_LOBBY: UiOwnerId = UiOwnerId::new("lobby");
pub(in crate::game) const OWNER_AUDIO_SETTINGS: UiOwnerId = UiOwnerId::new("audio_settings");
pub(in crate::game) const OWNER_AUDIO_MONITOR: UiOwnerId = UiOwnerId::new("audio_monitor");
pub(in crate::game) const OWNER_AUDIO_GALLERY: UiOwnerId = UiOwnerId::new("audio_gallery");
pub(in crate::game) const OWNER_TOUCH_RIPPLE: UiOwnerId = UiOwnerId::new("wanfa_touch_ripple");
pub(in crate::game) const OWNER_UI_GALLERY: UiOwnerId = UiOwnerId::new("ui_gallery");
pub(in crate::game) const OWNER_SAMPLE_SCENE: UiOwnerId = UiOwnerId::new("sample_scene");
pub(in crate::game) const OWNER_ROBOT_SYNC_SCENE: UiOwnerId = UiOwnerId::new("robot_sync_scene");

pub(in crate::game) const PANEL_LOGIN: UiPanelId = UiPanelId::new("login_page");
pub(in crate::game) const PANEL_GAME_LIST: UiPanelId = UiPanelId::new("game_list_page");
pub(in crate::game) const PANEL_AUDIO_SETTINGS: UiPanelId = UiPanelId::new("audio_settings_page");
pub(in crate::game) const PANEL_AUDIO_MONITOR: UiPanelId = UiPanelId::new("audio_monitor_page");
pub(in crate::game) const PANEL_AUDIO_GALLERY: UiPanelId = UiPanelId::new("audio_gallery_page");
pub(in crate::game) const PANEL_UI_GALLERY: UiPanelId = UiPanelId::new("ui_gallery_page");
pub(in crate::game) const PANEL_GALLERY_FLOATING: UiPanelId = UiPanelId::new("gallery_floating");
pub(in crate::game) const PANEL_TOUCH_RIPPLE_HUD: UiPanelId = UiPanelId::new("touch_ripple_hud");
#[allow(dead_code)]
pub(in crate::game) const PANEL_SAMPLE_SCENE_HUD: UiPanelId = UiPanelId::new("sample_scene_hud");
pub(in crate::game) const PANEL_ROBOT_SYNC_SCENE_HUD: UiPanelId =
    UiPanelId::new("robot_sync_scene_hud");

pub(in crate::game) const MODAL_TOUCH_RIPPLE_LAUNCH: UiModalId =
    UiModalId::new("touch_ripple_launch");
pub(in crate::game) const MODAL_GALLERY_CONFIRM: UiModalId = UiModalId::new("gallery_confirm");

pub(in crate::game) const ACTION_CANCEL: UiModalActionId = UiModalActionId::new("cancel");
pub(in crate::game) const ACTION_CONFIRM: UiModalActionId = UiModalActionId::new("confirm");
pub(in crate::game) const ACTION_TOUCH_RIPPLE_SINGLE_PLAYER: UiModalActionId =
    UiModalActionId::new("touch_ripple_single_player");
pub(in crate::game) const ACTION_TOUCH_RIPPLE_NETWORKED: UiModalActionId =
    UiModalActionId::new("touch_ripple_networked");
