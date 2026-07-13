use crate::framework::ui::{
    core::{UiOwnerId, UiPanelId},
    overlays::{UiModalActionId, UiModalId},
    widgets::{UiScrollAuditAnchorId, UiScrollAuditId},
};

pub(in crate::game) const OWNER_LOGIN: UiOwnerId = UiOwnerId::new("login");
pub(in crate::game) const OWNER_CHARACTER_SELECT: UiOwnerId = UiOwnerId::new("character_select");
pub(in crate::game) const OWNER_LOBBY: UiOwnerId = UiOwnerId::new("lobby");
pub(in crate::game) const OWNER_AUDIO_SETTINGS: UiOwnerId = UiOwnerId::new("audio_settings");
pub(in crate::game) const OWNER_AUDIO_MONITOR: UiOwnerId = UiOwnerId::new("audio_monitor");
pub(in crate::game) const OWNER_AUDIO_GALLERY: UiOwnerId = UiOwnerId::new("audio_gallery");
pub(in crate::game) const OWNER_TOUCH_RIPPLE: UiOwnerId = UiOwnerId::new("wanfa_touch_ripple");
pub(in crate::game) const OWNER_UI_GALLERY: UiOwnerId = UiOwnerId::new("ui_gallery");
pub(in crate::game) const OWNER_UI_DOCUMENT_GALLERY: UiOwnerId =
    UiOwnerId::new("ui_document_gallery");
pub(in crate::game) const OWNER_SAMPLE_SCENE: UiOwnerId = UiOwnerId::new("sample_scene");
pub(in crate::game) const OWNER_ROBOT_SYNC_SCENE: UiOwnerId = UiOwnerId::new("robot_sync_scene");
pub(in crate::game) const OWNER_FANGYUAN_HOME: UiOwnerId = UiOwnerId::new("fangyuan_home");
pub(in crate::game) const OWNER_FANGYUAN_PLAYER_PREVIEW: UiOwnerId =
    UiOwnerId::new("fangyuan_player_preview");

pub(in crate::game) const PANEL_LOGIN: UiPanelId = UiPanelId::new("login_page");
pub(in crate::game) const PANEL_CHARACTER_SELECT: UiPanelId =
    UiPanelId::new("character_select_page");
pub(in crate::game) const PANEL_GAME_LIST: UiPanelId = UiPanelId::new("game_list_page");
pub(in crate::game) const PANEL_AUDIO_SETTINGS: UiPanelId = UiPanelId::new("audio_settings_page");
pub(in crate::game) const PANEL_AUDIO_MONITOR: UiPanelId = UiPanelId::new("audio_monitor_page");
pub(in crate::game) const PANEL_AUDIO_GALLERY: UiPanelId = UiPanelId::new("audio_gallery_page");
pub(in crate::game) const PANEL_UI_GALLERY: UiPanelId = UiPanelId::new("ui_gallery_page");
pub(in crate::game) const SCROLL_UI_GALLERY_MAIN: UiScrollAuditId =
    UiScrollAuditId::new("ui_gallery.main");
pub(in crate::game) const ANCHOR_UI_GALLERY_VISUAL_ACCEPTANCE: UiScrollAuditAnchorId =
    UiScrollAuditAnchorId::new("ui_gallery.visual_acceptance");
pub(in crate::game) const ANCHOR_UI_GALLERY_IMAGE_MODES: UiScrollAuditAnchorId =
    UiScrollAuditAnchorId::new("ui_gallery.image_modes");
pub(in crate::game) const ANCHOR_UI_GALLERY_IMAGE_TILING: UiScrollAuditAnchorId =
    UiScrollAuditAnchorId::new("ui_gallery.image_tiling");
pub(in crate::game) const ANCHOR_UI_GALLERY_IMAGE_ATLAS: UiScrollAuditAnchorId =
    UiScrollAuditAnchorId::new("ui_gallery.image_atlas");
pub(in crate::game) const ANCHOR_UI_GALLERY_TYPOGRAPHY: UiScrollAuditAnchorId =
    UiScrollAuditAnchorId::new("ui_gallery.typography");
pub(in crate::game) const ANCHOR_UI_GALLERY_TYPOGRAPHY_OVERFLOW: UiScrollAuditAnchorId =
    UiScrollAuditAnchorId::new("ui_gallery.typography_overflow");
pub(in crate::game) const ANCHOR_UI_GALLERY_ICONS: UiScrollAuditAnchorId =
    UiScrollAuditAnchorId::new("ui_gallery.icons");
pub(in crate::game) const ANCHOR_UI_GALLERY_ICON_STATES: UiScrollAuditAnchorId =
    UiScrollAuditAnchorId::new("ui_gallery.icon_states");
pub(in crate::game) const ANCHOR_UI_GALLERY_STYLE_SCOPES: UiScrollAuditAnchorId =
    UiScrollAuditAnchorId::new("ui_gallery.style_scopes");
pub(in crate::game) const ANCHOR_UI_GALLERY_EFFECTS: UiScrollAuditAnchorId =
    UiScrollAuditAnchorId::new("ui_gallery.effects");
pub(in crate::game) const ANCHOR_UI_GALLERY_ANIMATIONS: UiScrollAuditAnchorId =
    UiScrollAuditAnchorId::new("ui_gallery.animations");
pub(in crate::game) const ANCHOR_UI_GALLERY_COMPONENTS: UiScrollAuditAnchorId =
    UiScrollAuditAnchorId::new("ui_gallery.components");
pub(in crate::game) const ANCHOR_UI_GALLERY_COMPONENT_DROPDOWN: UiScrollAuditAnchorId =
    UiScrollAuditAnchorId::new("ui_gallery.components.dropdown");
pub(in crate::game) const ANCHOR_UI_GALLERY_COMPONENT_TOOLTIP: UiScrollAuditAnchorId =
    UiScrollAuditAnchorId::new("ui_gallery.components.tooltip");
pub(in crate::game) const ANCHOR_UI_GALLERY_COMPONENT_CHECKBOXES: UiScrollAuditAnchorId =
    UiScrollAuditAnchorId::new("ui_gallery.components.checkboxes");
pub(in crate::game) const ANCHOR_UI_GALLERY_COMPONENT_TOGGLES: UiScrollAuditAnchorId =
    UiScrollAuditAnchorId::new("ui_gallery.components.toggles");
pub(in crate::game) const ANCHOR_UI_GALLERY_COMPONENT_SEGMENTED: UiScrollAuditAnchorId =
    UiScrollAuditAnchorId::new("ui_gallery.components.segmented");
pub(in crate::game) const PANEL_GALLERY_FLOATING: UiPanelId = UiPanelId::new("gallery_floating");
pub(in crate::game) const PANEL_TOUCH_RIPPLE_HUD: UiPanelId = UiPanelId::new("touch_ripple_hud");
#[allow(dead_code)]
pub(in crate::game) const PANEL_SAMPLE_SCENE_HUD: UiPanelId = UiPanelId::new("sample_scene_hud");
pub(in crate::game) const PANEL_ROBOT_SYNC_SCENE_HUD: UiPanelId =
    UiPanelId::new("robot_sync_scene_hud");
pub(in crate::game) const PANEL_FANGYUAN_HOME_HUD: UiPanelId = UiPanelId::new("fangyuan_home_hud");
pub(in crate::game) const PANEL_FANGYUAN_PLAYER_PREVIEW_HUD: UiPanelId =
    UiPanelId::new("fangyuan_player_preview_hud");

pub(in crate::game) const MODAL_TOUCH_RIPPLE_LAUNCH: UiModalId =
    UiModalId::new("touch_ripple_launch");
pub(in crate::game) const MODAL_GALLERY_CONFIRM: UiModalId = UiModalId::new("gallery_confirm");

pub(in crate::game) const ACTION_CANCEL: UiModalActionId = UiModalActionId::new("cancel");
pub(in crate::game) const ACTION_CONFIRM: UiModalActionId = UiModalActionId::new("confirm");
pub(in crate::game) const ACTION_TOUCH_RIPPLE_SINGLE_PLAYER: UiModalActionId =
    UiModalActionId::new("touch_ripple_single_player");
pub(in crate::game) const ACTION_TOUCH_RIPPLE_NETWORKED: UiModalActionId =
    UiModalActionId::new("touch_ripple_networked");
