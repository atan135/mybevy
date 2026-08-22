use crate::framework::devtools::live_preview::NetworkPreviewState;

pub trait NetworkPreviewAdapter {
    fn collect_network_preview(&self) -> NetworkPreviewState;
}
