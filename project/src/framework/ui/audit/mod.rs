mod local;
pub(crate) mod screenshot;

pub(crate) use local::{
    UiAuditCaptureRecipe, UiAuditCaptureState, UiAuditCaptureStateApplied, UiAuditPlugin,
    UiAuditRecipe, UiAuditRouteCommand, UiAuditScreen, UiAuditScreenRecipe, UiAuditScreenRegistry,
};
#[allow(unused_imports)]
pub(crate) use screenshot::{
    UiScreenshotCommand, UiScreenshotEvent, UiScreenshotFailed, UiScreenshotFailureReason,
    UiScreenshotRequestId, UiScreenshotRequestRecord, UiScreenshotSaved,
};
