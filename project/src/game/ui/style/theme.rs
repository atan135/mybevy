use bevy::prelude::*;
use serde::Deserialize;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

const UI_THEME_CONFIG_VERSION: u32 = 1;
const DEFAULT_THEME_ASSET_PATH: &str = "assets/ui/themes/default.ron";
const REPO_ROOT_THEME_ASSET_PATH: &str = "project/assets/ui/themes/default.ron";
const UI_THEME_ENV_VAR: &str = "MYBEVY_UI_THEME";
const UI_THEME_HOT_RELOAD_INTERVAL_SECS: f32 = 0.8;

pub(in crate::game) struct UiThemePlugin;

impl Plugin for UiThemePlugin {
    fn build(&self, app: &mut App) {
        let (theme, source) = load_ui_theme();
        let hot_reload = UiThemeHotReload::new(&source);
        app.insert_resource(theme)
            .insert_resource(source)
            .insert_resource(hot_reload)
            .add_systems(Startup, log_ui_theme_source)
            .add_systems(
                Update,
                (poll_ui_theme_hot_reload, refresh_ui_theme_visuals).chain(),
            );
    }
}

#[derive(Clone, Debug, Resource)]
pub(in crate::game) struct UiTheme {
    pub colors: UiColors,
    pub text: UiTextTheme,
    pub layout: UiLayoutTheme,
    pub button: UiButtonTheme,
    pub panel: UiPanelTheme,
}

#[derive(Clone, Debug)]
pub(in crate::game) struct UiColors {
    pub screen_background: Color,
    pub panel_background: Color,
    pub panel_border: Color,
    pub text_primary: Color,
    pub text_muted: Color,
    pub primary_button: ButtonColors,
    pub secondary_button: ButtonColors,
}

#[derive(Clone, Debug, Deserialize)]
pub(in crate::game) struct UiTextTheme {
    pub title_large: f32,
    pub title: f32,
    pub subtitle: f32,
    pub section_label: f32,
    pub body: f32,
    pub caption: f32,
    pub button: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub(in crate::game) struct UiLayoutTheme {
    pub screen_padding: f32,
    pub overlay_padding: f32,
    pub page_gap: f32,
    pub panel_gap: f32,
    pub card_gap: f32,
    pub header_gap: f32,
    pub row_gap: f32,
    pub row_padding_y: f32,
    pub row_column_gap: f32,
    pub auth_panel_width: f32,
    pub content_width: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub(in crate::game) struct UiButtonTheme {
    pub min_width: f32,
    pub height: f32,
    pub padding_x: f32,
    pub radius: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub(in crate::game) struct UiPanelTheme {
    pub padding: f32,
    pub border: f32,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::game) struct ButtonColors {
    pub idle: Color,
    pub hovered: Color,
    pub pressed: Color,
    pub focused: Color,
    pub selected: Color,
    pub disabled: Color,
    pub loading: Color,
}

#[derive(Clone, Copy, Debug, Component)]
pub(in crate::game) enum UiThemeBackgroundRole {
    Screen,
    Panel,
}

#[derive(Clone, Copy, Debug, Component)]
pub(in crate::game) enum UiThemeBorderRole {
    Panel,
}

#[derive(Clone, Copy, Debug, Component)]
pub(in crate::game) enum UiThemeTextColorRole {
    Primary,
    Muted,
}

impl UiThemeBackgroundRole {
    fn color(self, theme: &UiTheme) -> Color {
        match self {
            Self::Screen => theme.colors.screen_background,
            Self::Panel => theme.colors.panel_background,
        }
    }
}

impl UiThemeBorderRole {
    fn color(self, theme: &UiTheme) -> Color {
        match self {
            Self::Panel => theme.colors.panel_border,
        }
    }
}

impl UiThemeTextColorRole {
    pub(in crate::game) fn color(self, theme: &UiTheme) -> Color {
        match self {
            Self::Primary => theme.colors.text_primary,
            Self::Muted => theme.colors.text_muted,
        }
    }
}

impl Default for UiTheme {
    fn default() -> Self {
        Self {
            colors: UiColors {
                screen_background: Color::srgb(0.05, 0.08, 0.11),
                panel_background: Color::srgba(0.10, 0.13, 0.16, 0.94),
                panel_border: Color::srgb(0.22, 0.28, 0.31),
                text_primary: Color::srgb(0.92, 0.95, 0.95),
                text_muted: Color::srgb(0.62, 0.68, 0.70),
                primary_button: ButtonColors {
                    idle: Color::srgb(0.12, 0.58, 0.52),
                    hovered: Color::srgb(0.15, 0.68, 0.60),
                    pressed: Color::srgb(0.08, 0.42, 0.39),
                    focused: Color::srgb(0.18, 0.74, 0.66),
                    selected: Color::srgb(0.09, 0.48, 0.44),
                    disabled: Color::srgb(0.12, 0.25, 0.24),
                    loading: Color::srgb(0.10, 0.34, 0.32),
                },
                secondary_button: ButtonColors {
                    idle: Color::srgb(0.16, 0.19, 0.22),
                    hovered: Color::srgb(0.22, 0.26, 0.29),
                    pressed: Color::srgb(0.11, 0.13, 0.16),
                    focused: Color::srgb(0.27, 0.33, 0.36),
                    selected: Color::srgb(0.18, 0.34, 0.31),
                    disabled: Color::srgb(0.11, 0.13, 0.15),
                    loading: Color::srgb(0.13, 0.17, 0.19),
                },
            },
            text: UiTextTheme {
                title_large: 44.0,
                title: 34.0,
                subtitle: 18.0,
                section_label: 16.0,
                body: 24.0,
                caption: 15.0,
                button: 18.0,
            },
            layout: UiLayoutTheme {
                screen_padding: 24.0,
                overlay_padding: 16.0,
                page_gap: 18.0,
                panel_gap: 20.0,
                card_gap: 12.0,
                header_gap: 12.0,
                row_gap: 6.0,
                row_padding_y: 8.0,
                row_column_gap: 16.0,
                auth_panel_width: 420.0,
                content_width: 760.0,
            },
            button: UiButtonTheme {
                min_width: 112.0,
                height: 46.0,
                padding_x: 18.0,
                radius: 6.0,
            },
            panel: UiPanelTheme {
                padding: 28.0,
                border: 1.0,
                radius: 8.0,
            },
        }
    }
}

#[derive(Clone, Debug, Resource)]
struct UiThemeSource {
    loaded_path: Option<PathBuf>,
    diagnostics: Vec<String>,
}

#[derive(Debug, Resource)]
struct UiThemeHotReload {
    watched_path: PathBuf,
    last_modified: Option<SystemTime>,
    poll_timer: Timer,
    last_error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UiThemeConfig {
    version: u32,
    colors: UiColorsConfig,
    text: UiTextTheme,
    layout: UiLayoutTheme,
    button: UiButtonTheme,
    panel: UiPanelTheme,
}

#[derive(Debug, Deserialize)]
struct UiColorsConfig {
    screen_background: UiColorConfig,
    panel_background: UiColorConfig,
    panel_border: UiColorConfig,
    text_primary: UiColorConfig,
    text_muted: UiColorConfig,
    primary_button: ButtonColorsConfig,
    secondary_button: ButtonColorsConfig,
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct UiColorConfig {
    r: f32,
    g: f32,
    b: f32,
    #[serde(default = "default_color_alpha")]
    a: f32,
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct ButtonColorsConfig {
    idle: UiColorConfig,
    hovered: UiColorConfig,
    pressed: UiColorConfig,
    focused: UiColorConfig,
    selected: UiColorConfig,
    disabled: UiColorConfig,
    loading: UiColorConfig,
}

fn load_ui_theme() -> (UiTheme, UiThemeSource) {
    let mut diagnostics = Vec::new();

    for path in ui_theme_path_candidates() {
        match load_ui_theme_from_path(&path) {
            Ok(theme) => {
                return (
                    theme,
                    UiThemeSource {
                        loaded_path: Some(path),
                        diagnostics,
                    },
                );
            }
            Err(error) => diagnostics.push(error),
        }
    }

    (
        UiTheme::default(),
        UiThemeSource {
            loaded_path: None,
            diagnostics,
        },
    )
}

fn load_ui_theme_from_path(path: &Path) -> Result<UiTheme, String> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(format!("{} not found", path.display()));
        }
        Err(error) => {
            return Err(format!("{} could not be read: {error}", path.display()));
        }
    };

    match ron::from_str::<UiThemeConfig>(&source) {
        Ok(config) if config.version == UI_THEME_CONFIG_VERSION => Ok(config.into_theme()),
        Ok(config) => Err(format!(
            "{} uses unsupported version {}, expected {}",
            path.display(),
            config.version,
            UI_THEME_CONFIG_VERSION
        )),
        Err(error) => Err(format!("{} could not be parsed: {error}", path.display())),
    }
}

fn ui_theme_path_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(path) = env::var(UI_THEME_ENV_VAR) {
        push_unique_path(&mut paths, PathBuf::from(path));
    }

    push_unique_path(&mut paths, PathBuf::from(DEFAULT_THEME_ASSET_PATH));
    push_unique_path(&mut paths, PathBuf::from(REPO_ROOT_THEME_ASSET_PATH));
    push_unique_path(
        &mut paths,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_THEME_ASSET_PATH),
    );

    paths
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

fn log_ui_theme_source(source: Res<UiThemeSource>) {
    if let Some(path) = &source.loaded_path {
        info!(path = %path.display(), "loaded ui theme config");
    } else if source.diagnostics.is_empty() {
        info!("using built-in ui theme");
    } else {
        warn!(
            diagnostics = ?source.diagnostics,
            "using built-in ui theme fallback"
        );
    }
}

impl UiThemeHotReload {
    fn new(source: &UiThemeSource) -> Self {
        let watched_path = source
            .loaded_path
            .clone()
            .unwrap_or_else(preferred_ui_theme_watch_path);
        let last_modified = ui_theme_modified_time(&watched_path).ok();

        Self {
            watched_path,
            last_modified,
            poll_timer: Timer::from_seconds(
                UI_THEME_HOT_RELOAD_INTERVAL_SECS,
                TimerMode::Repeating,
            ),
            last_error: None,
        }
    }
}

fn preferred_ui_theme_watch_path() -> PathBuf {
    if let Ok(path) = env::var(UI_THEME_ENV_VAR) {
        return PathBuf::from(path);
    }

    ui_theme_path_candidates()
        .into_iter()
        .find(|path| path.exists())
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_THEME_ASSET_PATH))
}

fn ui_theme_modified_time(path: &Path) -> io::Result<SystemTime> {
    fs::metadata(path).and_then(|metadata| metadata.modified())
}

fn poll_ui_theme_hot_reload(
    time: Res<Time>,
    mut theme: ResMut<UiTheme>,
    mut source: ResMut<UiThemeSource>,
    mut hot_reload: ResMut<UiThemeHotReload>,
) {
    if !hot_reload.poll_timer.tick(time.delta()).just_finished() {
        return;
    }

    let modified = match ui_theme_modified_time(&hot_reload.watched_path) {
        Ok(modified) => modified,
        Err(error) => {
            let message = format!(
                "{} could not be stat'ed: {error}",
                hot_reload.watched_path.display()
            );
            warn_ui_theme_reload_error(&mut hot_reload, message);
            return;
        }
    };

    if hot_reload.last_modified == Some(modified) && hot_reload.last_error.is_none() {
        return;
    }

    match load_ui_theme_from_path(&hot_reload.watched_path) {
        Ok(next_theme) => {
            *theme = next_theme;
            source.loaded_path = Some(hot_reload.watched_path.clone());
            source.diagnostics.clear();
            hot_reload.last_modified = Some(modified);
            hot_reload.last_error = None;
            info!(
                path = %hot_reload.watched_path.display(),
                "hot reloaded ui theme config"
            );
        }
        Err(error) => {
            warn_ui_theme_reload_error(&mut hot_reload, error);
        }
    }
}

fn warn_ui_theme_reload_error(hot_reload: &mut UiThemeHotReload, error: String) {
    if hot_reload.last_error.as_deref() != Some(error.as_str()) {
        warn!(
            path = %hot_reload.watched_path.display(),
            error = %error,
            "failed to hot reload ui theme config; keeping current theme"
        );
    }

    hot_reload.last_error = Some(error);
}

fn refresh_ui_theme_visuals(
    theme: Res<UiTheme>,
    mut clear_color: ResMut<ClearColor>,
    mut backgrounds: Query<(&UiThemeBackgroundRole, &mut BackgroundColor)>,
    mut borders: Query<(&UiThemeBorderRole, &mut BorderColor)>,
    mut text_colors: Query<(&UiThemeTextColorRole, &mut TextColor)>,
) {
    if !theme.is_changed() {
        return;
    }

    let mut has_screen_background = false;

    for (role, mut background) in &mut backgrounds {
        if matches!(*role, UiThemeBackgroundRole::Screen) {
            has_screen_background = true;
        }

        *background = BackgroundColor(role.color(&theme));
    }

    if has_screen_background {
        clear_color.0 = theme.colors.screen_background;
    }

    for (role, mut border) in &mut borders {
        *border = BorderColor::all(role.color(&theme));
    }

    for (role, mut text_color) in &mut text_colors {
        *text_color = TextColor(role.color(&theme));
    }
}

fn default_color_alpha() -> f32 {
    1.0
}

impl UiThemeConfig {
    fn into_theme(self) -> UiTheme {
        UiTheme {
            colors: self.colors.into_colors(),
            text: self.text,
            layout: self.layout,
            button: self.button,
            panel: self.panel,
        }
    }
}

impl UiColorsConfig {
    fn into_colors(self) -> UiColors {
        UiColors {
            screen_background: self.screen_background.into_color(),
            panel_background: self.panel_background.into_color(),
            panel_border: self.panel_border.into_color(),
            text_primary: self.text_primary.into_color(),
            text_muted: self.text_muted.into_color(),
            primary_button: self.primary_button.into_button_colors(),
            secondary_button: self.secondary_button.into_button_colors(),
        }
    }
}

impl UiColorConfig {
    fn into_color(self) -> Color {
        Color::srgba(
            self.r.clamp(0.0, 1.0),
            self.g.clamp(0.0, 1.0),
            self.b.clamp(0.0, 1.0),
            self.a.clamp(0.0, 1.0),
        )
    }
}

impl ButtonColorsConfig {
    fn into_button_colors(self) -> ButtonColors {
        ButtonColors {
            idle: self.idle.into_color(),
            hovered: self.hovered.into_color(),
            pressed: self.pressed.into_color(),
            focused: self.focused.into_color(),
            selected: self.selected.into_color(),
            disabled: self.disabled.into_color(),
            loading: self.loading.into_color(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::{Path, PathBuf},
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
                "mybevy-theme-tests-{}-{unique}",
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

    fn valid_theme_config_with_version(version: u32) -> String {
        format!(
            r#"(
    version: {version},
    colors: (
        screen_background: (r: 0.11, g: 0.12, b: 0.13),
        panel_background: (r: 0.21, g: 0.22, b: 0.23, a: 0.88),
        panel_border: (r: 0.31, g: 0.32, b: 0.33),
        text_primary: (r: 0.41, g: 0.42, b: 0.43),
        text_muted: (r: 0.51, g: 0.52, b: 0.53),
        primary_button: (
            idle: (r: 0.10, g: 0.20, b: 0.30),
            hovered: (r: 0.11, g: 0.21, b: 0.31),
            pressed: (r: 0.12, g: 0.22, b: 0.32),
            focused: (r: 0.13, g: 0.23, b: 0.33),
            selected: (r: 0.14, g: 0.24, b: 0.34),
            disabled: (r: 0.15, g: 0.25, b: 0.35),
            loading: (r: 0.16, g: 0.26, b: 0.36),
        ),
        secondary_button: (
            idle: (r: 0.20, g: 0.30, b: 0.40),
            hovered: (r: 0.21, g: 0.31, b: 0.41),
            pressed: (r: 0.22, g: 0.32, b: 0.42),
            focused: (r: 0.23, g: 0.33, b: 0.43),
            selected: (r: 0.24, g: 0.34, b: 0.44),
            disabled: (r: 0.25, g: 0.35, b: 0.45),
            loading: (r: 0.26, g: 0.36, b: 0.46),
        ),
    ),
    text: (
        title_large: 44.0,
        title: 34.0,
        subtitle: 18.0,
        section_label: 16.0,
        body: 24.0,
        caption: 15.0,
        button: 18.0,
    ),
    layout: (
        screen_padding: 24.0,
        overlay_padding: 16.0,
        page_gap: 18.0,
        panel_gap: 20.0,
        card_gap: 12.0,
        header_gap: 12.0,
        row_gap: 6.0,
        row_padding_y: 8.0,
        row_column_gap: 16.0,
        auth_panel_width: 420.0,
        content_width: 760.0,
    ),
    button: (
        min_width: 112.0,
        height: 46.0,
        padding_x: 18.0,
        radius: 6.0,
    ),
    panel: (
        padding: 28.0,
        border: 1.0,
        radius: 8.0,
    ),
)"#
        )
    }

    fn load_config(source: &str) -> Result<UiTheme, String> {
        let temp = TempConfigDir::new("load_config");
        let path = temp.write_config("theme.ron", source);
        load_ui_theme_from_path(&path)
    }

    fn assert_error_contains(error: &str, expected: &str) {
        assert!(
            error.contains(expected),
            "expected error to contain {expected:?}, got {error:?}"
        );
    }

    fn assert_srgba(color: Color, expected: (f32, f32, f32, f32)) {
        let actual = color.to_srgba();
        assert_eq!(
            (actual.red, actual.green, actual.blue, actual.alpha),
            expected
        );
    }

    #[test]
    fn parses_valid_ron_theme_config() {
        let theme = load_config(&valid_theme_config_with_version(UI_THEME_CONFIG_VERSION)).unwrap();

        assert_srgba(theme.colors.screen_background, (0.11, 0.12, 0.13, 1.0));
        assert_srgba(theme.colors.panel_background, (0.21, 0.22, 0.23, 0.88));
        assert_srgba(theme.colors.primary_button.hovered, (0.11, 0.21, 0.31, 1.0));
        assert_eq!(theme.text.title_large, 44.0);
        assert_eq!(theme.layout.content_width, 760.0);
        assert_eq!(theme.button.height, 46.0);
        assert_eq!(theme.panel.radius, 8.0);
    }

    #[test]
    fn rejects_unsupported_theme_config_version() {
        let error = load_config(&valid_theme_config_with_version(
            UI_THEME_CONFIG_VERSION + 1,
        ))
        .unwrap_err();

        assert_error_contains(&error, "uses unsupported version 2, expected 1");
    }

    #[test]
    fn reports_bad_ron_theme_config_as_parse_error() {
        let error = load_config("(version: 1, colors:").unwrap_err();

        assert_error_contains(&error, "could not be parsed");
    }

    #[test]
    fn clamps_color_channels_and_defaults_alpha() {
        assert_srgba(
            UiColorConfig {
                r: -1.0,
                g: 0.42,
                b: 2.0,
                a: 1.5,
            }
            .into_color(),
            (0.0, 0.42, 1.0, 1.0),
        );

        let parsed: UiColorConfig =
            ron::from_str("(r: 0.2, g: 0.3, b: 0.4)").expect("color config should parse");

        assert_srgba(parsed.into_color(), (0.2, 0.3, 0.4, 1.0));
    }

    #[test]
    fn reports_missing_theme_file() {
        let temp = TempConfigDir::new("reports_missing_theme_file");
        let path = temp.path.join("missing.ron");
        let error = load_ui_theme_from_path(Path::new(&path)).unwrap_err();

        assert_error_contains(&error, "not found");
    }
}
