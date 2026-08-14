use std::{error::Error, fmt, path::Path};

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

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_fangyuan_relative_paths() {
        assert_eq!(
            validate_fangyuan_asset_path("fangyuan/avatars/minimal_player.ron"),
            Ok(())
        );
        assert_eq!(validate_fangyuan_asset_path("fangyuan"), Ok(()));
    }

    #[test]
    fn rejects_paths_outside_fangyuan_root() {
        assert!(matches!(
            validate_fangyuan_asset_path("audio/click.wav"),
            Err(FangyuanAssetPathError::OutsideFangyuanRoot(_))
        ));
    }

    #[test]
    fn rejects_platform_or_traversal_paths() {
        for path in [
            "",
            "\\fangyuan\\mesh.glb",
            "C:/fangyuan/mesh.glb",
            "/fangyuan/mesh.glb",
            "fangyuan/../mesh.glb",
            "fangyuan//mesh.glb",
        ] {
            assert!(
                validate_fangyuan_asset_path(path).is_err(),
                "{path} should be rejected"
            );
        }
    }
}
