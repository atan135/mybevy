use std::{
    collections::HashSet,
    error::Error,
    fmt, fs, io,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;

use super::{
    catalog::{
        AudioCatalog, AudioClipEntry, AudioCueClip, AudioCueEntry, AudioCuePlayback, AudioCueRules,
        AudioGroupClip, AudioGroupEntry,
    },
    id::{AudioClipId, AudioCueId, AudioGroupId},
    scope::{AudioBus, AudioScope},
};

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AudioCatalogConfig {
    #[serde(default)]
    pub clips: Vec<AudioClipConfig>,
    #[serde(default)]
    pub cues: Vec<AudioCueConfig>,
    #[serde(default)]
    pub groups: Vec<AudioGroupConfig>,
}

impl AudioCatalogConfig {
    pub fn from_ron_str(source: &str) -> Result<Self, AudioCatalogConfigError> {
        ron::from_str::<Self>(source).map_err(|source| AudioCatalogConfigError::Ron {
            message: source.to_string(),
        })
    }

    pub fn into_catalog(self) -> Result<AudioCatalog, AudioCatalogConfigError> {
        let mut clip_ids = HashSet::new();
        let mut cue_ids = HashSet::new();
        let mut group_ids = HashSet::new();

        let mut clips = Vec::with_capacity(self.clips.len());
        let mut cues = Vec::with_capacity(self.cues.len());
        let mut groups = Vec::with_capacity(self.groups.len());

        for clip in self.clips {
            let clip_id = parse_clip_id("clips", &clip.id)?;
            validate_audio_asset_path(&clip.path).map_err(|reason| {
                AudioCatalogConfigError::InvalidPath {
                    clip_id: clip.id.clone(),
                    path: clip.path.clone(),
                    reason,
                }
            })?;

            if !clip_ids.insert(clip_id.clone()) {
                return Err(AudioCatalogConfigError::DuplicateClip(clip_id));
            }

            clips.push((clip_id, AudioClipEntry::new(clip.path)));
        }

        for cue in self.cues {
            let cue_id = parse_cue_id("cues", &cue.id)?;
            if !cue_ids.insert(cue_id.clone()) {
                return Err(AudioCatalogConfigError::DuplicateCue(cue_id));
            }

            let mut cue_clips = Vec::with_capacity(cue.clips.len());
            for cue_clip in cue.clips {
                let clip_id = parse_clip_id("cues[].clips", &cue_clip.clip)?;
                if !clip_ids.contains(&clip_id) {
                    return Err(AudioCatalogConfigError::MissingCueClipReference {
                        cue_id: cue_id.clone(),
                        clip_id,
                    });
                }

                cue_clips.push(AudioCueClip::weighted(clip_id, cue_clip.weight));
            }

            cues.push((
                cue_id,
                AudioCueEntry::from_clips(cue_clips)
                    .with_playback(cue.playback.into_playback()?)
                    .with_rules(cue.rules.into_rules()),
            ));
        }

        for group in self.groups {
            let group_id = parse_group_id("groups", &group.id)?;
            if !group_ids.insert(group_id.clone()) {
                return Err(AudioCatalogConfigError::DuplicateGroup(group_id));
            }

            let mut group_clips = Vec::with_capacity(group.clips.len());
            for group_clip in group.clips {
                let clip_id = parse_clip_id("groups[].clips", &group_clip.clip)?;
                if !clip_ids.contains(&clip_id) {
                    return Err(AudioCatalogConfigError::MissingGroupClipReference {
                        group_id: group_id.clone(),
                        clip_id,
                    });
                }

                group_clips.push(if group_clip.required {
                    AudioGroupClip::required(clip_id)
                } else {
                    AudioGroupClip::optional(clip_id)
                });
            }

            groups.push((group_id, AudioGroupEntry::from_clips(group_clips)));
        }

        let mut catalog = AudioCatalog::default();
        for (clip_id, clip) in clips {
            catalog.register_clip(clip_id, clip.path);
        }
        for (cue_id, cue) in cues {
            catalog.register_cue(cue_id, cue);
        }
        for (group_id, group) in groups {
            catalog.register_group(group_id, group);
        }

        Ok(catalog)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AudioClipConfig {
    pub id: String,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AudioCueConfig {
    pub id: String,
    #[serde(default)]
    pub clips: Vec<AudioCueClipConfig>,
    #[serde(default)]
    pub playback: AudioCuePlaybackConfig,
    #[serde(default)]
    pub rules: AudioCueRulesConfig,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AudioCueClipConfig {
    pub clip: String,
    #[serde(default = "default_clip_weight")]
    pub weight: f32,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AudioCuePlaybackConfig {
    #[serde(default)]
    pub bus: AudioBusConfig,
    #[serde(default)]
    pub scope: AudioScopeConfig,
    #[serde(default)]
    pub looped: bool,
}

impl AudioCuePlaybackConfig {
    fn into_playback(self) -> Result<AudioCuePlayback, AudioCatalogConfigError> {
        Ok(AudioCuePlayback {
            bus: self.bus.into_bus(),
            scope: self.scope.into_scope()?,
            looped: self.looped,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AudioCueRulesConfig {
    #[serde(default = "default_rule_volume")]
    pub volume: f32,
    #[serde(default = "default_rule_pitch")]
    pub pitch: f32,
    #[serde(default, deserialize_with = "deserialize_optional_f32_value")]
    pub cooldown_seconds: Option<f32>,
    #[serde(default, deserialize_with = "deserialize_optional_usize_value")]
    pub max_concurrent: Option<usize>,
    #[serde(default)]
    pub priority: i32,
}

impl Default for AudioCueRulesConfig {
    fn default() -> Self {
        Self {
            volume: default_rule_volume(),
            pitch: default_rule_pitch(),
            cooldown_seconds: None,
            max_concurrent: None,
            priority: 0,
        }
    }
}

impl AudioCueRulesConfig {
    fn into_rules(self) -> AudioCueRules {
        AudioCueRules {
            volume: self.volume,
            pitch: self.pitch,
            cooldown_seconds: self.cooldown_seconds,
            max_concurrent: self.max_concurrent,
            priority: self.priority,
        }
    }
}

const fn default_rule_volume() -> f32 {
    1.0
}

const fn default_rule_pitch() -> f32 {
    1.0
}

const fn default_clip_weight() -> f32 {
    1.0
}

const fn default_group_required() -> bool {
    true
}

fn deserialize_optional_f32_value<'de, D>(deserializer: D) -> Result<Option<f32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    f32::deserialize(deserializer).map(Some)
}

fn deserialize_optional_usize_value<'de, D>(deserializer: D) -> Result<Option<usize>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    usize::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AudioGroupConfig {
    pub id: String,
    #[serde(default)]
    pub clips: Vec<AudioGroupClipConfig>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AudioGroupClipConfig {
    pub clip: String,
    #[serde(default = "default_group_required")]
    pub required: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AudioBusConfig {
    Master,
    Music,
    #[default]
    Sfx,
    Ui,
    Battle,
}

impl AudioBusConfig {
    const fn into_bus(self) -> AudioBus {
        match self {
            Self::Master => AudioBus::Master,
            Self::Music => AudioBus::Music,
            Self::Sfx => AudioBus::Sfx,
            Self::Ui => AudioBus::Ui,
            Self::Battle => AudioBus::Battle,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum AudioScopeConfig {
    #[default]
    Global,
    Ui,
    Scene(String),
    Battle(String),
}

impl AudioScopeConfig {
    fn into_scope(self) -> Result<AudioScope, AudioCatalogConfigError> {
        match self {
            Self::Global => Ok(AudioScope::Global),
            Self::Ui => Ok(AudioScope::Ui),
            Self::Scene(id) => AudioScope::scene(id.clone()).map_err(|reason| {
                AudioCatalogConfigError::InvalidScope {
                    scope: format!("scene:{id}"),
                    reason: reason.to_string(),
                }
            }),
            Self::Battle(id) => AudioScope::battle(id.clone()).map_err(|reason| {
                AudioCatalogConfigError::InvalidScope {
                    scope: format!("battle:{id}"),
                    reason: reason.to_string(),
                }
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioCatalogConfigError {
    Ron {
        message: String,
    },
    InvalidClipId {
        section: &'static str,
        value: String,
        reason: String,
    },
    InvalidCueId {
        section: &'static str,
        value: String,
        reason: String,
    },
    InvalidGroupId {
        section: &'static str,
        value: String,
        reason: String,
    },
    InvalidScope {
        scope: String,
        reason: String,
    },
    InvalidPath {
        clip_id: String,
        path: String,
        reason: AudioCatalogPathError,
    },
    DuplicateClip(AudioClipId),
    DuplicateCue(AudioCueId),
    DuplicateGroup(AudioGroupId),
    MissingCueClipReference {
        cue_id: AudioCueId,
        clip_id: AudioClipId,
    },
    MissingGroupClipReference {
        group_id: AudioGroupId,
        clip_id: AudioClipId,
    },
}

impl fmt::Display for AudioCatalogConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ron { message } => write!(formatter, "audio catalog RON parse failed: {message}"),
            Self::InvalidClipId {
                section,
                value,
                reason,
            } => write!(
                formatter,
                "audio catalog {section} has invalid clip id `{value}`: {reason}"
            ),
            Self::InvalidCueId {
                section,
                value,
                reason,
            } => write!(
                formatter,
                "audio catalog {section} has invalid cue id `{value}`: {reason}"
            ),
            Self::InvalidGroupId {
                section,
                value,
                reason,
            } => write!(
                formatter,
                "audio catalog {section} has invalid group id `{value}`: {reason}"
            ),
            Self::InvalidScope { scope, reason } => write!(
                formatter,
                "audio catalog playback has invalid scope `{scope}`: {reason}"
            ),
            Self::InvalidPath {
                clip_id,
                path,
                reason,
            } => write!(
                formatter,
                "audio catalog clip `{clip_id}` has invalid path `{path}`: {reason}"
            ),
            Self::DuplicateClip(clip_id) => {
                write!(
                    formatter,
                    "audio catalog defines duplicate clip `{clip_id}`"
                )
            }
            Self::DuplicateCue(cue_id) => {
                write!(formatter, "audio catalog defines duplicate cue `{cue_id}`")
            }
            Self::DuplicateGroup(group_id) => {
                write!(
                    formatter,
                    "audio catalog defines duplicate group `{group_id}`"
                )
            }
            Self::MissingCueClipReference { cue_id, clip_id } => write!(
                formatter,
                "audio catalog cue `{cue_id}` references missing clip `{clip_id}`"
            ),
            Self::MissingGroupClipReference { group_id, clip_id } => write!(
                formatter,
                "audio catalog group `{group_id}` references missing clip `{clip_id}`"
            ),
        }
    }
}

impl Error for AudioCatalogConfigError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioCatalogPathError {
    Empty,
    Backslash,
    WindowsDrive,
    Absolute,
    ParentSegment,
    UnsupportedScheme,
}

impl fmt::Display for AudioCatalogPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "path is empty",
            Self::Backslash => "path must use forward slashes",
            Self::WindowsDrive => "path must not contain a Windows drive prefix",
            Self::Absolute => "path must not be absolute",
            Self::ParentSegment => "path must not contain `..` segments",
            Self::UnsupportedScheme => "only content_cache:// URLs are supported",
        })
    }
}

impl Error for AudioCatalogPathError {}

#[derive(Debug)]
pub enum AudioCatalogLoadError {
    CatalogNotFound(String),
    ReadFailed {
        path: PathBuf,
        source: io::Error,
    },
    ParseFailed {
        path: PathBuf,
        source: AudioCatalogConfigError,
    },
}

impl fmt::Display for AudioCatalogLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CatalogNotFound(path) => write!(
                formatter,
                "audio catalog was not found under the first package assets root: {path}"
            ),
            Self::ReadFailed { path, source } => {
                write!(
                    formatter,
                    "failed to read audio catalog at {}: {source}",
                    path.display()
                )
            }
            Self::ParseFailed { path, source } => write!(
                formatter,
                "failed to parse audio catalog RON at {}: {source}",
                path.display()
            ),
        }
    }
}

impl Error for AudioCatalogLoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ReadFailed { source, .. } => Some(source),
            Self::ParseFailed { source, .. } => Some(source),
            Self::CatalogNotFound(_) => None,
        }
    }
}

pub fn load_catalog_from_ron_or_fallback(
    source: &str,
    fallback: AudioCatalog,
) -> Result<AudioCatalog, (AudioCatalogConfigError, AudioCatalog)> {
    AudioCatalogConfig::from_ron_str(source)
        .and_then(AudioCatalogConfig::into_catalog)
        .map_err(|error| (error, fallback))
}

pub fn apply_catalog_config_or_keep_existing(
    catalog: &mut AudioCatalog,
    source: &str,
) -> Result<(), AudioCatalogConfigError> {
    match AudioCatalogConfig::from_ron_str(source).and_then(AudioCatalogConfig::into_catalog) {
        Ok(loaded_catalog) => {
            *catalog = loaded_catalog;
            Ok(())
        }
        Err(error) => Err(error),
    }
}

pub fn load_catalog_from_first_package_ron(
    catalog_path: impl AsRef<str>,
) -> Result<AudioCatalog, AudioCatalogLoadError> {
    let catalog_path = catalog_path.as_ref();
    validate_catalog_file_path(catalog_path).map_err(|source| {
        AudioCatalogLoadError::ParseFailed {
            path: PathBuf::from(catalog_path),
            source,
        }
    })?;

    let fs_path = first_package_catalog_fs_path(catalog_path)
        .ok_or_else(|| AudioCatalogLoadError::CatalogNotFound(catalog_path.to_string()))?;

    let source =
        fs::read_to_string(&fs_path).map_err(|source| AudioCatalogLoadError::ReadFailed {
            path: fs_path.clone(),
            source,
        })?;

    AudioCatalogConfig::from_ron_str(&source)
        .and_then(AudioCatalogConfig::into_catalog)
        .map_err(|source| AudioCatalogLoadError::ParseFailed {
            path: fs_path,
            source,
        })
}

fn parse_clip_id(
    section: &'static str,
    value: &str,
) -> Result<AudioClipId, AudioCatalogConfigError> {
    AudioClipId::try_from(value).map_err(|reason| AudioCatalogConfigError::InvalidClipId {
        section,
        value: value.to_string(),
        reason: reason.to_string(),
    })
}

fn parse_cue_id(section: &'static str, value: &str) -> Result<AudioCueId, AudioCatalogConfigError> {
    AudioCueId::try_from(value).map_err(|reason| AudioCatalogConfigError::InvalidCueId {
        section,
        value: value.to_string(),
        reason: reason.to_string(),
    })
}

fn parse_group_id(
    section: &'static str,
    value: &str,
) -> Result<AudioGroupId, AudioCatalogConfigError> {
    AudioGroupId::try_from(value).map_err(|reason| AudioCatalogConfigError::InvalidGroupId {
        section,
        value: value.to_string(),
        reason: reason.to_string(),
    })
}

fn validate_catalog_file_path(path: &str) -> Result<(), AudioCatalogConfigError> {
    validate_audio_asset_path(path).map_err(|reason| AudioCatalogConfigError::InvalidPath {
        clip_id: "<catalog>".to_string(),
        path: path.to_string(),
        reason,
    })
}

fn validate_audio_asset_path(path: &str) -> Result<(), AudioCatalogPathError> {
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

    let path = Path::new(path);
    if path.components().any(|component| {
        matches!(
            component,
            Component::Prefix(_) | Component::RootDir | Component::ParentDir
        )
    }) {
        return Err(AudioCatalogPathError::ParentSegment);
    }

    Ok(())
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn first_package_catalog_fs_path(catalog_path: &str) -> Option<PathBuf> {
    first_package_asset_root_candidates()
        .into_iter()
        .map(|root| root.join(Path::new(catalog_path)))
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
    use crate::framework::audio::{AudioCatalogError, AudioResolvedGroupClip};

    const VALID_RON: &str = r#"
(
    clips: [
        (id: "ui.click", path: "audio/ui/click.wav"),
        (id: "ui.click_alt", path: "audio/ui/click_alt.wav"),
        (id: "music.title", path: "audio/music/title.ogg"),
    ],
    cues: [
        (
            id: "ui.click",
            clips: [
                (clip: "ui.click", weight: 1.0),
                (clip: "ui.click_alt", weight: 2.0),
            ],
            playback: (bus: ui, scope: (kind: ui), looped: false),
            rules: (
                volume: 0.75,
                pitch: 0.95,
                cooldown_seconds: 0.2,
                max_concurrent: 3,
                priority: 10,
            ),
        ),
        (
            id: "music.title",
            clips: [(clip: "music.title")],
            playback: (bus: music, scope: (kind: global), looped: true),
        ),
    ],
    groups: [
        (
            id: "boot",
            clips: [
                (clip: "ui.click", required: true),
                (clip: "music.title", required: false),
            ],
        ),
    ],
)
"#;

    fn clip_id(value: &str) -> AudioClipId {
        AudioClipId::try_from(value).unwrap()
    }

    fn cue_id(value: &str) -> AudioCueId {
        AudioCueId::try_from(value).unwrap()
    }

    fn group_id(value: &str) -> AudioGroupId {
        AudioGroupId::try_from(value).unwrap()
    }

    #[test]
    fn valid_ron_builds_catalog_with_clips_cues_groups_and_rules() {
        let catalog = AudioCatalogConfig::from_ron_str(VALID_RON)
            .unwrap()
            .into_catalog()
            .unwrap();

        assert_eq!(
            catalog.clip(&clip_id("ui.click")).unwrap(),
            &AudioClipEntry::new("audio/ui/click.wav")
        );

        let cue = catalog.resolve_cue(&cue_id("ui.click")).unwrap();
        assert_eq!(cue.playback.bus, AudioBus::Ui);
        assert_eq!(cue.playback.scope, AudioScope::Ui);
        assert_eq!(cue.rules.volume, 0.75);
        assert_eq!(cue.rules.pitch, 0.95);
        assert_eq!(cue.rules.cooldown_seconds, Some(0.2));
        assert_eq!(cue.rules.max_concurrent, Some(3));
        assert_eq!(cue.rules.priority, 10);
        assert_eq!(cue.clips.len(), 2);
        assert_eq!(cue.clips[0].weight, 1.0);
        assert_eq!(cue.clips[1].weight, 2.0);

        let music = catalog.resolve_cue(&cue_id("music.title")).unwrap();
        assert_eq!(music.playback.bus, AudioBus::Music);
        assert_eq!(music.playback.scope, AudioScope::Global);
        assert!(music.playback.looped);

        assert_eq!(
            catalog.resolve_group(&group_id("boot")).unwrap().clips,
            vec![
                AudioResolvedGroupClip {
                    clip_id: clip_id("ui.click"),
                    path: "audio/ui/click.wav".to_string(),
                    required: true,
                },
                AudioResolvedGroupClip {
                    clip_id: clip_id("music.title"),
                    path: "audio/music/title.ogg".to_string(),
                    required: false,
                },
            ]
        );
    }

    #[test]
    fn content_cache_paths_are_accepted() {
        let catalog = AudioCatalogConfig::from_ron_str(
            r#"
(
    clips: [
        (id: "voice.line_01", path: "content_cache://v1/audio/voice/line_01.ogg"),
    ],
    cues: [
        (id: "voice.line_01", clips: [(clip: "voice.line_01")]),
    ],
)
"#,
        )
        .unwrap()
        .into_catalog()
        .unwrap();

        assert_eq!(
            catalog.resolve_cue(&cue_id("voice.line_01")).unwrap().clips[0].path,
            "content_cache://v1/audio/voice/line_01.ogg"
        );
    }

    #[test]
    fn unsafe_paths_are_rejected() {
        for path in [
            "",
            "audio\\ui\\click.wav",
            "C:/assets/audio/click.wav",
            "/audio/ui/click.wav",
            "audio/../secret.wav",
            "http://example.com/audio/click.wav",
            "file://audio/ui/click.wav",
        ] {
            let config = AudioCatalogConfig {
                clips: vec![AudioClipConfig {
                    id: "ui.click".to_string(),
                    path: path.to_string(),
                }],
                cues: Vec::new(),
                groups: Vec::new(),
            };

            assert!(
                matches!(
                    config.into_catalog(),
                    Err(AudioCatalogConfigError::InvalidPath { .. })
                ),
                "{path} should fail"
            );
        }
    }

    #[test]
    fn invalid_ron_returns_fallback_catalog() {
        let fallback_clip = clip_id("ui.fallback");
        let mut fallback = AudioCatalog::default();
        fallback.register_clip(fallback_clip.clone(), "audio/ui/fallback.wav");

        let (error, catalog) =
            load_catalog_from_ron_or_fallback("not valid ron", fallback).unwrap_err();

        assert!(matches!(error, AudioCatalogConfigError::Ron { .. }));
        assert_eq!(
            catalog.clip(&fallback_clip).unwrap(),
            &AudioClipEntry::new("audio/ui/fallback.wav")
        );
    }

    #[test]
    fn invalid_field_keeps_existing_catalog_when_applying() {
        let fallback_clip = clip_id("ui.fallback");
        let mut catalog = AudioCatalog::default();
        catalog.register_clip(fallback_clip.clone(), "audio/ui/fallback.wav");

        let error = apply_catalog_config_or_keep_existing(
            &mut catalog,
            r#"
(
    clips: [(id: "ui.click", path: "audio/ui/click.wav", unknown: true)],
)
"#,
        )
        .unwrap_err();

        assert!(matches!(error, AudioCatalogConfigError::Ron { .. }));
        assert_eq!(
            catalog.clip(&fallback_clip).unwrap(),
            &AudioClipEntry::new("audio/ui/fallback.wav")
        );
        assert_eq!(
            catalog.clip(&clip_id("ui.click")),
            Err(AudioCatalogError::MissingClip(clip_id("ui.click")))
        );
    }

    #[test]
    fn missing_cue_clip_reference_reports_readable_error() {
        let error = AudioCatalogConfig::from_ron_str(
            r#"
(
    clips: [],
    cues: [
        (id: "ui.click", clips: [(clip: "ui.missing")]),
    ],
)
"#,
        )
        .unwrap()
        .into_catalog()
        .unwrap_err();

        assert_eq!(
            error,
            AudioCatalogConfigError::MissingCueClipReference {
                cue_id: cue_id("ui.click"),
                clip_id: clip_id("ui.missing"),
            }
        );
        assert!(error.to_string().contains("cue `ui.click`"));
        assert!(error.to_string().contains("missing clip `ui.missing`"));
    }

    #[test]
    fn missing_group_clip_reference_reports_readable_error() {
        let error = AudioCatalogConfig::from_ron_str(
            r#"
(
    clips: [],
    groups: [
        (id: "boot", clips: [(clip: "ui.missing")]),
    ],
)
"#,
        )
        .unwrap()
        .into_catalog()
        .unwrap_err();

        assert_eq!(
            error,
            AudioCatalogConfigError::MissingGroupClipReference {
                group_id: group_id("boot"),
                clip_id: clip_id("ui.missing"),
            }
        );
        assert!(error.to_string().contains("group `boot`"));
        assert!(error.to_string().contains("missing clip `ui.missing`"));
    }

    #[test]
    fn scene_and_battle_scopes_are_parsed() {
        let catalog = AudioCatalogConfig::from_ron_str(
            r#"
(
    clips: [
        (id: "ambience.room", path: "audio/ambience/room.ogg"),
        (id: "battle.hit", path: "audio/battle/hit.ogg"),
    ],
    cues: [
        (
            id: "ambience.room",
            clips: [(clip: "ambience.room")],
            playback: (scope: (kind: scene, id: "scene.room")),
        ),
        (
            id: "battle.hit",
            clips: [(clip: "battle.hit")],
            playback: (bus: battle, scope: (kind: battle, id: "battle_01")),
        ),
    ],
)
"#,
        )
        .unwrap()
        .into_catalog()
        .unwrap();

        assert_eq!(
            catalog
                .resolve_cue(&cue_id("ambience.room"))
                .unwrap()
                .playback
                .scope,
            AudioScope::scene("scene.room").unwrap()
        );
        assert_eq!(
            catalog
                .resolve_cue(&cue_id("battle.hit"))
                .unwrap()
                .playback
                .scope,
            AudioScope::battle("battle_01").unwrap()
        );
    }
}
