pub(in crate::game) mod loading;
pub(in crate::game) mod modal;
pub(in crate::game) mod router;
pub(in crate::game) mod toast;

pub(in crate::game) use loading::{UiLoading, UiLoadingRoot};
pub(in crate::game) use modal::{
    UiConfirmModal, UiModal, UiModalAction, UiModalActionSpec, UiModalActionStyle, UiModalId,
    UiModalResult, UiModalRoot,
};
pub(in crate::game) use router::{UiRouteCommand, UiRouterPlugin};
pub(in crate::game) use toast::UiToast;
