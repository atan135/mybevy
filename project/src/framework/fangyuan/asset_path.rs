use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

pub const FANGYUAN_FIRST_PACKAGE_ASSET_ROOT: &str = "fangyuan";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanAssetPathError {
    Empty,
    Absolute(String),
    Backslash(String),
    WindowsDrive(String),
    ParentOrEmptySegment(String),
    OutsideFangyuanRoot(String),
}

impl fmt::Display for FangyuanAssetPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("fangyuan asset path must not be empty"),
            Self::Absolute(path) => write!(
                formatter,
                "fangyuan asset path must be relative to assets: {path}"
            ),
            Self::Backslash(path) => write!(
                formatter,
                "fangyuan asset path must use forward slashes: {path}"
            ),
            Self::WindowsDrive(path) => write!(
                formatter,
                "fangyuan asset path must not include a Windows drive prefix: {path}"
            ),
            Self::ParentOrEmptySegment(path) => {
                write!(
                    formatter,
                    "fangyuan asset path must stay inside assets: {path}"
                )
            }
            Self::OutsideFangyuanRoot(path) => write!(
                formatter,
                "fangyuan asset path must stay inside assets/fangyuan: {path}"
            ),
        }
    }
}

impl Error for FangyuanAssetPathError {}

pub fn validate_fangyuan_asset_path(path: &str) -> Result<(), FangyuanAssetPathError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(FangyuanAssetPathError::Empty);
    }
    if trimmed.contains('\\') {
        return Err(FangyuanAssetPathError::Backslash(trimmed.to_string()));
    }
    if has_windows_drive_prefix(trimmed) {
        return Err(FangyuanAssetPathError::WindowsDrive(trimmed.to_string()));
    }
    if Path::new(trimmed).is_absolute() || trimmed.starts_with('/') {
        return Err(FangyuanAssetPathError::Absolute(trimmed.to_string()));
    }
    if trimmed
        .split('/')
        .any(|segment| segment.is_empty() || segment == "..")
    {
        return Err(FangyuanAssetPathError::ParentOrEmptySegment(
            trimmed.to_string(),
        ));
    }
    if trimmed != FANGYUAN_FIRST_PACKAGE_ASSET_ROOT
        && !trimmed.starts_with(&format!("{FANGYUAN_FIRST_PACKAGE_ASSET_ROOT}/"))
    {
        return Err(FangyuanAssetPathError::OutsideFangyuanRoot(
            trimmed.to_string(),
        ));
    }

    Ok(())
}

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

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}
