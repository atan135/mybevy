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

pub(in crate::game) struct UiI18nPlugin;

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
                (poll_ui_i18n_hot_reload, refresh_ui_i18n_texts).chain(),
            );
    }
}

#[derive(Clone, Debug, Resource)]
pub(in crate::game) struct UiI18n {
    locale: String,
    texts: HashMap<String, String>,
    fallback_texts: HashMap<String, String>,
}

#[derive(Clone, Debug, Component)]
#[allow(dead_code)]
pub(in crate::game) struct UiI18nText {
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
    pub(in crate::game) fn locale(&self) -> &str {
        &self.locale
    }

    pub(in crate::game) fn tr(&self, key: &str, fallback: impl Into<String>) -> String {
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
    pub(in crate::game) fn text(&self, key: &str) -> String {
        self.tr(key, key)
    }

    fn built_in(locale: impl Into<String>) -> Self {
        Self {
            locale: locale.into(),
            texts: built_in_zh_cn_texts(),
            fallback_texts: built_in_zh_cn_texts(),
        }
    }
}

impl UiI18nText {
    pub(in crate::game) fn new(key: impl Into<String>, fallback: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            fallback: fallback.into(),
        }
    }
}

fn load_ui_i18n() -> (UiI18n, UiI18nSource) {
    let mut diagnostics = Vec::new();
    let fallback_texts = built_in_zh_cn_texts();

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
        UiI18n::built_in(DEFAULT_LOCALE),
        UiI18nSource {
            loaded_path: None,
            diagnostics,
        },
    )
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

    push_locale_candidates(&mut paths, &locale);

    if locale != DEFAULT_LOCALE {
        push_locale_candidates(&mut paths, DEFAULT_LOCALE);
    }

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
        let last_modified = ui_i18n_modified_time(&watched_path).ok();

        Self {
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

    match load_ui_i18n_from_path(&hot_reload.watched_path, built_in_zh_cn_texts()) {
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

fn built_in_zh_cn_texts() -> HashMap<String, String> {
    [
        ("app.name", "MyBevy"),
        ("common.cancel", "取消"),
        ("common.confirm", "确认"),
        ("nav.lobby", "大厅"),
        ("nav.ui_gallery", "UI 示例"),
        ("nav.logout", "退出登录"),
        ("auth.login.subtitle", "玩家登录"),
        ("auth.login.guest_login", "游客登录"),
        ("lobby.title", "游戏列表"),
        ("lobby.available", "可用游戏"),
        ("lobby.touch_ripple.title", "触控水波纹"),
        ("lobby.touch_ripple.description", "当前原型"),
        ("lobby.play", "开始"),
        ("lobby.touch_ripple.confirm.title", "触控水波纹"),
        ("lobby.touch_ripple.confirm.body", "要以单人会话开始吗？"),
        (
            "lobby.touch_ripple.confirm.detail",
            "单人模式仅使用本地控制机。",
        ),
        ("lobby.touch_ripple.confirm.networked", "联机"),
        ("lobby.touch_ripple.confirm.single_player", "单人"),
        ("lobby.touch_ripple.toast.local", "正在启动本地触控水波纹"),
        (
            "lobby.touch_ripple.toast.networked",
            "正在启动联机触控水波纹",
        ),
        ("ui_gallery.title", "UI 示例"),
        ("ui_gallery.typography.section", "文字排版"),
        ("ui_gallery.typography.large_title", "大标题"),
        ("ui_gallery.typography.section_title", "章节标题"),
        ("ui_gallery.typography.subtitle", "副标题文本"),
        ("ui_gallery.typography.body", "正文文本"),
        ("ui_gallery.typography.caption", "说明文本"),
        ("ui_gallery.buttons.section", "按钮"),
        ("ui_gallery.buttons.primary", "主按钮"),
        ("ui_gallery.buttons.secondary", "次按钮"),
        ("ui_gallery.buttons.focused", "聚焦"),
        ("ui_gallery.buttons.selected", "选中"),
        ("ui_gallery.buttons.loading", "加载中"),
        ("ui_gallery.buttons.disabled", "禁用"),
        ("ui_gallery.buttons.unavailable", "不可用"),
        ("ui_gallery.buttons.action", "操作"),
        ("ui_gallery.selection.section", "选择控件"),
        ("ui_gallery.selection.checkbox.unchecked", "未勾选"),
        ("ui_gallery.selection.checkbox.checked", "已勾选"),
        ("ui_gallery.selection.checkbox.disabled", "禁用"),
        ("ui_gallery.selection.toggle.off", "开关关闭"),
        ("ui_gallery.selection.toggle.on", "开关开启"),
        ("ui_gallery.selection.toggle.disabled", "开关禁用"),
        ("ui_gallery.selection.segment.small", "小"),
        ("ui_gallery.selection.segment.medium", "中"),
        ("ui_gallery.selection.segment.large", "大"),
        ("ui_gallery.numeric.section", "数值控件"),
        ("ui_gallery.numeric.slider.volume", "音量"),
        ("ui_gallery.numeric.slider.disabled", "禁用滑条"),
        ("ui_gallery.numeric.stepper.players", "玩家数"),
        ("ui_gallery.numeric.stepper.disabled", "禁用步进器"),
        ("ui_gallery.inputs.section", "输入框"),
        ("ui_gallery.inputs.placeholder.player_name", "玩家名称"),
        ("ui_gallery.inputs.placeholder.required", "必填"),
        ("ui_gallery.inputs.placeholder.error", "错误状态"),
        ("ui_gallery.inputs.placeholder.note", "输入备注"),
        ("ui_gallery.inputs.placeholder.readonly", "只读"),
        ("ui_gallery.inputs.placeholder.disabled", "禁用"),
        ("ui_gallery.inputs.placeholder.short_code", "最多 6 个字符"),
        ("ui_gallery.inputs.placeholder.empty", "空输入"),
        ("ui_gallery.inputs.helper.player_name", "会展示给其他玩家。"),
        ("ui_gallery.inputs.helper.required", "必填字段会校验空值。"),
        ("ui_gallery.inputs.helper.note", "最多 12 个字符。"),
        (
            "ui_gallery.inputs.helper.readonly",
            "只读输入框可聚焦但不能编辑。",
        ),
        (
            "ui_gallery.inputs.helper.short_code",
            "必填，最多 6 个字符。",
        ),
        ("ui_gallery.inputs.helper.empty", "可选的空输入。"),
        ("ui_gallery.inputs.validation.required", "此字段为必填。"),
        (
            "ui_gallery.inputs.validation.error",
            "请输入 4-8 位字母或数字。",
        ),
        (
            "ui_gallery.inputs.validation.disabled_error",
            "禁用态视觉优先于错误态。",
        ),
        ("ui_gallery.overlays.section", "覆盖层"),
        ("ui_gallery.overlays.show_toast", "显示 Toast"),
        ("ui_gallery.overlays.loading", "Loading"),
        ("ui_gallery.overlays.cancelable", "可取消"),
        ("ui_gallery.overlays.hide", "隐藏"),
        ("ui_gallery.overlays.show_confirm", "显示确认框"),
        ("ui_gallery.overlays.show_floating", "显示浮动面板"),
        ("ui_gallery.overlays.close_top", "关闭顶层"),
        ("ui_gallery.toast.preview", "来自 UI 示例的 Toast"),
        ("ui_gallery.loading.preview", "加载预览"),
        ("ui_gallery.loading.cancelable", "可取消加载中"),
        ("ui_gallery.confirm.title", "示例确认框"),
        (
            "ui_gallery.confirm.body",
            "这里用于确认弹窗层级和输入阻断。",
        ),
        (
            "ui_gallery.confirm.detail",
            "弹窗打开时，下方页面按钮不应响应。",
        ),
        ("ui_gallery.floating.title", "浮动面板"),
        ("ui_gallery.floating.body", "此面板不会覆盖整个页面。"),
        (
            "ui_gallery.floating.detail",
            "使用关闭顶层按钮或 Esc 关闭它。",
        ),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), value.to_string()))
    .collect()
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

    fn load_config(source: &str) -> Result<UiI18n, String> {
        let temp = TempConfigDir::new("load_config");
        let path = temp.write_config("i18n.ron", source);
        load_ui_i18n_from_path(&path, built_in_zh_cn_texts())
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
    fn missing_key_falls_back_to_built_in_chinese() {
        let i18n = UiI18n {
            locale: "en_us".to_string(),
            texts: HashMap::new(),
            fallback_texts: built_in_zh_cn_texts(),
        };

        assert_eq!(i18n.tr("common.cancel", "Cancel"), "取消");
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
            fallback_texts: built_in_zh_cn_texts(),
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
    fn hot_reload_keeps_current_i18n_when_updated_file_is_invalid() {
        let temp = TempConfigDir::new("hot_reload_keeps_current_i18n_when_updated_file_is_invalid");
        let path = temp.write_config(
            "i18n.ron",
            &valid_i18n_config_with_version(UI_I18N_CONFIG_VERSION),
        );
        let current_i18n = load_ui_i18n_from_path(&path, built_in_zh_cn_texts()).unwrap();
        let current_app_name = current_i18n.tr("app.name", "Fallback");
        fs::write(&path, "(version: 1, locale:").expect("bad temp config should be written");

        let mut hot_reload = UiI18nHotReload {
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
}
