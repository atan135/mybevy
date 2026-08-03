use bevy::prelude::*;

use crate::game::myserver::GameConnectionState;

pub(super) const ENTRY_TOUCH_RIPPLE: &str = "game:touch_ripple";
pub(super) const ENTRY_LOCKSTEP_SIM: &str = "scene:lockstep_sim_arena";
pub(super) const ENTRY_SAMPLE_DUNGEON: &str = "scene:sample_dungeon_room";
pub(super) const ENTRY_ROBOT_SYNC: &str = "scene:robot_sync_arena";
pub(super) const ENTRY_FANGYUAN_HOME: &str = "scene:fangyuan_home";
pub(super) const ENTRY_FANGYUAN_PLAYER_PREVIEW: &str = "route:fangyuan_player_preview";
pub(super) const LOBBY_MAX_ENTRIES: u16 = 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LobbyEntryTarget {
    TouchRipple,
    LockstepSim,
    SampleDungeon,
    RobotSync,
    FangyuanHome,
    FangyuanPlayerPreview,
    #[cfg(all(debug_assertions, not(target_os = "android")))]
    AuditOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LobbyEntry {
    pub entry_id: String,
    pub title: String,
    pub description: String,
    pub badge: String,
    pub target: LobbyEntryTarget,
    pub enabled: bool,
}

impl LobbyEntry {
    fn built_in(
        entry_id: &str,
        title: &str,
        description: &str,
        badge: &str,
        target: LobbyEntryTarget,
    ) -> Self {
        Self {
            entry_id: entry_id.to_owned(),
            title: title.to_owned(),
            description: description.to_owned(),
            badge: badge.to_owned(),
            target,
            enabled: true,
        }
    }
}

pub(super) fn built_in_lobby_entries() -> Vec<LobbyEntry> {
    vec![
        LobbyEntry::built_in(
            ENTRY_TOUCH_RIPPLE,
            "Touch Ripple",
            "Current touch and mouse interaction prototype",
            "PLAY",
            LobbyEntryTarget::TouchRipple,
        ),
        LobbyEntry::built_in(
            ENTRY_LOCKSTEP_SIM,
            "Lockstep Sim",
            "Shared deterministic simulation arena",
            "SCENE",
            LobbyEntryTarget::LockstepSim,
        ),
        LobbyEntry::built_in(
            ENTRY_SAMPLE_DUNGEON,
            "Sample Scene",
            "Dungeon room scene prototype",
            "SCENE",
            LobbyEntryTarget::SampleDungeon,
        ),
        LobbyEntry::built_in(
            ENTRY_ROBOT_SYNC,
            "Robot Sync",
            "500x500 authority frame robot arena",
            "ONLINE",
            LobbyEntryTarget::RobotSync,
        ),
        LobbyEntry::built_in(
            ENTRY_FANGYUAN_HOME,
            "Fangyuan Home",
            "Blueprint home scene preview",
            "SCENE",
            LobbyEntryTarget::FangyuanHome,
        ),
        LobbyEntry::built_in(
            ENTRY_FANGYUAN_PLAYER_PREVIEW,
            "Fangyuan Player Preview",
            "Minimal player entity appearance loop",
            "PREVIEW",
            LobbyEntryTarget::FangyuanPlayerPreview,
        ),
    ]
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum LobbyCollectionState {
    Loading,
    #[default]
    Ready,
    Error,
}

impl LobbyCollectionState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Ready => "ready",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Resource)]
pub(super) struct LobbyUiState {
    pub entries: Vec<LobbyEntry>,
    pub collection_state: LobbyCollectionState,
    pub selected_entry_id: Option<String>,
    pub pending_entry_id: Option<String>,
    pub confirming_entry_id: Option<String>,
    pub error_title: String,
    pub error_detail: String,
    pub resource_title: String,
    pub resource_detail: String,
    pub resource_notice_visible: bool,
    pub connection_override: Option<GameConnectionState>,
    pub reload_frames_remaining: u8,
    pub audit_fixture_active: bool,
}

impl Default for LobbyUiState {
    fn default() -> Self {
        let entries = built_in_lobby_entries();
        let selected_entry_id = entries.first().map(|entry| entry.entry_id.clone());
        Self {
            entries,
            collection_state: LobbyCollectionState::Ready,
            selected_entry_id,
            pending_entry_id: None,
            confirming_entry_id: None,
            error_title: String::new(),
            error_detail: String::new(),
            resource_title: "Packaged UI ready".to_owned(),
            resource_detail: "Lobby resources passed validation.".to_owned(),
            resource_notice_visible: false,
            connection_override: None,
            reload_frames_remaining: 0,
            audit_fixture_active: false,
        }
    }
}

impl LobbyUiState {
    pub fn entry(&self, entry_id: &str) -> Option<&LobbyEntry> {
        self.entries.iter().find(|entry| entry.entry_id == entry_id)
    }

    pub fn select(&mut self, entry_id: &str) -> bool {
        if self.entry(entry_id).is_some_and(|entry| entry.enabled) {
            self.selected_entry_id = Some(entry_id.to_owned());
            true
        } else {
            false
        }
    }

    pub fn begin_reload(&mut self) {
        self.collection_state = LobbyCollectionState::Loading;
        self.error_title.clear();
        self.error_detail.clear();
        self.reload_frames_remaining = 1;
    }

    pub fn finish_reload(&mut self) {
        self.entries = built_in_lobby_entries();
        self.collection_state = LobbyCollectionState::Ready;
        if self
            .selected_entry_id
            .as_deref()
            .is_none_or(|entry_id| self.entry(entry_id).is_none())
        {
            self.selected_entry_id = self.entries.first().map(|entry| entry.entry_id.clone());
        }
        self.reload_frames_remaining = 0;
    }

    pub fn clear_pending(&mut self) {
        self.pending_entry_id = None;
    }

    pub fn clear_transient(&mut self) {
        self.pending_entry_id = None;
        self.confirming_entry_id = None;
        self.reload_frames_remaining = 0;
        self.connection_override = None;
        self.audit_fixture_active = false;
    }
}
