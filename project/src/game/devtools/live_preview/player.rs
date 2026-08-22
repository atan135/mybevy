use crate::framework::devtools::live_preview::PlayerPreviewState;

pub trait PlayerPreviewAdapter {
    fn collect_player_preview(&self) -> PlayerPreviewState;
}
