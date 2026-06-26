pub(crate) mod screenshot;

#[allow(unused_imports)]
pub(crate) use screenshot::{
    UiAuditPlugin, UiScreenshotCommand, UiScreenshotEvent, UiScreenshotFailed,
    UiScreenshotFailureReason, UiScreenshotRequestId, UiScreenshotRequestRecord, UiScreenshotSaved,
};
