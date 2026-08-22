mod host;

#[cfg(test)]
mod tests;

pub(in crate::game) use host::MAIN_WORLD_SETTINGS_ROUTE;
#[cfg(all(debug_assertions, not(target_os = "android")))]
pub(super) use host::{
    AudioSettingsAuditFixture, clear_audio_settings_audit_fixture,
    mark_audio_settings_audit_scroll, prepare_audio_settings_audit_fixture,
};
pub(super) use host::{
    AudioSettingsHostContract, MainWorldSettingsTab, cleanup_audio_settings_screen,
    handle_audio_settings_document_actions, register_audio_settings_contract,
    sync_audio_settings_document_bindings, sync_main_world_audio_settings_document_bindings,
};
