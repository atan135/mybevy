use std::path::{Path, PathBuf};

pub use fangyuan_core::{
    FANGYUAN_FIRST_PACKAGE_ASSET_ROOT, FangyuanAssetPathError, validate_fangyuan_asset_path,
};

pub(super) fn first_package_fangyuan_asset_fs_path(asset_path: &str) -> Option<PathBuf> {
    first_package_asset_root_candidates()
        .into_iter()
        .map(|root| root.join(Path::new(asset_path)))
        .find(|candidate| candidate.is_file())
}

fn first_package_asset_root_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("assets"));
        candidates.push(current_dir.join("project").join("assets"));
    }
    candidates.push(PathBuf::from("assets"));
    candidates.push(PathBuf::from("project").join("assets"));
    candidates
}
