use bevy::prelude::*;
use serde::Deserialize;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

const UI_THEME_CONFIG_VERSION: u32 = 1;
const DEFAULT_THEME_ASSET_PATH: &str = "assets/ui/themes/default.ron";
const REPO_ROOT_THEME_ASSET_PATH: &str = "project/assets/ui/themes/default.ron";
const UI_THEME_ENV_VAR: &str = "MYBEVY_UI_THEME";

pub(in crate::game) struct UiThemePlugin;

impl Plugin for UiThemePlugin {
    fn build(&self, app: &mut App) {
        let (theme, source) = load_ui_theme();
        app.insert_resource(theme)
            .insert_resource(source)
            .add_systems(Startup, log_ui_theme_source);
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
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                diagnostics.push(format!("{} not found", path.display()));
                continue;
            }
            Err(error) => {
                diagnostics.push(format!("{} could not be read: {error}", path.display()));
                continue;
            }
        };

        match ron::from_str::<UiThemeConfig>(&source) {
            Ok(config) if config.version == UI_THEME_CONFIG_VERSION => {
                return (
                    config.into_theme(),
                    UiThemeSource {
                        loaded_path: Some(path),
                        diagnostics,
                    },
                );
            }
            Ok(config) => {
                diagnostics.push(format!(
                    "{} uses unsupported version {}, expected {}",
                    path.display(),
                    config.version,
                    UI_THEME_CONFIG_VERSION
                ));
            }
            Err(error) => {
                diagnostics.push(format!("{} could not be parsed: {error}", path.display()));
            }
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
