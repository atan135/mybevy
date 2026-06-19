use bevy::prelude::*;
use serde::Deserialize;
use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

use crate::framework::audio::prelude::AudioClipId;
use crate::framework::scene::prelude::{SceneId, SceneKind, SceneRegistry, SceneSpawnPointId};

const GAME_SCENE_CATALOG_PATH: &str = "game/scenes.csv";

pub(crate) struct GameSceneCatalogPlugin;

impl Plugin for GameSceneCatalogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameSceneCatalog>()
            .configure_sets(Startup, GameSceneCatalogStartupSet)
            .add_systems(
                Startup,
                (load_game_scene_catalog, register_game_scene_catalog)
                    .chain()
                    .in_set(GameSceneCatalogStartupSet),
            );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, SystemSet)]
pub(crate) struct GameSceneCatalogStartupSet;

#[derive(Clone, Debug, Default, Resource)]
pub(crate) struct GameSceneCatalog {
    entries: Vec<GameSceneEntry>,
}

impl GameSceneCatalog {
    pub(crate) fn from_entries(mut entries: Vec<GameSceneEntry>) -> Self {
        entries.sort_by_key(|entry| entry.sort_order);
        Self { entries }
    }

    pub(crate) fn load_first_package_csv(
        catalog_path: impl AsRef<str>,
    ) -> Result<Self, GameSceneCatalogLoadError> {
        let catalog_path = catalog_path.as_ref();
        let fs_path = first_package_catalog_fs_path(catalog_path)
            .ok_or_else(|| GameSceneCatalogLoadError::CatalogNotFound(catalog_path.to_string()))?;

        let csv_source = fs::read_to_string(&fs_path).map_err(|source| {
            GameSceneCatalogLoadError::ReadFailed {
                path: fs_path.clone(),
                source,
            }
        })?;

        Self::from_csv_str(&csv_source).map_err(|source| GameSceneCatalogLoadError::ParseFailed {
            path: fs_path,
            source,
        })
    }

    pub(crate) fn from_csv_str(source: &str) -> Result<Self, GameSceneCatalogParseError> {
        let mut reader = csv::Reader::from_reader(source.as_bytes());
        let mut entries = Vec::new();

        for (index, result) in reader.deserialize::<GameSceneCsvRow>().enumerate() {
            let row = result.map_err(GameSceneCatalogParseError::from)?;
            let row_number = index + 2;
            entries.push(row.into_entry(row_number)?);
        }

        Ok(Self::from_entries(entries))
    }

    pub(crate) fn entries(&self) -> &[GameSceneEntry] {
        &self.entries
    }

    pub(crate) fn enabled_entries(&self) -> impl Iterator<Item = &GameSceneEntry> {
        self.entries.iter().filter(|entry| entry.enabled)
    }

    #[allow(dead_code)]
    pub(crate) fn find_enabled(&self, scene_id: &SceneId) -> Option<&GameSceneEntry> {
        self.enabled_entries()
            .find(|entry| &entry.scene_id == scene_id)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GameSceneEntry {
    pub(crate) scene_id: SceneId,
    pub(crate) enabled: bool,
    pub(crate) sort_order: i32,
    pub(crate) title_key: String,
    pub(crate) title_fallback: String,
    pub(crate) description_key: String,
    pub(crate) description_fallback: String,
    pub(crate) kind: SceneKind,
    pub(crate) manifest_path: String,
    pub(crate) layout_path: Option<String>,
    pub(crate) default_spawn: Option<SceneSpawnPointId>,
    pub(crate) ui_mode: GameSceneUiMode,
    pub(crate) music: Option<GameSceneMusicConfig>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GameSceneMusicConfig {
    pub(crate) clip_id: AudioClipId,
    pub(crate) path: String,
    pub(crate) volume: Option<f32>,
    pub(crate) fade_in_seconds: Option<f32>,
    pub(crate) exit_fade_out_seconds: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum GameSceneUiMode {
    SampleScene,
}

#[derive(Clone, Debug, Deserialize)]
struct GameSceneCsvRow {
    scene_id: String,
    enabled: bool,
    sort_order: i32,
    title_key: String,
    title_fallback: String,
    description_key: String,
    description_fallback: String,
    kind: String,
    manifest_path: String,
    layout_path: String,
    default_spawn: String,
    ui_mode: String,
    #[serde(default)]
    music_clip_id: String,
    #[serde(default)]
    music_path: String,
    #[serde(default)]
    music_volume: String,
    #[serde(default)]
    music_fade_in_seconds: String,
    #[serde(default)]
    music_exit_fade_out_seconds: String,
}

impl GameSceneCsvRow {
    fn into_entry(self, row_number: usize) -> Result<GameSceneEntry, GameSceneCatalogParseError> {
        let scene_id = parse_scene_id(row_number, self.scene_id)?;
        let manifest_path =
            required_trimmed_field(row_number, "manifest_path", self.manifest_path)?;

        Ok(GameSceneEntry {
            scene_id,
            enabled: self.enabled,
            sort_order: self.sort_order,
            title_key: self.title_key.trim().to_string(),
            title_fallback: self.title_fallback.trim().to_string(),
            description_key: self.description_key.trim().to_string(),
            description_fallback: self.description_fallback.trim().to_string(),
            kind: parse_scene_kind(row_number, &self.kind)?,
            manifest_path,
            layout_path: optional_trimmed_field(self.layout_path),
            default_spawn: optional_trimmed_field(self.default_spawn).map(SceneSpawnPointId::from),
            ui_mode: parse_ui_mode(row_number, &self.ui_mode)?,
            music: parse_music_config(
                row_number,
                self.music_clip_id,
                self.music_path,
                self.music_volume,
                self.music_fade_in_seconds,
                self.music_exit_fade_out_seconds,
            )?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GameSceneCatalogParseError {
    Csv(String),
    EmptyField {
        row: usize,
        field: &'static str,
    },
    InvalidSceneId {
        row: usize,
        scene_id: String,
        reason: String,
    },
    InvalidKind {
        row: usize,
        value: String,
    },
    InvalidUiMode {
        row: usize,
        value: String,
    },
    IncompleteMusicConfig {
        row: usize,
        missing_field: &'static str,
    },
    InvalidMusicClipId {
        row: usize,
        value: String,
        reason: String,
    },
    InvalidMusicNumber {
        row: usize,
        field: &'static str,
        value: String,
        reason: String,
    },
}

impl fmt::Display for GameSceneCatalogParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Csv(error) => write!(formatter, "CSV parse failed: {error}"),
            Self::EmptyField { row, field } => {
                write!(
                    formatter,
                    "scene catalog row {row} has empty required field: {field}"
                )
            }
            Self::InvalidSceneId {
                row,
                scene_id,
                reason,
            } => write!(
                formatter,
                "scene catalog row {row} has invalid scene_id `{scene_id}`: {reason}"
            ),
            Self::InvalidKind { row, value } => {
                write!(
                    formatter,
                    "scene catalog row {row} has invalid kind `{value}`"
                )
            }
            Self::InvalidUiMode { row, value } => {
                write!(
                    formatter,
                    "scene catalog row {row} has invalid ui_mode `{value}`"
                )
            }
            Self::IncompleteMusicConfig { row, missing_field } => write!(
                formatter,
                "scene catalog row {row} has incomplete music config: missing required field `{missing_field}`"
            ),
            Self::InvalidMusicClipId { row, value, reason } => write!(
                formatter,
                "scene catalog row {row} has invalid music_clip_id `{value}`: {reason}"
            ),
            Self::InvalidMusicNumber {
                row,
                field,
                value,
                reason,
            } => write!(
                formatter,
                "scene catalog row {row} has invalid {field} `{value}`: {reason}"
            ),
        }
    }
}

impl std::error::Error for GameSceneCatalogParseError {}

#[derive(Debug)]
pub(crate) enum GameSceneCatalogLoadError {
    CatalogNotFound(String),
    ReadFailed {
        path: PathBuf,
        source: io::Error,
    },
    ParseFailed {
        path: PathBuf,
        source: GameSceneCatalogParseError,
    },
}

impl fmt::Display for GameSceneCatalogLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CatalogNotFound(path) => {
                write!(
                    formatter,
                    "scene catalog was not found under the first package assets root: {path}"
                )
            }
            Self::ReadFailed { path, source } => {
                write!(
                    formatter,
                    "failed to read scene catalog at {}: {source}",
                    path.display()
                )
            }
            Self::ParseFailed { path, source } => {
                write!(
                    formatter,
                    "failed to parse scene catalog CSV at {}: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for GameSceneCatalogLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadFailed { source, .. } => Some(source),
            Self::ParseFailed { source, .. } => Some(source),
            Self::CatalogNotFound(_) => None,
        }
    }
}

impl From<csv::Error> for GameSceneCatalogParseError {
    fn from(error: csv::Error) -> Self {
        Self::Csv(error.to_string())
    }
}

fn load_game_scene_catalog(mut catalog: ResMut<GameSceneCatalog>) {
    match GameSceneCatalog::load_first_package_csv(GAME_SCENE_CATALOG_PATH) {
        Ok(loaded_catalog) => {
            let entry_count = loaded_catalog.entries().len();
            let enabled_count = loaded_catalog.enabled_entries().count();
            *catalog = loaded_catalog;
            info!("loaded game scene catalog: {entry_count} entries, {enabled_count} enabled");
        }
        Err(error) => {
            warn!("failed to load game scene catalog; keeping empty catalog: {error}");
        }
    }
}

fn register_game_scene_catalog(
    catalog: Res<GameSceneCatalog>,
    mut registry: ResMut<SceneRegistry>,
) {
    let summary = register_game_scene_catalog_entries(&catalog, &mut registry);

    if summary.registered_count > 0 || summary.failed_count > 0 {
        info!(
            "registered game scene catalog: {} registered, {} skipped, {} failed",
            summary.registered_count, summary.skipped_count, summary.failed_count
        );
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct GameSceneCatalogRegistrationSummary {
    registered_count: usize,
    skipped_count: usize,
    failed_count: usize,
}

fn register_game_scene_catalog_entries(
    catalog: &GameSceneCatalog,
    registry: &mut SceneRegistry,
) -> GameSceneCatalogRegistrationSummary {
    let mut summary = GameSceneCatalogRegistrationSummary::default();

    for entry in catalog.entries() {
        if !entry.enabled {
            summary.skipped_count += 1;
            continue;
        }

        match registry.register_manifest_scene(
            entry.scene_id.clone(),
            entry.kind,
            entry.manifest_path.clone(),
        ) {
            Ok(()) => {
                summary.registered_count += 1;
            }
            Err(error) => {
                summary.failed_count += 1;
                warn!(
                    "failed to register game scene `{}` from catalog manifest `{}`: {error}",
                    entry.scene_id, entry.manifest_path
                );
            }
        }
    }

    summary
}

fn parse_scene_id(row: usize, value: String) -> Result<SceneId, GameSceneCatalogParseError> {
    let value = required_trimmed_field(row, "scene_id", value)?;
    let scene_id = SceneId::from(value.clone());
    scene_id
        .validate()
        .map_err(|error| GameSceneCatalogParseError::InvalidSceneId {
            row,
            scene_id: value,
            reason: error.to_string(),
        })?;
    Ok(scene_id)
}

fn parse_scene_kind(row: usize, value: &str) -> Result<SceneKind, GameSceneCatalogParseError> {
    let normalized = normalize_catalog_token(value);
    match normalized.as_str() {
        "boot" => Ok(SceneKind::Boot),
        "ui" => Ok(SceneKind::Ui),
        "lobby" => Ok(SceneKind::Lobby),
        "gameplay" => Ok(SceneKind::Gameplay),
        "dungeon" => Ok(SceneKind::Dungeon),
        "world" => Ok(SceneKind::World),
        "arena" => Ok(SceneKind::Arena),
        "dev" => Ok(SceneKind::Dev),
        _ => Err(GameSceneCatalogParseError::InvalidKind {
            row,
            value: value.trim().to_string(),
        }),
    }
}

fn parse_ui_mode(row: usize, value: &str) -> Result<GameSceneUiMode, GameSceneCatalogParseError> {
    match normalize_catalog_token(value).as_str() {
        "samplescene" => Ok(GameSceneUiMode::SampleScene),
        _ => Err(GameSceneCatalogParseError::InvalidUiMode {
            row,
            value: value.trim().to_string(),
        }),
    }
}

fn parse_music_config(
    row: usize,
    clip_id: String,
    path: String,
    volume: String,
    fade_in_seconds: String,
    exit_fade_out_seconds: String,
) -> Result<Option<GameSceneMusicConfig>, GameSceneCatalogParseError> {
    let clip_id = optional_trimmed_field(clip_id);
    let path = optional_trimmed_field(path);
    let volume = optional_non_negative_f32(row, "music_volume", volume)?;
    let fade_in_seconds = optional_non_negative_f32(row, "music_fade_in_seconds", fade_in_seconds)?;
    let exit_fade_out_seconds =
        optional_non_negative_f32(row, "music_exit_fade_out_seconds", exit_fade_out_seconds)?;

    if clip_id.is_none()
        && path.is_none()
        && volume.is_none()
        && fade_in_seconds.is_none()
        && exit_fade_out_seconds.is_none()
    {
        return Ok(None);
    }

    let clip_id = clip_id.ok_or(GameSceneCatalogParseError::IncompleteMusicConfig {
        row,
        missing_field: "music_clip_id",
    })?;
    let path = path.ok_or(GameSceneCatalogParseError::IncompleteMusicConfig {
        row,
        missing_field: "music_path",
    })?;
    let clip_id = AudioClipId::try_from(clip_id.clone()).map_err(|reason| {
        GameSceneCatalogParseError::InvalidMusicClipId {
            row,
            value: clip_id,
            reason: reason.to_string(),
        }
    })?;

    Ok(Some(GameSceneMusicConfig {
        clip_id,
        path,
        volume,
        fade_in_seconds,
        exit_fade_out_seconds,
    }))
}

fn optional_non_negative_f32(
    row: usize,
    field: &'static str,
    value: String,
) -> Result<Option<f32>, GameSceneCatalogParseError> {
    let Some(value) = optional_trimmed_field(value) else {
        return Ok(None);
    };

    let parsed =
        value
            .parse::<f32>()
            .map_err(|error| GameSceneCatalogParseError::InvalidMusicNumber {
                row,
                field,
                value: value.clone(),
                reason: error.to_string(),
            })?;

    if !parsed.is_finite() || parsed < 0.0 {
        return Err(GameSceneCatalogParseError::InvalidMusicNumber {
            row,
            field,
            value,
            reason: "value must be a finite number greater than or equal to 0".to_string(),
        });
    }

    Ok(Some(parsed))
}

fn required_trimmed_field(
    row: usize,
    field: &'static str,
    value: String,
) -> Result<String, GameSceneCatalogParseError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        Err(GameSceneCatalogParseError::EmptyField { row, field })
    } else {
        Ok(value)
    }
}

fn optional_trimmed_field(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn normalize_catalog_token(value: &str) -> String {
    value
        .trim()
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| !matches!(character, '-' | '_' | ' '))
        .collect()
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
    use crate::game::scenes::sample_dungeon_room::SAMPLE_DUNGEON_ROOM_SCENE_ID;

    const HEADER: &str = "scene_id,enabled,sort_order,title_key,title_fallback,description_key,description_fallback,kind,manifest_path,layout_path,default_spawn,ui_mode,music_clip_id,music_path,music_volume,music_fade_in_seconds,music_exit_fade_out_seconds\n";

    fn test_entry(scene_id: &str, enabled: bool, sort_order: i32) -> GameSceneEntry {
        GameSceneEntry {
            scene_id: SceneId::from(scene_id),
            enabled,
            sort_order,
            title_key: format!("scene.{scene_id}.title"),
            title_fallback: scene_id.to_string(),
            description_key: format!("scene.{scene_id}.description"),
            description_fallback: "Description".to_string(),
            kind: SceneKind::Dungeon,
            manifest_path: format!("scenes/{scene_id}/scene.ron"),
            layout_path: None,
            default_spawn: None,
            ui_mode: GameSceneUiMode::SampleScene,
            music: None,
        }
    }

    #[test]
    fn parse_catalog_accepts_valid_row() {
        let catalog = GameSceneCatalog::from_csv_str(&format!(
            "{HEADER}{SAMPLE_DUNGEON_ROOM_SCENE_ID},true,100,scene.title,Sample,scene.description,Description,dungeon,scenes/sample/scene.ron,scenes/sample/layout.ron,spawn.default,sample_scene,music.sample,audio/music/sample.wav,0.65,0.25,0.5\n"
        ))
        .unwrap();

        let entry = catalog
            .find_enabled(&SceneId::from(SAMPLE_DUNGEON_ROOM_SCENE_ID))
            .unwrap();
        assert_eq!(entry.scene_id, SceneId::from(SAMPLE_DUNGEON_ROOM_SCENE_ID));
        assert!(entry.enabled);
        assert_eq!(entry.sort_order, 100);
        assert_eq!(entry.kind, SceneKind::Dungeon);
        assert_eq!(entry.manifest_path, "scenes/sample/scene.ron");
        assert_eq!(
            entry.layout_path.as_deref(),
            Some("scenes/sample/layout.ron")
        );
        assert_eq!(
            entry.default_spawn,
            Some(SceneSpawnPointId::from("spawn.default"))
        );
        assert_eq!(entry.ui_mode, GameSceneUiMode::SampleScene);
        assert_eq!(
            entry.music,
            Some(GameSceneMusicConfig {
                clip_id: AudioClipId::try_from("music.sample").unwrap(),
                path: "audio/music/sample.wav".to_string(),
                volume: Some(0.65),
                fade_in_seconds: Some(0.25),
                exit_fade_out_seconds: Some(0.5),
            })
        );
    }

    #[test]
    fn register_catalog_entries_registers_enabled_manifest_scenes() {
        let catalog = GameSceneCatalog::from_entries(vec![test_entry(
            SAMPLE_DUNGEON_ROOM_SCENE_ID,
            true,
            100,
        )]);
        let mut registry = SceneRegistry::default();

        let summary = register_game_scene_catalog_entries(&catalog, &mut registry);

        assert_eq!(
            summary,
            GameSceneCatalogRegistrationSummary {
                registered_count: 1,
                skipped_count: 0,
                failed_count: 0
            }
        );

        let definition = registry
            .get(&SceneId::from(SAMPLE_DUNGEON_ROOM_SCENE_ID))
            .unwrap();
        assert_eq!(definition.kind, SceneKind::Dungeon);
        assert_eq!(
            definition.manifest_path.as_deref(),
            Some("scenes/sample.dungeon_room/scene.ron")
        );
    }

    #[test]
    fn register_catalog_entries_skips_disabled_scenes() {
        let catalog = GameSceneCatalog::from_entries(vec![test_entry(
            SAMPLE_DUNGEON_ROOM_SCENE_ID,
            false,
            100,
        )]);
        let mut registry = SceneRegistry::default();

        let summary = register_game_scene_catalog_entries(&catalog, &mut registry);

        assert_eq!(
            summary,
            GameSceneCatalogRegistrationSummary {
                registered_count: 0,
                skipped_count: 1,
                failed_count: 0
            }
        );
        assert!(!registry.contains(&SceneId::from(SAMPLE_DUNGEON_ROOM_SCENE_ID)));
    }

    #[test]
    fn register_catalog_entries_warns_and_keeps_existing_scene_on_duplicate() {
        let catalog = GameSceneCatalog::from_entries(vec![test_entry(
            SAMPLE_DUNGEON_ROOM_SCENE_ID,
            true,
            100,
        )]);
        let mut registry = SceneRegistry::default();
        registry
            .register_manifest_scene(
                SAMPLE_DUNGEON_ROOM_SCENE_ID,
                SceneKind::Dev,
                "scenes/existing/scene.ron",
            )
            .unwrap();

        let summary = register_game_scene_catalog_entries(&catalog, &mut registry);

        assert_eq!(
            summary,
            GameSceneCatalogRegistrationSummary {
                registered_count: 0,
                skipped_count: 0,
                failed_count: 1
            }
        );

        let definition = registry
            .get(&SceneId::from(SAMPLE_DUNGEON_ROOM_SCENE_ID))
            .unwrap();
        assert_eq!(registry.len(), 1);
        assert_eq!(definition.kind, SceneKind::Dev);
        assert_eq!(
            definition.manifest_path.as_deref(),
            Some("scenes/existing/scene.ron")
        );
    }

    #[test]
    fn disabled_row_is_not_returned_by_enabled_queries() {
        let catalog = GameSceneCatalog::from_csv_str(&format!(
            "{HEADER}disabled.scene,false,10,scene.title,Sample,scene.description,Description,dungeon,scenes/sample/scene.ron,,,sample_scene,,,,,\n"
        ))
        .unwrap();

        assert_eq!(catalog.entries().len(), 1);
        assert_eq!(catalog.enabled_entries().count(), 0);
        assert!(
            catalog
                .find_enabled(&SceneId::from("disabled.scene"))
                .is_none()
        );
    }

    #[test]
    fn parse_catalog_keeps_empty_music_config_optional() {
        let catalog = GameSceneCatalog::from_csv_str(&format!(
            "{HEADER}sample.scene,true,100,scene.title,Sample,scene.description,Description,dungeon,scenes/sample/scene.ron,,,sample_scene,,,,,\n"
        ))
        .unwrap();

        assert_eq!(catalog.entries()[0].music, None);
    }

    #[test]
    fn parse_catalog_rejects_partial_music_config() {
        let error = GameSceneCatalog::from_csv_str(&format!(
            "{HEADER}sample.scene,true,100,scene.title,Sample,scene.description,Description,dungeon,scenes/sample/scene.ron,,,sample_scene,music.sample,,,,\n"
        ))
        .unwrap_err();

        assert_eq!(
            error,
            GameSceneCatalogParseError::IncompleteMusicConfig {
                row: 2,
                missing_field: "music_path"
            }
        );
    }

    #[test]
    fn parse_catalog_rejects_invalid_music_number() {
        let error = GameSceneCatalog::from_csv_str(&format!(
            "{HEADER}sample.scene,true,100,scene.title,Sample,scene.description,Description,dungeon,scenes/sample/scene.ron,,,sample_scene,music.sample,audio/music/sample.wav,-0.1,,\n"
        ))
        .unwrap_err();

        assert_eq!(
            error,
            GameSceneCatalogParseError::InvalidMusicNumber {
                row: 2,
                field: "music_volume",
                value: "-0.1".to_string(),
                reason: "value must be a finite number greater than or equal to 0".to_string(),
            }
        );
    }

    #[test]
    fn parse_catalog_sorts_entries_by_sort_order() {
        let catalog = GameSceneCatalog::from_csv_str(&format!(
            "{HEADER}second.scene,true,20,scene.title,Second,scene.description,Description,dungeon,scenes/second/scene.ron,,,sample_scene,,,,,\nfirst.scene,true,10,scene.title,First,scene.description,Description,dungeon,scenes/first/scene.ron,,,sample_scene,,,,,\n"
        ))
        .unwrap();

        let scene_ids = catalog
            .enabled_entries()
            .map(|entry| entry.scene_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(scene_ids, vec!["first.scene", "second.scene"]);
    }

    #[test]
    fn parse_catalog_rejects_invalid_kind() {
        let error = GameSceneCatalog::from_csv_str(&format!(
            "{HEADER}sample.scene,true,100,scene.title,Sample,scene.description,Description,unknown,scenes/sample/scene.ron,,,sample_scene,,,,,\n"
        ))
        .unwrap_err();

        assert_eq!(
            error,
            GameSceneCatalogParseError::InvalidKind {
                row: 2,
                value: "unknown".to_string()
            }
        );
    }

    #[test]
    fn parse_catalog_rejects_invalid_ui_mode() {
        let error = GameSceneCatalog::from_csv_str(&format!(
            "{HEADER}sample.scene,true,100,scene.title,Sample,scene.description,Description,dungeon,scenes/sample/scene.ron,,,unknown,,,,,\n"
        ))
        .unwrap_err();

        assert_eq!(
            error,
            GameSceneCatalogParseError::InvalidUiMode {
                row: 2,
                value: "unknown".to_string()
            }
        );
    }

    #[test]
    fn parse_catalog_rejects_empty_manifest_path() {
        let error = GameSceneCatalog::from_csv_str(&format!(
            "{HEADER}sample.scene,true,100,scene.title,Sample,scene.description,Description,dungeon,,,,sample_scene,,,,,\n"
        ))
        .unwrap_err();

        assert_eq!(
            error,
            GameSceneCatalogParseError::EmptyField {
                row: 2,
                field: "manifest_path"
            }
        );
    }

    #[test]
    fn parse_catalog_rejects_invalid_scene_id() {
        let error = GameSceneCatalog::from_csv_str(&format!(
            "{HEADER}Sample.Scene,true,100,scene.title,Sample,scene.description,Description,dungeon,scenes/sample/scene.ron,,,sample_scene,,,,,\n"
        ))
        .unwrap_err();

        assert!(matches!(
            error,
            GameSceneCatalogParseError::InvalidSceneId { row: 2, .. }
        ));
    }
}
