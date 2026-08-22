/// Adapter boundary for authority-specific preview facts. The concrete
/// authority session and protocol types remain private to the game layer.
pub trait AuthorityPreviewAdapter {
    fn collect_authority_frame(&self) -> Option<u64>;
}
