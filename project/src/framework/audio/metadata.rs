use std::{
    collections::HashMap,
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use bevy::prelude::*;
use serde::Deserialize;

use super::{catalog_config::AudioCatalogPathError, id::AudioClipId};

pub const DEFAULT_AUDIO_MANIFEST_PATH: &str = "audio/audio_manifest.ron";

#[derive(Clone, Debug, Default, Resource, PartialEq)]
pub struct AudioMetadata {
    clips: HashMap<AudioClipId, AudioClipMetadata>,
    durations_by_path: HashMap<String, f32>,
}

impl AudioMetadata {
    pub fn from_manifest(manifest: AudioManifest) -> Result<Self, AudioMetadataError> {
        manifest.into_metadata()
    }

    pub fn insert_clip(
        &mut self,
        clip_id: AudioClipId,
        metadata: AudioClipMetadata,
    ) -> Option<AudioClipMetadata> {
        let previous = self.clips.insert(clip_id, metadata.clone());
        if let Some(previous) = &previous {
            if !self.clips.values().any(|clip| clip.path == previous.path) {
                self.durations_by_path.remove(&previous.path);
            }
        }
        self.durations_by_path
            .insert(metadata.path.clone(), metadata.duration_seconds);
        previous
    }

    pub fn clip(&self, clip_id: &AudioClipId) -> Option<&AudioClipMetadata> {
        self.clips.get(clip_id)
    }

    pub fn clip_duration_seconds(&self, clip_id: &AudioClipId) -> Option<f32> {
        self.clip(clip_id).map(|clip| clip.duration_seconds)
    }

    pub fn clip_duration_seconds_by_path(&self, path: &str) -> Option<f32> {
        self.durations_by_path.get(path).copied()
    }

    pub fn len(&self) -> usize {
        self.clips.len()
    }

    pub fn is_empty(&self) -> bool {
        self.clips.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioClipMetadata {
    pub path: String,
    pub duration_seconds: f32,
}

impl AudioClipMetadata {
    pub fn new(path: impl Into<String>, duration_seconds: f32) -> Self {
        Self {
            path: path.into(),
            duration_seconds,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AudioManifest {
    #[serde(default)]
    pub clips: Vec<AudioManifestClip>,
}

impl AudioManifest {
    pub fn from_ron_str(source: &str) -> Result<Self, AudioMetadataError> {
        ron::from_str::<Self>(source).map_err(|source| AudioMetadataError::Ron {
            message: source.to_string(),
        })
    }

    pub fn into_metadata(self) -> Result<AudioMetadata, AudioMetadataError> {
        let mut metadata = AudioMetadata::default();

        for clip in self.clips {
            let clip_id = AudioClipId::try_from(clip.id.clone()).map_err(|reason| {
                AudioMetadataError::InvalidClipId {
                    value: clip.id.clone(),
                    reason: reason.to_string(),
                }
            })?;
            validate_audio_manifest_asset_path(&clip.path).map_err(|reason| {
                AudioMetadataError::InvalidPath {
                    clip_id: clip.id.clone(),
                    path: clip.path.clone(),
                    reason,
                }
            })?;
            if !clip.duration_seconds.is_finite() || clip.duration_seconds < 0.0 {
                return Err(AudioMetadataError::InvalidDuration {
                    clip_id: clip.id,
                    duration_seconds: clip.duration_seconds,
                });
            }

            let previous = metadata.insert_clip(
                clip_id.clone(),
                AudioClipMetadata::new(clip.path, clip.duration_seconds),
            );
            if previous.is_some() {
                return Err(AudioMetadataError::DuplicateClip(clip_id));
            }
        }

        Ok(metadata)
    }

    pub fn load_first_package_ron(
        manifest_path: impl AsRef<str>,
    ) -> Result<Self, AudioManifestLoadError> {
        let manifest_path = manifest_path.as_ref();
        validate_audio_manifest_asset_path(manifest_path).map_err(|source| {
            AudioManifestLoadError::ParseFailed {
                path: PathBuf::from(manifest_path),
                source: AudioMetadataError::InvalidPath {
                    clip_id: "<manifest>".to_string(),
                    path: manifest_path.to_string(),
                    reason: source,
                },
            }
        })?;

        let fs_path = first_package_manifest_fs_path(manifest_path)
            .ok_or_else(|| AudioManifestLoadError::ManifestNotFound(manifest_path.to_string()))?;

        let source =
            fs::read_to_string(&fs_path).map_err(|source| AudioManifestLoadError::ReadFailed {
                path: fs_path.clone(),
                source,
            })?;

        Self::from_ron_str(&source).map_err(|source| AudioManifestLoadError::ParseFailed {
            path: fs_path,
            source,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AudioManifestClip {
    pub id: String,
    pub path: String,
    pub duration_seconds: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AudioMetadataError {
    Ron {
        message: String,
    },
    InvalidClipId {
        value: String,
        reason: String,
    },
    InvalidPath {
        clip_id: String,
        path: String,
        reason: AudioCatalogPathError,
    },
    InvalidDuration {
        clip_id: String,
        duration_seconds: f32,
    },
    DuplicateClip(AudioClipId),
}

impl fmt::Display for AudioMetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ron { message } => {
                write!(formatter, "audio metadata RON parse failed: {message}")
            }
            Self::InvalidClipId { value, reason } => {
                write!(
                    formatter,
                    "audio metadata has invalid clip id `{value}`: {reason}"
                )
            }
            Self::InvalidPath {
                clip_id,
                path,
                reason,
            } => write!(
                formatter,
                "audio metadata clip `{clip_id}` has invalid path `{path}`: {reason}"
            ),
            Self::InvalidDuration {
                clip_id,
                duration_seconds,
            } => write!(
                formatter,
                "audio metadata clip `{clip_id}` has invalid duration: {duration_seconds}"
            ),
            Self::DuplicateClip(clip_id) => {
                write!(
                    formatter,
                    "audio metadata defines duplicate clip `{clip_id}`"
                )
            }
        }
    }
}

impl Error for AudioMetadataError {}

#[derive(Debug)]
pub enum AudioManifestLoadError {
    ManifestNotFound(String),
    ReadFailed {
        path: PathBuf,
        source: io::Error,
    },
    ParseFailed {
        path: PathBuf,
        source: AudioMetadataError,
    },
}

impl fmt::Display for AudioManifestLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManifestNotFound(path) => write!(
                formatter,
                "audio metadata manifest was not found under the first package assets root: {path}"
            ),
            Self::ReadFailed { path, source } => {
                write!(
                    formatter,
                    "failed to read audio metadata manifest at {}: {source}",
                    path.display()
                )
            }
            Self::ParseFailed { path, source } => write!(
                formatter,
                "failed to parse audio metadata manifest RON at {}: {source}",
                path.display()
            ),
        }
    }
}

impl Error for AudioManifestLoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ReadFailed { source, .. } => Some(source),
            Self::ParseFailed { source, .. } => Some(source),
            Self::ManifestNotFound(_) => None,
        }
    }
}

pub fn load_audio_metadata_from_first_package_ron(
    manifest_path: impl AsRef<str>,
) -> Result<AudioMetadata, AudioManifestLoadError> {
    let manifest_path = manifest_path.as_ref();
    AudioManifest::load_first_package_ron(manifest_path)?
        .into_metadata()
        .map_err(|source| AudioManifestLoadError::ParseFailed {
            path: PathBuf::from(manifest_path),
            source,
        })
}

pub(crate) fn validate_audio_manifest_asset_path(path: &str) -> Result<(), AudioCatalogPathError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(AudioCatalogPathError::Empty);
    }
    if trimmed.contains('\\') {
        return Err(AudioCatalogPathError::Backslash);
    }
    if has_windows_drive_prefix(trimmed) {
        return Err(AudioCatalogPathError::WindowsDrive);
    }

    if let Some((scheme, rest)) = trimmed.split_once("://") {
        if scheme != "content_cache" {
            return Err(AudioCatalogPathError::UnsupportedScheme);
        }

        return validate_forward_relative_segments(rest);
    }

    if Path::new(trimmed).is_absolute() || trimmed.starts_with('/') {
        return Err(AudioCatalogPathError::Absolute);
    }

    validate_forward_relative_segments(trimmed)
}

fn validate_forward_relative_segments(path: &str) -> Result<(), AudioCatalogPathError> {
    if path
        .split('/')
        .any(|segment| segment == ".." || segment.is_empty())
    {
        return Err(AudioCatalogPathError::ParentSegment);
    }

    Ok(())
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn first_package_manifest_fs_path(manifest_path: &str) -> Option<PathBuf> {
    first_package_asset_root_candidates()
        .into_iter()
        .map(|root| root.join(Path::new(manifest_path)))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn clip_id(value: &str) -> AudioClipId {
        AudioClipId::try_from(value).unwrap()
    }

    #[test]
    fn valid_manifest_builds_queryable_metadata() {
        let metadata = AudioManifest::from_ron_str(
            r#"
(
    clips: [
        (id: "ui.click", path: "audio/ui/click.wav", duration_seconds: 0.25),
        (id: "music.menu", path: "audio/music/menu.wav", duration_seconds: 12.5),
    ],
)
"#,
        )
        .unwrap()
        .into_metadata()
        .unwrap();

        assert_eq!(metadata.len(), 2);
        assert_eq!(
            metadata.clip_duration_seconds(&clip_id("ui.click")),
            Some(0.25)
        );
        assert_eq!(
            metadata.clip(&clip_id("music.menu")).unwrap().path,
            "audio/music/menu.wav"
        );
        assert_eq!(
            metadata.clip_duration_seconds_by_path("audio/music/menu.wav"),
            Some(12.5)
        );
    }

    #[test]
    fn missing_clip_duration_returns_none() {
        let metadata = AudioMetadata::default();

        assert_eq!(metadata.clip_duration_seconds(&clip_id("ui.missing")), None);
    }

    #[test]
    fn rejects_unsafe_paths_and_invalid_durations() {
        let invalid_path = AudioManifest::from_ron_str(
            r#"
(
    clips: [
        (id: "ui.click", path: "audio/../secret.wav", duration_seconds: 0.25),
    ],
)
"#,
        )
        .unwrap()
        .into_metadata()
        .unwrap_err();
        assert!(matches!(
            invalid_path,
            AudioMetadataError::InvalidPath {
                reason: AudioCatalogPathError::ParentSegment,
                ..
            }
        ));

        let invalid_duration = AudioManifest::from_ron_str(
            r#"
(
    clips: [
        (id: "ui.click", path: "audio/ui/click.wav", duration_seconds: -1.0),
    ],
)
"#,
        )
        .unwrap()
        .into_metadata()
        .unwrap_err();
        assert!(matches!(
            invalid_duration,
            AudioMetadataError::InvalidDuration { .. }
        ));
    }

    #[test]
    fn duplicate_clip_ids_are_rejected() {
        let error = AudioManifest::from_ron_str(
            r#"
(
    clips: [
        (id: "ui.click", path: "audio/ui/click.wav", duration_seconds: 0.25),
        (id: "ui.click", path: "audio/ui/click_alt.wav", duration_seconds: 0.3),
    ],
)
"#,
        )
        .unwrap()
        .into_metadata()
        .unwrap_err();

        assert_eq!(
            error,
            AudioMetadataError::DuplicateClip(clip_id("ui.click"))
        );
    }
}
