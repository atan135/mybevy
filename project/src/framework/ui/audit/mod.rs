mod local;
pub(crate) mod screenshot;

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
