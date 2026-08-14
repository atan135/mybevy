mod approval;
mod asset;
mod binding_action;
mod budget;
mod canonical;
mod content;
mod control;
mod id;
mod layout;
mod model;
mod preview;
mod remote;
mod report;
mod responsive;
mod runtime;
#[cfg(all(not(target_os = "android"), feature = "ui-document-preview-tool"))]
mod standalone_preview;
mod style;
pub mod tooling;
mod update;
mod validation;

/// Runtime-free schema facade shared with repository UI tooling. Runtime systems continue to
/// use the Bevy adaptation modules in this directory; this alias keeps the shared core explicit.
pub mod core {
    pub use ui_document_core::*;
}

pub use approval::*;
pub use asset::*;
pub use binding_action::*;
pub use budget::*;
pub use content::*;
pub use control::*;
pub use id::*;
pub use layout::*;
pub use model::*;
pub use preview::*;
pub use remote::*;
pub use report::*;
pub use responsive::*;
pub use runtime::*;
#[cfg(all(not(target_os = "android"), feature = "ui-document-preview-tool"))]
pub use standalone_preview::*;
pub use style::*;
pub use update::*;
pub use validation::*;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod validation_tests;
