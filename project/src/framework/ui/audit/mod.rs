mod local;
pub(crate) mod screenshot;
mod semantic;

#[cfg(all(debug_assertions, not(target_os = "android")))]
pub(crate) use local::UiAuditConfig;
pub(crate) use local::{
    UiAuditCaptureRecipe, UiAuditCaptureState, UiAuditCaptureStateApplied,
    UiAuditDynamicContentRecipe, UiAuditPlugin, UiAuditReadyCondition, UiAuditRecipe,
    UiAuditReferenceRecipe, UiAuditRouteCommand, UiAuditScreen, UiAuditScreenRecipe,
    UiAuditScreenRegistry, UiAuditTargetViewport,
};
#[allow(unused_imports)]
pub(crate) use screenshot::{
    UiScreenshotCommand, UiScreenshotEvent, UiScreenshotFailed, UiScreenshotFailureReason,
    UiScreenshotRequestId, UiScreenshotRequestRecord, UiScreenshotSaved,
};
