use bevy::prelude::*;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    env, fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

const UI_I18N_CONFIG_VERSION: u32 = 1;
const DEFAULT_LOCALE: &str = "zh_cn";
const DEFAULT_I18N_ASSET_DIR: &str = "assets/ui/i18n";
const REPO_ROOT_I18N_ASSET_DIR: &str = "project/assets/ui/i18n";
const UI_I18N_LOCALE_ENV_VAR: &str = "MYBEVY_UI_LOCALE";
const UI_I18N_PATH_ENV_VAR: &str = "MYBEVY_UI_I18N";
const UI_I18N_HOT_RELOAD_INTERVAL_SECS: f32 = 0.8;

pub(crate) struct UiI18nPlugin;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(crate) enum UiI18nSystems {
    Refresh,
}

impl Plugin for UiI18nPlugin {
    fn build(&self, app: &mut App) {
        let (i18n, source) = load_ui_i18n();
        let hot_reload = UiI18nHotReload::new(&source);
        app.insert_resource(i18n)
            .insert_resource(source)
            .insert_resource(hot_reload)
            .add_systems(Startup, log_ui_i18n_source)
            .add_systems(
                Update,
                (poll_ui_i18n_hot_reload, refresh_ui_i18n_texts)
                    .chain()
                    .in_set(UiI18nSystems::Refresh),
            );
    }
}

#[derive(Clone, Debug, Resource)]
pub(crate) struct UiI18n {
    locale: String,
    texts: HashMap<String, String>,
    fallback_texts: HashMap<String, String>,
}

#[derive(Clone, Debug, Component)]
#[allow(dead_code)]
pub(crate) struct UiI18nText {
    pub key: String,
    pub fallback: String,
}

#[derive(Clone, Debug, Resource)]
struct UiI18nSource {
    loaded_path: Option<PathBuf>,
    diagnostics: Vec<String>,
}

#[derive(Debug, Resource)]
struct UiI18nHotReload {
    enabled: bool,
    watched_path: PathBuf,
    last_modified: Option<SystemTime>,
    poll_timer: Timer,
    last_error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UiI18nConfig {
    version: u32,
    locale: String,
    texts: HashMap<String, String>,
}

impl UiI18n {
    pub(crate) fn locale(&self) -> &str {
        &self.locale
    }

    pub(crate) fn tr(&self, key: &str, fallback: impl Into<String>) -> String {
        if let Some(text) = self.texts.get(key) {
            return text.clone();
        }

        if let Some(text) = self.fallback_texts.get(key) {
            warn!(
                key,
                locale = %self.locale,
                fallback = %text,
                "missing ui i18n text key; using built-in fallback"
            );
            return text.clone();
        }

        let fallback = fallback.into();
        warn!(
            key,
            locale = %self.locale,
            fallback = %fallback,
            "missing ui i18n text key"
        );

        if fallback.is_empty() {
            key.to_string()
        } else {
            fallback
        }
    }

    #[allow(dead_code)]
    pub(crate) fn text(&self, key: &str) -> String {
        self.tr(key, key)
    }

    fn from_fallback(locale: impl Into<String>, fallback_texts: HashMap<String, String>) -> Self {
        Self {
            locale: locale.into(),
            texts: fallback_texts.clone(),
            fallback_texts,
        }
    }

    #[cfg(test)]
    pub(crate) fn test_with_texts(locale: &str, entries: &[(&str, &str)]) -> Self {
        let texts = entries
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect::<HashMap<_, _>>();
        Self {
            locale: locale.to_string(),
            fallback_texts: texts.clone(),
            texts,
        }
    }
}

impl crate::framework::ui::document::UiDocumentI18nCatalog for UiI18n {
    fn lookup(&self, key: &crate::framework::ui::document::UiI18nKey) -> Option<&str> {
        self.texts
            .get(key.as_str())
            .or_else(|| self.fallback_texts.get(key.as_str()))
            .map(String::as_str)
    }
}

impl UiI18nText {
    pub(crate) fn new(key: impl Into<String>, fallback: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            fallback: fallback.into(),
        }
    }
}

fn load_ui_i18n() -> (UiI18n, UiI18nSource) {
    let mut diagnostics = Vec::new();
    let fallback_texts = load_default_fallback_texts(&mut diagnostics);

    for path in ui_i18n_path_candidates() {
        match load_ui_i18n_from_path(&path, fallback_texts.clone()) {
            Ok(i18n) => {
                return (
                    i18n,
                    UiI18nSource {
                        loaded_path: Some(path),
                        diagnostics,
                    },
                );
            }
            Err(error) => diagnostics.push(error),
        }
    }

    (
        UiI18n::from_fallback(DEFAULT_LOCALE, fallback_texts),
        UiI18nSource {
            loaded_path: None,
            diagnostics,
        },
    )
}

fn load_default_fallback_texts(diagnostics: &mut Vec<String>) -> HashMap<String, String> {
    load_fallback_texts_from_paths(locale_path_candidates(DEFAULT_LOCALE), diagnostics)
}

fn load_fallback_texts_from_paths(
    paths: impl IntoIterator<Item = PathBuf>,
    diagnostics: &mut Vec<String>,
) -> HashMap<String, String> {
    for path in paths {
        match load_ui_i18n_from_path(&path, HashMap::new()) {
            Ok(i18n) if i18n.locale() == DEFAULT_LOCALE => return i18n.texts,
            Ok(i18n) => diagnostics.push(format!(
                "default locale fallback: {} declares locale {}, expected {}",
                path.display(),
                i18n.locale(),
                DEFAULT_LOCALE
            )),
            Err(error) => diagnostics.push(format!("default locale fallback: {error}")),
        }
    }

    HashMap::new()
}

fn load_ui_i18n_from_path(
    path: &Path,
    fallback_texts: HashMap<String, String>,
) -> Result<UiI18n, String> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(format!("{} not found", path.display()));
        }
        Err(error) => {
            return Err(format!("{} could not be read: {error}", path.display()));
        }
    };

    match ron::from_str::<UiI18nConfig>(&source) {
        Ok(config) if config.version == UI_I18N_CONFIG_VERSION => Ok(UiI18n {
            locale: normalize_locale(&config.locale),
            texts: config.texts,
            fallback_texts,
        }),
        Ok(config) => Err(format!(
            "{} uses unsupported version {}, expected {}",
            path.display(),
            config.version,
            UI_I18N_CONFIG_VERSION
        )),
        Err(error) => Err(format!("{} could not be parsed: {error}", path.display())),
    }
}

fn ui_i18n_path_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let locale = preferred_locale();

    if let Ok(path) = env::var(UI_I18N_PATH_ENV_VAR) {
        push_unique_path(&mut paths, PathBuf::from(path));
    }

    for path in locale_path_candidates(&locale) {
        push_unique_path(&mut paths, path);
    }

    if locale != DEFAULT_LOCALE {
        for path in locale_path_candidates(DEFAULT_LOCALE) {
            push_unique_path(&mut paths, path);
        }
    }

    paths
}

fn locale_path_candidates(locale: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    push_locale_candidates(&mut paths, locale);
    paths
}

fn push_locale_candidates(paths: &mut Vec<PathBuf>, locale: &str) {
    let file_name = format!("{locale}.ron");
    push_unique_path(
        paths,
        PathBuf::from(DEFAULT_I18N_ASSET_DIR).join(file_name.as_str()),
    );
    push_unique_path(
        paths,
        PathBuf::from(REPO_ROOT_I18N_ASSET_DIR).join(file_name.as_str()),
    );
    push_unique_path(
        paths,
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(DEFAULT_I18N_ASSET_DIR)
            .join(file_name),
    );
}

fn preferred_locale() -> String {
    env::var(UI_I18N_LOCALE_ENV_VAR)
        .ok()
        .map(|locale| normalize_locale(&locale))
        .filter(|locale| !locale.is_empty())
        .unwrap_or_else(|| DEFAULT_LOCALE.to_string())
}

fn normalize_locale(locale: &str) -> String {
    locale.trim().to_ascii_lowercase().replace('-', "_")
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| same_path(existing, &path)) {
        paths.push(path);
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }

    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn log_ui_i18n_source(source: Res<UiI18nSource>, i18n: Res<UiI18n>) {
    if let Some(path) = &source.loaded_path {
        info!(
            path = %path.display(),
            locale = %i18n.locale(),
            "loaded ui i18n config"
        );
    } else if source.diagnostics.is_empty() {
        info!(locale = %i18n.locale(), "using built-in ui i18n");
    } else {
        warn!(
            diagnostics = ?source.diagnostics,
            locale = %i18n.locale(),
            "using built-in ui i18n fallback"
        );
    }
}

impl UiI18nHotReload {
    fn new(source: &UiI18nSource) -> Self {
        let watched_path = source
            .loaded_path
            .clone()
            .unwrap_or_else(preferred_ui_i18n_watch_path);
        let enabled = source.loaded_path.is_some();
        let last_modified = enabled
            .then(|| ui_i18n_modified_time(&watched_path).ok())
            .flatten();

        Self {
            enabled,
            watched_path,
            last_modified,
            poll_timer: Timer::from_seconds(UI_I18N_HOT_RELOAD_INTERVAL_SECS, TimerMode::Repeating),
            last_error: None,
        }
    }
}

fn preferred_ui_i18n_watch_path() -> PathBuf {
    if let Ok(path) = env::var(UI_I18N_PATH_ENV_VAR) {
        return PathBuf::from(path);
    }

    ui_i18n_path_candidates()
        .into_iter()
        .find(|path| path.exists())
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(DEFAULT_I18N_ASSET_DIR)
                .join(format!("{}.ron", preferred_locale()))
        })
}

fn ui_i18n_modified_time(path: &Path) -> io::Result<SystemTime> {
    fs::metadata(path).and_then(|metadata| metadata.modified())
}

fn poll_ui_i18n_hot_reload(
    time: Res<Time>,
    mut i18n: ResMut<UiI18n>,
    mut source: ResMut<UiI18nSource>,
    mut hot_reload: ResMut<UiI18nHotReload>,
) {
    if !hot_reload.enabled {
        return;
    }

    if !hot_reload.poll_timer.tick(time.delta()).just_finished() {
        return;
    }

    let modified = match ui_i18n_modified_time(&hot_reload.watched_path) {
        Ok(modified) => modified,
        Err(error) => {
            let message = format!(
                "{} could not be stat'ed: {error}",
                hot_reload.watched_path.display()
            );
            warn_ui_i18n_reload_error(&mut hot_reload, message);
            return;
        }
    };

    if hot_reload.last_modified == Some(modified) && hot_reload.last_error.is_none() {
        return;
    }

    let fallback_texts = i18n.fallback_texts.clone();
    match load_ui_i18n_from_path(&hot_reload.watched_path, fallback_texts) {
        Ok(next_i18n) => {
            *i18n = next_i18n;
            source.loaded_path = Some(hot_reload.watched_path.clone());
            source.diagnostics.clear();
            hot_reload.last_modified = Some(modified);
            hot_reload.last_error = None;
            info!(
                path = %hot_reload.watched_path.display(),
                locale = %i18n.locale(),
                "hot reloaded ui i18n config"
            );
        }
        Err(error) => {
            warn_ui_i18n_reload_error(&mut hot_reload, error);
        }
    }
}

fn warn_ui_i18n_reload_error(hot_reload: &mut UiI18nHotReload, error: String) {
    if hot_reload.last_error.as_deref() != Some(error.as_str()) {
        warn!(
            path = %hot_reload.watched_path.display(),
            error = %error,
            "failed to hot reload ui i18n config; keeping current i18n"
        );
    }

    hot_reload.last_error = Some(error);
}

fn refresh_ui_i18n_texts(i18n: Res<UiI18n>, mut texts: Query<(&UiI18nText, &mut Text)>) {
    if !i18n.is_changed() {
        return;
    }

    for (i18n_text, mut text) in &mut texts {
        let next_text = i18n.tr(&i18n_text.key, i18n_text.fallback.clone());
        if text.0 != next_text {
            text.0 = next_text;
        }
    }
}

#[allow(dead_code)]
fn missing_keys_for_locale(i18n: &UiI18n) -> HashSet<&str> {
    i18n.fallback_texts
        .keys()
        .filter_map(|key| {
            if i18n.texts.contains_key(key) {
                None
            } else {
                Some(key.as_str())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::HashMap,
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct TempConfigDir {
        path: PathBuf,
    }

    impl TempConfigDir {
        fn new(test_name: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos();
            let path = env::temp_dir().join(format!(
                "mybevy-i18n-tests-{}-{unique}",
                test_name.replace("::", "-")
            ));
            fs::create_dir(&path).expect("temp test directory should be created");
            Self { path }
        }

        fn write_config(&self, file_name: &str, source: &str) -> PathBuf {
            let path = self.path.join(file_name);
            fs::write(&path, source).expect("temp config should be written");
            path
        }
    }

    impl Drop for TempConfigDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.path).ok();
        }
    }

    fn valid_i18n_config_with_version(version: u32) -> String {
        format!(
            r#"(
    version: {version},
    locale: " EN-US ",
    texts: {{
        "app.name": "Custom App",
        "custom.key": "Custom Text",
    }},
)"#
        )
    }

    fn test_fallback_texts() -> HashMap<String, String> {
        HashMap::from([("common.cancel".to_string(), "取消".to_string())])
    }

    fn load_config(source: &str) -> Result<UiI18n, String> {
        let temp = TempConfigDir::new("load_config");
        let path = temp.write_config("i18n.ron", source);
        load_ui_i18n_from_path(&path, test_fallback_texts())
    }

    fn assert_error_contains(error: &str, expected: &str) {
        assert!(
            error.contains(expected),
            "expected error to contain {expected:?}, got {error:?}"
        );
    }

    #[test]
    fn normalizes_locale_names() {
        assert_eq!(normalize_locale(" EN-US "), "en_us");
        assert_eq!(normalize_locale("zh_CN"), "zh_cn");
    }

    #[test]
    fn parses_valid_ron_i18n_config() {
        let i18n = load_config(&valid_i18n_config_with_version(UI_I18N_CONFIG_VERSION)).unwrap();

        assert_eq!(i18n.locale(), "en_us");
        assert_eq!(i18n.tr("app.name", "Fallback"), "Custom App");
        assert_eq!(i18n.tr("custom.key", "Fallback"), "Custom Text");
    }

    #[test]
    fn rejects_unsupported_i18n_config_version() {
        let error =
            load_config(&valid_i18n_config_with_version(UI_I18N_CONFIG_VERSION + 1)).unwrap_err();

        assert_error_contains(&error, "uses unsupported version 2, expected 1");
    }

    #[test]
    fn reports_bad_ron_i18n_config_as_parse_error() {
        let error = load_config("(version: 1, locale:").unwrap_err();

        assert_error_contains(&error, "could not be parsed");
    }

    #[test]
    fn missing_key_falls_back_to_loaded_default_locale() {
        let i18n = UiI18n {
            locale: "en_us".to_string(),
            texts: HashMap::new(),
            fallback_texts: test_fallback_texts(),
        };

        assert_eq!(i18n.tr("common.cancel", "Cancel"), "取消");
    }

    #[test]
    fn default_fallback_texts_are_loaded_from_ron() {
        let temp = TempConfigDir::new("default_fallback_texts_are_loaded_from_ron");
        let missing = temp.path.join("missing.ron");
        let zh_cn = temp.write_config(
            "zh_cn.ron",
            r#"(
    version: 1,
    locale: "zh_cn",
    texts: {
        "new.translation": "无需重新编译",
    },
)"#,
        );
        let mut diagnostics = Vec::new();

        let texts = load_fallback_texts_from_paths([missing, zh_cn], &mut diagnostics);

        assert_eq!(
            texts.get("new.translation").map(String::as_str),
            Some("无需重新编译")
        );
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn empty_missing_key_fallback_displays_key() {
        let i18n = UiI18n {
            locale: "en_us".to_string(),
            texts: HashMap::new(),
            fallback_texts: HashMap::new(),
        };

        assert_eq!(i18n.tr("missing.key", ""), "missing.key");
    }

    #[test]
    fn refresh_i18n_texts_updates_marked_text_nodes() {
        let mut texts = HashMap::new();
        texts.insert("app.name".to_string(), "Runtime App".to_string());
        let i18n = UiI18n {
            locale: "en_us".to_string(),
            texts,
            fallback_texts: test_fallback_texts(),
        };
        let mut app = App::new();
        app.insert_resource(i18n)
            .add_systems(Update, refresh_ui_i18n_texts);
        let entity = app
            .world_mut()
            .spawn((
                Text::new("Old App"),
                UiI18nText::new("app.name", "Fallback"),
            ))
            .id();

        app.update();

        let text = app.world().entity(entity).get::<Text>().unwrap();
        assert_eq!(text.0, "Runtime App");
    }

    #[test]
    fn i18n_refresh_preserves_font_role_layout_and_node_constraints() {
        use crate::framework::ui::style::{
            UiFontWeight, UiTextAlignment, UiTextLineHeight, UiTextStyleToken, UiTextTruncation,
            UiTextWrap, theme::UiThemeTextStyleRole,
        };

        let mut texts = HashMap::new();
        texts.insert(
            "ui_gallery.typography.body".to_string(),
            "Runtime body text".to_string(),
        );
        let i18n = UiI18n {
            locale: "en_us".to_string(),
            texts,
            fallback_texts: test_fallback_texts(),
        };
        let mut style = UiTextStyleToken::latin_fixture(UiFontWeight::Medium, 18.0);
        style.line_height = UiTextLineHeight::Relative(1.45);
        style.alignment = UiTextAlignment::Center;
        style.wrap = UiTextWrap::WordOrCharacter;
        style.truncation = UiTextTruncation::Ellipsis { max_graphemes: 12 };
        let expected_style = style.clone();
        let expected_layout = TextLayout::new(Justify::Center, LineBreak::WordOrCharacter);
        let mut app = App::new();
        app.insert_resource(i18n)
            .add_systems(Update, refresh_ui_i18n_texts);
        let entity = app
            .world_mut()
            .spawn((
                Text::new("Old body text"),
                UiI18nText::new("ui_gallery.typography.body", "Fallback body"),
                style,
                UiThemeTextStyleRole::Body,
                expected_layout,
                bevy::text::LineHeight::RelativeToFont(1.45),
                Node {
                    max_width: px(240),
                    overflow: Overflow::clip(),
                    ..default()
                },
            ))
            .id();

        app.update();

        let entity_ref = app.world().entity(entity);
        assert_eq!(entity_ref.get::<Text>().unwrap().0, "Runtime body text");
        assert_eq!(
            entity_ref.get::<UiTextStyleToken>().unwrap(),
            &expected_style
        );
        assert!(entity_ref.contains::<UiThemeTextStyleRole>());
        let layout = entity_ref.get::<TextLayout>().unwrap();
        assert_eq!(layout.justify, Justify::Center);
        assert_eq!(layout.linebreak, LineBreak::WordOrCharacter);
        assert_eq!(
            entity_ref.get::<bevy::text::LineHeight>().unwrap(),
            &bevy::text::LineHeight::RelativeToFont(1.45)
        );
        let node = entity_ref.get::<Node>().unwrap();
        assert_eq!(node.max_width, px(240));
        assert_eq!(node.overflow, Overflow::clip());
    }

    #[test]
    fn hot_reload_keeps_current_i18n_when_updated_file_is_invalid() {
        let temp = TempConfigDir::new("hot_reload_keeps_current_i18n_when_updated_file_is_invalid");
        let path = temp.write_config(
            "i18n.ron",
            &valid_i18n_config_with_version(UI_I18N_CONFIG_VERSION),
        );
        let current_i18n = load_ui_i18n_from_path(&path, test_fallback_texts()).unwrap();
        let current_app_name = current_i18n.tr("app.name", "Fallback");
        fs::write(&path, "(version: 1, locale:").expect("bad temp config should be written");

        let mut hot_reload = UiI18nHotReload {
            enabled: true,
            watched_path: path,
            last_modified: None,
            poll_timer: Timer::from_seconds(0.0, TimerMode::Repeating),
            last_error: None,
        };
        hot_reload.poll_timer.tick(std::time::Duration::ZERO);
        let source = UiI18nSource {
            loaded_path: Some(hot_reload.watched_path.clone()),
            diagnostics: Vec::new(),
        };
        let mut app = App::new();
        app.insert_resource(current_i18n)
            .insert_resource(source)
            .insert_resource(hot_reload)
            .insert_resource(Time::<()>::default())
            .add_systems(Update, poll_ui_i18n_hot_reload);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));

        app.update();

        let i18n = app.world().resource::<UiI18n>();
        assert_eq!(i18n.tr("app.name", "Fallback"), current_app_name);
    }

    #[test]
    fn disabled_hot_reload_keeps_loaded_fallback_without_polling_missing_path() {
        let missing_path = PathBuf::from("missing/i18n.ron");
        let mut hot_reload = UiI18nHotReload {
            enabled: false,
            watched_path: missing_path,
            last_modified: None,
            poll_timer: Timer::from_seconds(0.0, TimerMode::Repeating),
            last_error: None,
        };
        hot_reload.poll_timer.tick(std::time::Duration::ZERO);
        let source = UiI18nSource {
            loaded_path: None,
            diagnostics: Vec::new(),
        };
        let mut app = App::new();
        app.insert_resource(UiI18n::from_fallback(DEFAULT_LOCALE, test_fallback_texts()))
            .insert_resource(source)
            .insert_resource(hot_reload)
            .insert_resource(Time::<()>::default())
            .add_systems(Update, poll_ui_i18n_hot_reload);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));

        app.update();

        let hot_reload = app.world().resource::<UiI18nHotReload>();
        assert!(hot_reload.last_error.is_none());
    }
}
