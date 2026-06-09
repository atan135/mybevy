use bevy::prelude::*;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    env, fs, io,
    path::{Path, PathBuf},
};

const UI_I18N_CONFIG_VERSION: u32 = 1;
const DEFAULT_LOCALE: &str = "zh_cn";
const DEFAULT_I18N_ASSET_DIR: &str = "assets/ui/i18n";
const REPO_ROOT_I18N_ASSET_DIR: &str = "project/assets/ui/i18n";
const UI_I18N_LOCALE_ENV_VAR: &str = "MYBEVY_UI_LOCALE";
const UI_I18N_PATH_ENV_VAR: &str = "MYBEVY_UI_I18N";

pub(in crate::game) struct UiI18nPlugin;

impl Plugin for UiI18nPlugin {
    fn build(&self, app: &mut App) {
        let (i18n, source) = load_ui_i18n();
        app.insert_resource(i18n)
            .insert_resource(source)
            .add_systems(Startup, log_ui_i18n_source);
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
        ("ui_gallery.inputs.section", "输入框"),
        ("ui_gallery.inputs.placeholder.player_name", "玩家名称"),
        ("ui_gallery.inputs.placeholder.note", "输入备注"),
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
