use bevy::prelude::*;
use serde::Deserialize;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::framework::ui::{
    core::{UiFocusSystems, UiMetrics, UiViewport},
    style::{
        effects::{
            UiEffectCatalog, UiEffectCatalogConfig, UiMaterialRuntime, apply_resolved_ui_effects,
            cleanup_removed_ui_effect_bindings, resolve_ui_effect_bindings,
        },
        fonts::UiTextStyleToken,
        scopes::{
            UiStyleSheet, UiStyleSheetConfig, apply_resolved_ui_styles, resolve_ui_style_bindings,
        },
    },
};

const UI_THEME_CONFIG_VERSION: u32 = 1;
const DEFAULT_THEME_ASSET_PATH: &str = "assets/ui/themes/default.ron";
const REPO_ROOT_THEME_ASSET_PATH: &str = "project/assets/ui/themes/default.ron";
const UI_THEME_ENV_VAR: &str = "MYBEVY_UI_THEME";
const UI_THEME_HOT_RELOAD_INTERVAL_SECS: f32 = 0.8;

pub(crate) struct UiThemePlugin;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(crate) enum UiThemeSystems {
    Refresh,
}

impl Plugin for UiThemePlugin {
    fn build(&self, app: &mut App) {
        let (theme, source) = load_ui_theme();
        let hot_reload = UiThemeHotReload::new(&source);
        app.insert_resource(theme)
            .insert_resource(source)
            .insert_resource(hot_reload)
            .init_resource::<UiMaterialRuntime>()
            .configure_sets(
                Update,
                UiThemeSystems::Refresh.before(UiFocusSystems::Visuals),
            )
            .add_systems(Startup, log_ui_theme_source)
            .add_systems(
                Update,
                (
                    poll_ui_theme_hot_reload,
                    refresh_ui_theme_visuals,
                    resolve_ui_style_bindings,
                    apply_resolved_ui_styles,
                    resolve_ui_effect_bindings,
                    apply_resolved_ui_effects,
                    cleanup_removed_ui_effect_bindings,
                )
                    .chain()
                    .in_set(UiThemeSystems::Refresh),
            );
    }
}

#[derive(Clone, Debug, Resource)]
pub(crate) struct UiTheme {
    pub colors: UiColors,
    pub text: UiTextTheme,
    pub layout: UiLayoutTheme,
    pub button: UiButtonTheme,
    pub panel: UiPanelTheme,
    pub styles: UiStyleSheet,
    pub effects: UiEffectCatalog,
}

#[derive(Clone, Debug)]
pub(crate) struct UiColors {
    pub screen_background: Color,
    pub panel_background: Color,
    pub panel_border: Color,
    pub loading_overlay_background: Color,
    pub modal_overlay_background: Color,
    pub text_primary: Color,
    pub text_muted: Color,
    pub text_error: Color,
    pub error: Color,
    pub primary_button: ButtonColors,
    pub secondary_button: ButtonColors,
    pub icon_tint: ButtonColors,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct UiTextTheme {
    pub title_large: f32,
    pub title: f32,
    pub subtitle: f32,
    pub section_label: f32,
    pub body: f32,
    pub caption: f32,
    pub button: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct UiLayoutTheme {
    pub screen_padding: f32,
    #[allow(dead_code)]
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
pub(crate) struct UiButtonTheme {
    pub min_width: f32,
    pub height: f32,
    pub padding_x: f32,
    pub radius: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct UiPanelTheme {
    pub padding: f32,
    pub border: f32,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ButtonColors {
    pub idle: Color,
    pub hovered: Color,
    pub pressed: Color,
    pub focused: Color,
    pub selected: Color,
    pub disabled: Color,
    pub loading: Color,
}

#[derive(Clone, Copy, Debug, Component)]
pub(crate) enum UiThemeBackgroundRole {
    Screen,
    Panel,
    LoadingOverlay,
    ModalOverlay,
}

#[derive(Clone, Copy, Debug, Component)]
pub(crate) enum UiThemeBorderRole {
    Panel,
}

#[derive(Clone, Copy, Debug, Component)]
pub(crate) enum UiThemeTextColorRole {
    Primary,
    Muted,
}

#[derive(Clone, Copy, Debug, Component)]
pub(crate) enum UiThemeTextStyleRole {
    TitleLarge,
    Title,
    Subtitle,
    SectionLabel,
    Body,
    Caption,
    Button,
}

#[derive(Clone, Copy, Debug, Component)]
pub(crate) enum UiThemeButtonNodeRole {
    Button,
    TextInput,
}

#[derive(Clone, Copy, Debug, Component)]
#[allow(dead_code)]
pub(crate) enum UiThemePanelNodeRole {
    Standard,
    Content,
    Toast,
    Loading,
    Debug,
}

#[derive(Clone, Copy, Debug, Component)]
pub(crate) enum UiThemeRootNodeRole {
    Screen,
    Overlay,
    BlockingOverlay,
    Toast,
    FloatingPanel,
    Debug,
}

impl UiThemeBackgroundRole {
    fn color(self, theme: &UiTheme) -> Color {
        match self {
            Self::Screen => theme.colors.screen_background,
            Self::Panel => theme.colors.panel_background,
            Self::LoadingOverlay => theme.colors.loading_overlay_background,
            Self::ModalOverlay => theme.colors.modal_overlay_background,
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
    pub(crate) fn color(self, theme: &UiTheme) -> Color {
        match self {
            Self::Primary => theme.colors.text_primary,
            Self::Muted => theme.colors.text_muted,
        }
    }
}

impl UiThemeTextStyleRole {
    pub(crate) fn font_size(self, theme: &UiTheme) -> f32 {
        match self {
            Self::TitleLarge => theme.text.title_large,
            Self::Title => theme.text.title,
            Self::Subtitle => theme.text.subtitle,
            Self::SectionLabel => theme.text.section_label,
            Self::Body => theme.text.body,
            Self::Caption => theme.text.caption,
            Self::Button => theme.text.button,
        }
    }
}

impl UiThemeButtonNodeRole {
    fn apply(self, theme: &UiTheme, metrics: &UiMetrics, node: &mut Node) {
        match self {
            Self::Button => {
                node.min_width = px(theme.button.min_width.max(metrics.button_height * 2.25));
                node.height = px(metrics.button_height);
            }
            Self::TextInput => {
                node.min_height = px(metrics.input_height);
            }
        }

        node.padding = UiRect::axes(px((metrics.control_gap * 2.0).clamp(12.0, 24.0)), px(0));
        node.border_radius = BorderRadius::all(px(theme.button.radius));
    }
}

impl UiThemePanelNodeRole {
    fn apply(self, theme: &UiTheme, node: &mut Node) {
        node.padding = match self {
            Self::Standard => UiRect::all(px(theme.panel.padding)),
            Self::Content => UiRect::all(px(theme.layout.panel_gap)),
            Self::Toast => UiRect::axes(px(18), px(12)),
            Self::Loading => UiRect::axes(px(22), px(16)),
            Self::Debug => UiRect::all(px(14)),
        };
        node.border = UiRect::all(px(theme.panel.border));
        node.border_radius = BorderRadius::all(px(match self {
            Self::Toast => theme.button.radius,
            Self::Standard | Self::Content | Self::Loading | Self::Debug => theme.panel.radius,
        }));
    }
}

impl UiThemeRootNodeRole {
    fn apply(self, metrics: &UiMetrics, viewport: &UiViewport, node: &mut Node) {
        match self {
            Self::Screen => {
                node.padding = viewport.safe_area_padding(metrics.page_padding);
            }
            Self::Overlay => {
                node.padding = viewport.safe_area_padding(metrics.page_padding);
            }
            Self::BlockingOverlay => {
                node.padding = viewport.safe_area_padding(metrics.page_padding);
            }
            Self::Toast => {
                node.top = px(metrics.page_padding + viewport.safe_area.top);
                node.padding = UiRect {
                    left: px(metrics.page_padding + viewport.safe_area.left),
                    right: px(metrics.page_padding + viewport.safe_area.right),
                    top: px(0),
                    bottom: px(0),
                };
            }
            Self::FloatingPanel => {
                node.right = px(metrics.page_padding + viewport.safe_area.right);
            }
            Self::Debug => {
                node.left = px(metrics.page_padding + viewport.safe_area.left);
                node.top = px(metrics.page_padding + viewport.safe_area.top);
            }
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
                loading_overlay_background: Color::srgba(0.01, 0.02, 0.03, 0.56),
                modal_overlay_background: Color::srgba(0.01, 0.02, 0.03, 0.72),
                text_primary: Color::srgb(0.92, 0.95, 0.95),
                text_muted: Color::srgb(0.62, 0.68, 0.70),
                text_error: Color::srgb(1.0, 0.55, 0.52),
                error: Color::srgb(0.92, 0.26, 0.24),
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
                icon_tint: ButtonColors {
                    idle: Color::srgb(0.92, 0.95, 0.95),
                    hovered: Color::srgb(0.42, 0.94, 0.84),
                    pressed: Color::srgb(0.27, 0.78, 0.70),
                    focused: Color::srgb(0.62, 1.0, 0.90),
                    selected: Color::srgb(1.0, 0.82, 0.30),
                    disabled: Color::srgba(0.62, 0.68, 0.70, 0.62),
                    loading: Color::srgb(0.46, 0.80, 1.0),
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
            styles: UiStyleSheet::built_in(),
            effects: UiEffectCatalog::built_in(),
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
    enabled: bool,
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
    #[serde(default)]
    styles: UiStyleSheetConfig,
    #[serde(default)]
    effects: UiEffectCatalogConfig,
}

#[derive(Debug, Deserialize)]
struct UiColorsConfig {
    screen_background: UiColorConfig,
    panel_background: UiColorConfig,
    panel_border: UiColorConfig,
    #[serde(default = "default_loading_overlay_background")]
    loading_overlay_background: UiColorConfig,
    #[serde(default = "default_modal_overlay_background")]
    modal_overlay_background: UiColorConfig,
    text_primary: UiColorConfig,
    text_muted: UiColorConfig,
    #[serde(default = "default_text_error")]
    text_error: UiColorConfig,
    #[serde(default = "default_error")]
    error: UiColorConfig,
    primary_button: ButtonColorsConfig,
    secondary_button: ButtonColorsConfig,
    #[serde(default = "default_icon_tint")]
    icon_tint: ButtonColorsConfig,
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
        Ok(config) if config.version == UI_THEME_CONFIG_VERSION => config
            .into_theme()
            .map_err(|error| format!("{} has invalid visual config: {error}", path.display())),
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
        let enabled = source.loaded_path.is_some();
        let last_modified = enabled
            .then(|| ui_theme_modified_time(&watched_path).ok())
            .flatten();

        Self {
            enabled,
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
    if !hot_reload.enabled {
        return;
    }

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
    metrics: Res<UiMetrics>,
    viewport: Res<UiViewport>,
    mut clear_color: ResMut<ClearColor>,
    mut backgrounds: Query<(&UiThemeBackgroundRole, &mut BackgroundColor)>,
    mut borders: Query<(&UiThemeBorderRole, &mut BorderColor)>,
    mut text_colors: Query<(&UiThemeTextColorRole, &mut TextColor)>,
    mut text_styles: Query<(
        &UiThemeTextStyleRole,
        &mut TextFont,
        Option<&mut UiTextStyleToken>,
    )>,
    mut node_roles: ParamSet<(
        Query<(&UiThemeButtonNodeRole, &mut Node)>,
        Query<(&UiThemePanelNodeRole, &mut Node)>,
        Query<(&UiThemeRootNodeRole, &mut Node)>,
    )>,
) {
    if !theme.is_changed() && !metrics.is_changed() && !viewport.is_changed() {
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

    for (role, mut font, style) in &mut text_styles {
        let font_size = role.font_size(&theme);
        font.font_size = font_size;
        if let Some(mut style) = style {
            style.font_size = font_size;
        }
    }

    for (role, mut node) in &mut node_roles.p0() {
        role.apply(&theme, &metrics, &mut node);
    }

    for (role, mut node) in &mut node_roles.p1() {
        role.apply(&theme, &mut node);
    }

    for (role, mut node) in &mut node_roles.p2() {
        role.apply(&metrics, &viewport, &mut node);
    }
}

fn default_color_alpha() -> f32 {
    1.0
}

fn default_loading_overlay_background() -> UiColorConfig {
    UiColorConfig {
        r: 0.01,
        g: 0.02,
        b: 0.03,
        a: 0.56,
    }
}

fn default_modal_overlay_background() -> UiColorConfig {
    UiColorConfig {
        r: 0.01,
        g: 0.02,
        b: 0.03,
        a: 0.72,
    }
}

fn default_text_error() -> UiColorConfig {
    UiColorConfig {
        r: 1.0,
        g: 0.55,
        b: 0.52,
        a: 1.0,
    }
}

fn default_error() -> UiColorConfig {
    UiColorConfig {
        r: 0.92,
        g: 0.26,
        b: 0.24,
        a: 1.0,
    }
}

fn default_icon_tint() -> ButtonColorsConfig {
    ButtonColorsConfig {
        idle: UiColorConfig {
            r: 0.92,
            g: 0.95,
            b: 0.95,
            a: 1.0,
        },
        hovered: UiColorConfig {
            r: 0.42,
            g: 0.94,
            b: 0.84,
            a: 1.0,
        },
        pressed: UiColorConfig {
            r: 0.27,
            g: 0.78,
            b: 0.70,
            a: 1.0,
        },
        focused: UiColorConfig {
            r: 0.62,
            g: 1.0,
            b: 0.90,
            a: 1.0,
        },
        selected: UiColorConfig {
            r: 1.0,
            g: 0.82,
            b: 0.30,
            a: 1.0,
        },
        disabled: UiColorConfig {
            r: 0.62,
            g: 0.68,
            b: 0.70,
            a: 0.62,
        },
        loading: UiColorConfig {
            r: 0.46,
            g: 0.80,
            b: 1.0,
            a: 1.0,
        },
    }
}

impl UiThemeConfig {
    fn into_theme(self) -> Result<UiTheme, String> {
        let styles = UiStyleSheet::compile(self.styles).map_err(|error| error.to_string())?;
        let effects = UiEffectCatalog::compile(self.effects).map_err(|error| error.to_string())?;
        Ok(UiTheme {
            colors: self.colors.into_colors(),
            text: self.text,
            layout: self.layout,
            button: self.button,
            panel: self.panel,
            styles,
            effects,
        })
    }
}

impl UiColorsConfig {
    fn into_colors(self) -> UiColors {
        UiColors {
            screen_background: self.screen_background.into_color(),
            panel_background: self.panel_background.into_color(),
            panel_border: self.panel_border.into_color(),
            loading_overlay_background: self.loading_overlay_background.into_color(),
            modal_overlay_background: self.modal_overlay_background.into_color(),
            text_primary: self.text_primary.into_color(),
            text_muted: self.text_muted.into_color(),
            text_error: self.text_error.into_color(),
            error: self.error.into_color(),
            primary_button: self.primary_button.into_button_colors(),
            secondary_button: self.secondary_button.into_button_colors(),
            icon_tint: self.icon_tint.into_button_colors(),
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
        loading_overlay_background: (r: 0.34, g: 0.35, b: 0.36, a: 0.57),
        modal_overlay_background: (r: 0.37, g: 0.38, b: 0.39, a: 0.73),
        text_primary: (r: 0.41, g: 0.42, b: 0.43),
        text_muted: (r: 0.51, g: 0.52, b: 0.53),
        text_error: (r: 0.61, g: 0.12, b: 0.13),
        error: (r: 0.71, g: 0.22, b: 0.23),
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
        title_large: 52.0,
        title: 38.0,
        subtitle: 22.0,
        section_label: 17.0,
        body: 26.0,
        caption: 13.0,
        button: 19.0,
    ),
    layout: (
        screen_padding: 30.0,
        overlay_padding: 18.0,
        page_gap: 18.0,
        panel_gap: 21.0,
        card_gap: 12.0,
        header_gap: 12.0,
        row_gap: 6.0,
        row_padding_y: 8.0,
        row_column_gap: 16.0,
        auth_panel_width: 420.0,
        content_width: 760.0,
    ),
    button: (
        min_width: 130.0,
        height: 50.0,
        padding_x: 24.0,
        radius: 9.0,
    ),
    panel: (
        padding: 32.0,
        border: 2.0,
        radius: 11.0,
    ),
)"#
        )
    }

    fn legacy_theme_config_without_overlay_colors(version: u32) -> String {
        valid_theme_config_with_version(version)
            .replace(
                "        loading_overlay_background: (r: 0.34, g: 0.35, b: 0.36, a: 0.57),\n",
                "",
            )
            .replace(
                "        modal_overlay_background: (r: 0.37, g: 0.38, b: 0.39, a: 0.73),\n",
                "",
            )
    }

    fn legacy_theme_config_without_form_error_colors(version: u32) -> String {
        valid_theme_config_with_version(version)
            .replace("        text_error: (r: 0.61, g: 0.12, b: 0.13),\n", "")
            .replace("        error: (r: 0.71, g: 0.22, b: 0.23),\n", "")
    }

    fn theme_config_with_styles(styles: &str) -> String {
        let mut config = valid_theme_config_with_version(UI_THEME_CONFIG_VERSION);
        assert_eq!(config.pop(), Some(')'));
        config.push_str(&format!("    styles: {styles},\n)"));
        config
    }

    fn theme_config_with_effects(effects: &str) -> String {
        let mut config = valid_theme_config_with_version(UI_THEME_CONFIG_VERSION);
        assert_eq!(config.pop(), Some(')'));
        config.push_str(&format!("    effects: {effects},\n)"));
        config
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

    fn assert_px(value: Val, expected: f32) {
        assert_eq!(value, px(expected));
    }

    fn assert_rect_all_px(rect: UiRect, expected: f32) {
        assert_eq!(rect, UiRect::all(px(expected)));
    }

    fn assert_radius_all_px(radius: BorderRadius, expected: f32) {
        assert_eq!(radius, BorderRadius::all(px(expected)));
    }

    fn app_with_theme(theme: UiTheme) -> App {
        let mut app = App::new();
        app.insert_resource(theme)
            .insert_resource(UiMetrics::default())
            .insert_resource(UiViewport::default())
            .insert_resource(ClearColor(Color::BLACK))
            .add_systems(Update, refresh_ui_theme_visuals);
        app
    }

    #[test]
    fn parses_valid_ron_theme_config() {
        let theme = load_config(&valid_theme_config_with_version(UI_THEME_CONFIG_VERSION)).unwrap();

        assert_srgba(theme.colors.screen_background, (0.11, 0.12, 0.13, 1.0));
        assert_srgba(theme.colors.panel_background, (0.21, 0.22, 0.23, 0.88));
        assert_srgba(
            theme.colors.loading_overlay_background,
            (0.34, 0.35, 0.36, 0.57),
        );
        assert_srgba(
            theme.colors.modal_overlay_background,
            (0.37, 0.38, 0.39, 0.73),
        );
        assert_srgba(theme.colors.text_error, (0.61, 0.12, 0.13, 1.0));
        assert_srgba(theme.colors.error, (0.71, 0.22, 0.23, 1.0));
        assert_srgba(theme.colors.primary_button.hovered, (0.11, 0.21, 0.31, 1.0));
        assert_eq!(theme.text.title_large, 52.0);
        assert_eq!(theme.layout.content_width, 760.0);
        assert_eq!(theme.button.height, 50.0);
        assert_eq!(theme.panel.radius, 11.0);
        assert!(theme.styles.contains_variant(
            &crate::framework::ui::style::UiStyleVariantId::new(
                crate::framework::ui::style::UI_STYLE_VARIANT_GALLERY_PARENT,
            ),
        ));
    }

    #[test]
    fn packaged_default_theme_owns_icon_state_tints() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_THEME_ASSET_PATH);
        let theme = load_ui_theme_from_path(&path).expect("packaged default theme should parse");

        assert_srgba(theme.colors.icon_tint.idle, (0.92, 0.95, 0.95, 1.0));
        assert_srgba(theme.colors.icon_tint.hovered, (0.42, 0.94, 0.84, 1.0));
        assert_srgba(theme.colors.icon_tint.disabled, (0.62, 0.68, 0.70, 0.62));
        assert_srgba(theme.colors.icon_tint.loading, (0.46, 0.80, 1.0, 1.0));
        assert!(theme.styles.contains_variant(
            &crate::framework::ui::style::UiStyleVariantId::new(
                crate::framework::ui::style::UI_STYLE_VARIANT_GALLERY_NESTED,
            ),
        ));
        for preset in [
            crate::framework::ui::style::UI_EFFECT_PRESET_GALLERY_SHADOW,
            crate::framework::ui::style::UI_EFFECT_PRESET_GALLERY_TEXT_SHADOW,
            crate::framework::ui::style::UI_EFFECT_PRESET_GALLERY_GRADIENT,
            crate::framework::ui::style::UI_EFFECT_PRESET_GALLERY_COMPOSITE,
            crate::framework::ui::style::UI_EFFECT_PRESET_GALLERY_MATERIAL_FALLBACK,
        ] {
            assert!(
                theme
                    .effects
                    .contains_preset(&crate::framework::ui::style::UiEffectPresetId::new(preset))
            );
        }
    }

    #[test]
    fn parses_legacy_theme_config_without_overlay_colors() {
        let theme = load_config(&legacy_theme_config_without_overlay_colors(
            UI_THEME_CONFIG_VERSION,
        ))
        .unwrap();

        assert_srgba(
            theme.colors.loading_overlay_background,
            (0.01, 0.02, 0.03, 0.56),
        );
        assert_srgba(
            theme.colors.modal_overlay_background,
            (0.01, 0.02, 0.03, 0.72),
        );
    }

    #[test]
    fn parses_legacy_theme_config_without_form_error_colors() {
        let theme = load_config(&legacy_theme_config_without_form_error_colors(
            UI_THEME_CONFIG_VERSION,
        ))
        .unwrap();

        assert_srgba(theme.colors.text_error, (1.0, 0.55, 0.52, 1.0));
        assert_srgba(theme.colors.error, (0.92, 0.26, 0.24, 1.0));
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
    fn rejects_invalid_scoped_styles_with_stable_error_code() {
        let config = theme_config_with_styles(
            r#"(
                variants: [(name: "bad", overrides: [
                    SurfaceBackground(role: panel, token: "missing"),
                ])],
            )"#,
        );
        let error = load_config(&config).unwrap_err();

        assert_error_contains(&error, "ui_style_unknown_token");
    }

    #[test]
    fn rejects_invalid_effects_with_stable_error_code() {
        let config = theme_config_with_effects(
            r#"(
                presets: [(
                    name: "bad.gradient",
                    background_gradient: Some((
                        angle_degrees: 90.0,
                        stops: [
                            (position: 0.8, color: (r: 0.1, g: 0.2, b: 0.3)),
                            (position: 0.2, color: (r: 0.4, g: 0.5, b: 0.6)),
                        ],
                    )),
                )],
            )"#,
        );
        let error = load_config(&config).unwrap_err();

        assert_error_contains(&error, "ui_effect_invalid_gradient_stops");
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
    fn refresh_theme_visuals_updates_text_font_sizes() {
        let theme = load_config(&valid_theme_config_with_version(UI_THEME_CONFIG_VERSION)).unwrap();
        let mut app = app_with_theme(theme);
        let title = app
            .world_mut()
            .spawn((
                TextFont::from_font_size(1.0),
                UiThemeTextStyleRole::TitleLarge,
            ))
            .id();
        let button = app
            .world_mut()
            .spawn((TextFont::from_font_size(2.0), UiThemeTextStyleRole::Button))
            .id();

        app.update();

        assert_eq!(
            app.world()
                .entity(title)
                .get::<TextFont>()
                .unwrap()
                .font_size,
            52.0
        );
        assert_eq!(
            app.world()
                .entity(button)
                .get::<TextFont>()
                .unwrap()
                .font_size,
            19.0
        );
    }

    #[test]
    fn refresh_theme_updates_only_theme_owned_text_size_in_style_token() {
        use crate::framework::ui::style::{
            UiFontWeight, UiTextAlignment, UiTextLineHeight, UiTextTruncation, UiTextWrap,
        };

        let theme = load_config(&valid_theme_config_with_version(UI_THEME_CONFIG_VERSION)).unwrap();
        let mut style = UiTextStyleToken::latin_fixture(UiFontWeight::Bold, 1.0);
        style.line_height = UiTextLineHeight::Pixels(27.0);
        style.alignment = UiTextAlignment::Right;
        style.wrap = UiTextWrap::NoWrap;
        style.truncation = UiTextTruncation::Ellipsis { max_graphemes: 9 };
        let expected = style.clone();
        let mut app = app_with_theme(theme);
        let entity = app
            .world_mut()
            .spawn((
                TextFont::from_font_size(1.0),
                UiThemeTextStyleRole::Title,
                style,
            ))
            .id();

        app.update();

        let actual = app
            .world()
            .entity(entity)
            .get::<UiTextStyleToken>()
            .unwrap();
        assert_eq!(actual.font_size, 38.0);
        assert_eq!(actual.font_role, expected.font_role);
        assert_eq!(actual.font_family, expected.font_family);
        assert_eq!(actual.font_weight, expected.font_weight);
        assert_eq!(actual.line_height, expected.line_height);
        assert_eq!(actual.alignment, expected.alignment);
        assert_eq!(actual.wrap, expected.wrap);
        assert_eq!(actual.truncation, expected.truncation);
    }

    #[test]
    fn refresh_theme_visuals_updates_button_and_text_input_nodes() {
        let theme = load_config(&valid_theme_config_with_version(UI_THEME_CONFIG_VERSION)).unwrap();
        let metrics = UiMetrics::default();
        let mut app = app_with_theme(theme);
        let button = app
            .world_mut()
            .spawn((Node::default(), UiThemeButtonNodeRole::Button))
            .id();
        let text_input = app
            .world_mut()
            .spawn((Node::default(), UiThemeButtonNodeRole::TextInput))
            .id();

        app.update();

        let button_node = app.world().entity(button).get::<Node>().unwrap();
        assert_px(button_node.min_width, 130.0);
        assert_px(button_node.height, metrics.button_height);
        assert_eq!(button_node.padding, UiRect::axes(px(24.0), px(0.0)));
        assert_radius_all_px(button_node.border_radius, 9.0);

        let input_node = app.world().entity(text_input).get::<Node>().unwrap();
        assert_px(input_node.min_height, metrics.input_height);
        assert_eq!(input_node.padding, UiRect::axes(px(24.0), px(0.0)));
        assert_radius_all_px(input_node.border_radius, 9.0);
    }

    #[test]
    fn refresh_theme_visuals_updates_panel_nodes_and_overlay_roots() {
        let theme = load_config(&valid_theme_config_with_version(UI_THEME_CONFIG_VERSION)).unwrap();
        let mut app = app_with_theme(theme);
        let panel = app
            .world_mut()
            .spawn((Node::default(), UiThemePanelNodeRole::Standard))
            .id();
        let content_panel = app
            .world_mut()
            .spawn((Node::default(), UiThemePanelNodeRole::Content))
            .id();
        let toast_panel = app
            .world_mut()
            .spawn((Node::default(), UiThemePanelNodeRole::Toast))
            .id();
        let screen_root = app
            .world_mut()
            .spawn((Node::default(), UiThemeRootNodeRole::Screen))
            .id();
        let overlay_root = app
            .world_mut()
            .spawn((Node::default(), UiThemeRootNodeRole::Overlay))
            .id();
        let toast_root = app
            .world_mut()
            .spawn((Node::default(), UiThemeRootNodeRole::Toast))
            .id();

        app.update();

        let panel_node = app.world().entity(panel).get::<Node>().unwrap();
        assert_rect_all_px(panel_node.padding, 32.0);
        assert_rect_all_px(panel_node.border, 2.0);
        assert_radius_all_px(panel_node.border_radius, 11.0);

        let content_panel_node = app.world().entity(content_panel).get::<Node>().unwrap();
        assert_rect_all_px(content_panel_node.padding, 21.0);
        assert_rect_all_px(content_panel_node.border, 2.0);
        assert_radius_all_px(content_panel_node.border_radius, 11.0);

        let toast_panel_node = app.world().entity(toast_panel).get::<Node>().unwrap();
        assert_eq!(toast_panel_node.padding, UiRect::axes(px(18.0), px(12.0)));
        assert_rect_all_px(toast_panel_node.border, 2.0);
        assert_radius_all_px(toast_panel_node.border_radius, 9.0);

        let screen_root_node = app.world().entity(screen_root).get::<Node>().unwrap();
        assert_rect_all_px(screen_root_node.padding, UiMetrics::default().page_padding);

        let overlay_root_node = app.world().entity(overlay_root).get::<Node>().unwrap();
        assert_rect_all_px(overlay_root_node.padding, UiMetrics::default().page_padding);

        let toast_root_node = app.world().entity(toast_root).get::<Node>().unwrap();
        assert_px(toast_root_node.top, UiMetrics::default().page_padding);
        assert_eq!(
            toast_root_node.padding,
            UiRect {
                left: px(UiMetrics::default().page_padding),
                right: px(UiMetrics::default().page_padding),
                top: px(0.0),
                bottom: px(0.0),
            }
        );
    }

    #[test]
    fn refresh_theme_visuals_updates_overlay_background_tokens() {
        let theme = load_config(&valid_theme_config_with_version(UI_THEME_CONFIG_VERSION)).unwrap();
        let mut app = app_with_theme(theme);
        let loading = app
            .world_mut()
            .spawn((
                BackgroundColor(Color::BLACK),
                UiThemeBackgroundRole::LoadingOverlay,
            ))
            .id();
        let modal = app
            .world_mut()
            .spawn((
                BackgroundColor(Color::BLACK),
                UiThemeBackgroundRole::ModalOverlay,
            ))
            .id();

        app.update();

        assert_srgba(
            app.world()
                .entity(loading)
                .get::<BackgroundColor>()
                .unwrap()
                .0,
            (0.34, 0.35, 0.36, 0.57),
        );
        assert_srgba(
            app.world()
                .entity(modal)
                .get::<BackgroundColor>()
                .unwrap()
                .0,
            (0.37, 0.38, 0.39, 0.73),
        );
    }

    #[test]
    fn reports_missing_theme_file() {
        let temp = TempConfigDir::new("reports_missing_theme_file");
        let path = temp.path.join("missing.ron");
        let error = load_ui_theme_from_path(Path::new(&path)).unwrap_err();

        assert_error_contains(&error, "not found");
    }

    #[test]
    fn hot_reload_keeps_current_theme_when_updated_file_is_invalid() {
        let temp =
            TempConfigDir::new("hot_reload_keeps_current_theme_when_updated_file_is_invalid");
        let path = temp.write_config(
            "theme.ron",
            &valid_theme_config_with_version(UI_THEME_CONFIG_VERSION),
        );
        let current_theme = load_ui_theme_from_path(&path).unwrap();
        let current_title_size = current_theme.text.title_large;
        let current_button_height = current_theme.button.height;
        fs::write(&path, "(version: 1, colors:").expect("bad temp config should be written");

        let mut hot_reload = UiThemeHotReload {
            enabled: true,
            watched_path: path,
            last_modified: None,
            poll_timer: Timer::from_seconds(0.0, TimerMode::Repeating),
            last_error: None,
        };
        hot_reload.poll_timer.tick(std::time::Duration::ZERO);
        let source = UiThemeSource {
            loaded_path: Some(hot_reload.watched_path.clone()),
            diagnostics: Vec::new(),
        };
        let mut app = App::new();
        app.insert_resource(current_theme)
            .insert_resource(source)
            .insert_resource(hot_reload)
            .insert_resource(Time::<()>::default())
            .add_systems(Update, poll_ui_theme_hot_reload);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));

        app.update();

        let theme = app.world().resource::<UiTheme>();
        assert_eq!(theme.text.title_large, current_title_size);
        assert_eq!(theme.button.height, current_button_height);
    }

    #[test]
    fn hot_reload_keeps_last_known_good_theme_when_scoped_styles_are_invalid() {
        let temp = TempConfigDir::new(
            "hot_reload_keeps_last_known_good_theme_when_scoped_styles_are_invalid",
        );
        let path = temp.write_config(
            "theme.ron",
            &valid_theme_config_with_version(UI_THEME_CONFIG_VERSION),
        );
        let current_theme = load_ui_theme_from_path(&path).unwrap();
        let current_background = current_theme.colors.screen_background;
        let invalid = theme_config_with_styles(
            r#"(
                tokens: [(name: "radius", value: Scalar(-1.0))],
                variants: [(name: "bad", overrides: [
                    BorderRadius(role: panel, token: "radius"),
                ])],
            )"#,
        );
        fs::write(&path, invalid).expect("invalid scoped style config should be written");

        let mut hot_reload = UiThemeHotReload {
            enabled: true,
            watched_path: path,
            last_modified: None,
            poll_timer: Timer::from_seconds(0.0, TimerMode::Repeating),
            last_error: None,
        };
        hot_reload.poll_timer.tick(std::time::Duration::ZERO);
        let source = UiThemeSource {
            loaded_path: Some(hot_reload.watched_path.clone()),
            diagnostics: Vec::new(),
        };
        let mut app = App::new();
        app.insert_resource(current_theme)
            .insert_resource(source)
            .insert_resource(hot_reload)
            .insert_resource(Time::<()>::default())
            .add_systems(Update, poll_ui_theme_hot_reload);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));

        app.update();

        assert_eq!(
            app.world().resource::<UiTheme>().colors.screen_background,
            current_background
        );
        assert!(
            app.world()
                .resource::<UiThemeHotReload>()
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("ui_style_invalid_value"))
        );
    }

    #[test]
    fn hot_reload_keeps_last_known_good_effect_catalog_when_effects_are_invalid() {
        let temp = TempConfigDir::new(
            "hot_reload_keeps_last_known_good_effect_catalog_when_effects_are_invalid",
        );
        let path = temp.write_config(
            "theme.ron",
            &valid_theme_config_with_version(UI_THEME_CONFIG_VERSION),
        );
        let current_theme = load_ui_theme_from_path(&path).unwrap();
        assert!(current_theme.effects.contains_preset(
            &crate::framework::ui::style::UiEffectPresetId::new(
                crate::framework::ui::style::UI_EFFECT_PRESET_GALLERY_COMPOSITE,
            ),
        ));
        let invalid = theme_config_with_effects(
            r#"(
                presets: [(
                    name: "bad.gradient",
                    background_gradient: Some((
                        angle_degrees: 90.0,
                        stops: [
                            (position: 0.8, color: (r: 0.1, g: 0.2, b: 0.3)),
                            (position: 0.2, color: (r: 0.4, g: 0.5, b: 0.6)),
                        ],
                    )),
                )],
            )"#,
        );
        fs::write(&path, invalid).expect("invalid effect config should be written");

        let mut hot_reload = UiThemeHotReload {
            enabled: true,
            watched_path: path,
            last_modified: None,
            poll_timer: Timer::from_seconds(0.0, TimerMode::Repeating),
            last_error: None,
        };
        hot_reload.poll_timer.tick(std::time::Duration::ZERO);
        let source = UiThemeSource {
            loaded_path: Some(hot_reload.watched_path.clone()),
            diagnostics: Vec::new(),
        };
        let mut app = App::new();
        app.insert_resource(current_theme)
            .insert_resource(source)
            .insert_resource(hot_reload)
            .insert_resource(Time::<()>::default())
            .add_systems(Update, poll_ui_theme_hot_reload);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));

        app.update();

        assert!(app.world().resource::<UiTheme>().effects.contains_preset(
            &crate::framework::ui::style::UiEffectPresetId::new(
                crate::framework::ui::style::UI_EFFECT_PRESET_GALLERY_COMPOSITE,
            ),
        ));
        assert!(
            app.world()
                .resource::<UiThemeHotReload>()
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("ui_effect_invalid_gradient_stops"))
        );
    }

    #[test]
    fn disabled_hot_reload_keeps_built_in_theme_without_polling_missing_path() {
        let missing_path = PathBuf::from("missing/theme.ron");
        let mut hot_reload = UiThemeHotReload {
            enabled: false,
            watched_path: missing_path,
            last_modified: None,
            poll_timer: Timer::from_seconds(0.0, TimerMode::Repeating),
            last_error: None,
        };
        hot_reload.poll_timer.tick(std::time::Duration::ZERO);
        let source = UiThemeSource {
            loaded_path: None,
            diagnostics: Vec::new(),
        };
        let mut app = App::new();
        app.insert_resource(UiTheme::default())
            .insert_resource(source)
            .insert_resource(hot_reload)
            .insert_resource(Time::<()>::default())
            .add_systems(Update, poll_ui_theme_hot_reload);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));

        app.update();

        let hot_reload = app.world().resource::<UiThemeHotReload>();
        assert!(hot_reload.last_error.is_none());
    }
}
