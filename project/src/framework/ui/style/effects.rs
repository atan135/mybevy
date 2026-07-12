use std::collections::HashMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::framework::ui::style::theme::UiTheme;

pub(crate) const UI_EFFECT_PRESET_GALLERY_SHADOW: &str = "gallery.shadow";
pub(crate) const UI_EFFECT_PRESET_GALLERY_TEXT_SHADOW: &str = "gallery.text_shadow";
pub(crate) const UI_EFFECT_PRESET_GALLERY_GRADIENT: &str = "gallery.gradient";
pub(crate) const UI_EFFECT_PRESET_GALLERY_COMPOSITE: &str = "gallery.composite";
pub(crate) const UI_EFFECT_PRESET_GALLERY_MATERIAL_FALLBACK: &str = "gallery.material_fallback";

pub(crate) const MAX_BOX_SHADOW_LAYERS: usize = 3;
pub(crate) const MAX_TEXT_SHADOW_LAYERS: usize = 1;
pub(crate) const MAX_GRADIENT_STOPS: usize = 6;
pub(crate) const MAX_EFFECT_DRAW_CALL_UPPER_BOUND: u8 = 8;
pub(crate) const MAX_EFFECT_OVERDRAW_LAYERS: u8 = 5;

const MAX_SHADOW_OFFSET_PX: f32 = 64.0;
const MIN_SHADOW_SPREAD_PX: f32 = -24.0;
const MAX_SHADOW_SPREAD_PX: f32 = 48.0;
const MAX_SHADOW_BLUR_PX: f32 = 64.0;
const MAX_BORDER_WIDTH_PX: f32 = 16.0;
const MAX_BORDER_RADIUS_PX: f32 = 64.0;
const MAX_OUTLINE_WIDTH_PX: f32 = 8.0;
const MAX_OUTLINE_OFFSET_PX: f32 = 16.0;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct UiEffectPresetId(String);

impl UiEffectPresetId {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Component, Eq, PartialEq)]
pub(crate) struct UiEffectBinding {
    pub preset: UiEffectPresetId,
}

impl UiEffectBinding {
    pub(crate) fn new(preset: impl Into<String>) -> Self {
        Self {
            preset: UiEffectPresetId::new(preset),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiEffectErrorCode {
    UnknownPreset,
    DuplicatePreset,
    InvalidColor,
    InvalidValue,
    TooManyShadowLayers,
    TextShadowUnsupported,
    InvalidGradientStops,
    BudgetExceeded,
    MaterialNotAllowlisted,
    MaterialFallbackMissing,
    MaterialInvalidParameters,
    MaterialGpuUnsupported,
    MaterialPlatformUnsupported,
    MaterialShaderLoading,
    MaterialShaderLoadFailed,
    MaterialShaderUnavailable,
    MaterialAdapterUnavailable,
}

impl UiEffectErrorCode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::UnknownPreset => "ui_effect_unknown_preset",
            Self::DuplicatePreset => "ui_effect_duplicate_preset",
            Self::InvalidColor => "ui_effect_invalid_color",
            Self::InvalidValue => "ui_effect_invalid_value",
            Self::TooManyShadowLayers => "ui_effect_too_many_shadow_layers",
            Self::TextShadowUnsupported => "ui_effect_text_shadow_unsupported",
            Self::InvalidGradientStops => "ui_effect_invalid_gradient_stops",
            Self::BudgetExceeded => "ui_effect_budget_exceeded",
            Self::MaterialNotAllowlisted => "ui_material_not_allowlisted",
            Self::MaterialFallbackMissing => "ui_material_fallback_missing",
            Self::MaterialInvalidParameters => "ui_material_invalid_parameters",
            Self::MaterialGpuUnsupported => "ui_material_gpu_unsupported",
            Self::MaterialPlatformUnsupported => "ui_material_platform_unsupported",
            Self::MaterialShaderLoading => "ui_material_shader_loading",
            Self::MaterialShaderLoadFailed => "ui_material_shader_load_failed",
            Self::MaterialShaderUnavailable => "ui_material_shader_unavailable",
            Self::MaterialAdapterUnavailable => "ui_material_adapter_unavailable",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UiEffectError {
    pub code: UiEffectErrorCode,
    pub detail: String,
}

impl UiEffectError {
    fn new(code: UiEffectErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for UiEffectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code.as_str(), self.detail)
    }
}

impl std::error::Error for UiEffectError {}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiMaterialId {
    FrostedPanelV1,
}

impl UiMaterialId {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::FrostedPanelV1 => "frosted_panel_v1",
        }
    }

    fn parse(value: &str) -> Result<Self, UiEffectError> {
        match value {
            "frosted_panel_v1" => Ok(Self::FrostedPanelV1),
            _ => Err(UiEffectError::new(
                UiEffectErrorCode::MaterialNotAllowlisted,
                format!("material id '{value}' is not in the framework allowlist"),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiMaterialPlatform {
    Windows,
    Linux,
    MacOs,
    Android,
    Web,
    Other,
}

impl UiMaterialPlatform {
    fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "android") {
            Self::Android
        } else if cfg!(target_family = "wasm") {
            Self::Web
        } else {
            Self::Other
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) enum UiMaterialShaderState {
    Loading,
    Loaded,
    Failed,
}

#[derive(Clone, Debug, Resource)]
pub(crate) struct UiMaterialRuntime {
    platform: UiMaterialPlatform,
    gpu_custom_materials_supported: bool,
    shader_states: HashMap<UiMaterialId, UiMaterialShaderState>,
}

impl Default for UiMaterialRuntime {
    fn default() -> Self {
        let platform = UiMaterialPlatform::current();
        Self {
            platform,
            gpu_custom_materials_supported: !matches!(
                platform,
                UiMaterialPlatform::Web | UiMaterialPlatform::Other
            ),
            shader_states: HashMap::new(),
        }
    }
}

impl UiMaterialRuntime {
    #[allow(dead_code)]
    pub(crate) fn set_gpu_custom_materials_supported(&mut self, supported: bool) {
        self.gpu_custom_materials_supported = supported;
    }

    #[allow(dead_code)]
    pub(crate) fn report_shader_state(&mut self, id: UiMaterialId, state: UiMaterialShaderState) {
        self.shader_states.insert(id, state);
    }

    #[cfg(test)]
    fn for_test(platform: UiMaterialPlatform, gpu_supported: bool) -> Self {
        Self {
            platform,
            gpu_custom_materials_supported: gpu_supported,
            shader_states: HashMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct UiMaterialPolicy {
    id: UiMaterialId,
    shader_path: &'static str,
    platforms: &'static [UiMaterialPlatform],
    max_scalars: usize,
    max_colors: usize,
    max_textures: u8,
}

const NATIVE_MATERIAL_PLATFORMS: &[UiMaterialPlatform] = &[
    UiMaterialPlatform::Windows,
    UiMaterialPlatform::Linux,
    UiMaterialPlatform::MacOs,
    UiMaterialPlatform::Android,
];

const FROSTED_PANEL_POLICY: UiMaterialPolicy = UiMaterialPolicy {
    id: UiMaterialId::FrostedPanelV1,
    shader_path: "shaders/ui/frosted_panel_v1.wgsl",
    platforms: NATIVE_MATERIAL_PLATFORMS,
    max_scalars: 4,
    max_colors: 2,
    max_textures: 0,
};

fn material_policy(id: UiMaterialId) -> &'static UiMaterialPolicy {
    match id {
        UiMaterialId::FrostedPanelV1 => &FROSTED_PANEL_POLICY,
    }
}

#[derive(Clone, Debug, PartialEq)]
struct UiMaterialRequest {
    id: UiMaterialId,
    scalars: Vec<f32>,
    colors: Vec<Color>,
    texture_count: u8,
}

#[derive(Clone, Debug, PartialEq)]
struct UiMaterialFallback {
    background: Color,
    border: Color,
}

#[derive(Clone, Debug, PartialEq)]
struct UiMaterialResolution {
    shader_path: &'static str,
    fallback_reason: UiEffectErrorCode,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct UiEffectCatalogConfig {
    #[serde(default)]
    presets: Vec<UiEffectPresetConfig>,
}

impl Default for UiEffectCatalogConfig {
    fn default() -> Self {
        built_in_effect_catalog_config()
    }
}

#[derive(Clone, Debug, Deserialize)]
struct UiEffectPresetConfig {
    name: String,
    #[serde(default)]
    box_shadows: Vec<UiShadowLayerConfig>,
    #[serde(default)]
    text_shadows: Vec<UiShadowLayerConfig>,
    #[serde(default)]
    background_gradient: Option<UiLinearGradientConfig>,
    #[serde(default)]
    border_gradient: Option<UiLinearGradientConfig>,
    #[serde(default)]
    border: Option<UiBorderWidthsConfig>,
    #[serde(default)]
    radius: Option<UiCornerRadiiConfig>,
    #[serde(default)]
    outline: Option<UiOutlineConfig>,
    #[serde(default)]
    clip: bool,
    #[serde(default)]
    material: Option<UiMaterialRequestConfig>,
    #[serde(default)]
    material_fallback: Option<UiMaterialFallbackConfig>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct UiEffectColorConfig {
    r: f32,
    g: f32,
    b: f32,
    #[serde(default = "default_alpha")]
    a: f32,
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct UiShadowLayerConfig {
    color: UiEffectColorConfig,
    x_offset: f32,
    y_offset: f32,
    #[serde(default)]
    spread: f32,
    #[serde(default)]
    blur: f32,
}

#[derive(Clone, Debug, Deserialize)]
struct UiLinearGradientConfig {
    angle_degrees: f32,
    stops: Vec<UiGradientStopConfig>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct UiGradientStopConfig {
    position: f32,
    color: UiEffectColorConfig,
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct UiBorderWidthsConfig {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct UiCornerRadiiConfig {
    top_left: f32,
    top_right: f32,
    bottom_right: f32,
    bottom_left: f32,
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct UiOutlineConfig {
    width: f32,
    #[serde(default)]
    offset: f32,
    color: UiEffectColorConfig,
}

#[derive(Clone, Debug, Deserialize)]
struct UiMaterialRequestConfig {
    id: String,
    #[serde(default)]
    scalars: Vec<f32>,
    #[serde(default)]
    colors: Vec<UiEffectColorConfig>,
    #[serde(default)]
    texture_count: u8,
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct UiMaterialFallbackConfig {
    background: UiEffectColorConfig,
    border: UiEffectColorConfig,
}

#[derive(Clone, Debug, PartialEq)]
struct UiCompiledEffectPreset {
    name: String,
    box_shadow: Option<BoxShadow>,
    text_shadow: Option<TextShadow>,
    background_gradient: Option<BackgroundGradient>,
    border_gradient: Option<BorderGradient>,
    border: Option<UiRect>,
    radius: Option<BorderRadius>,
    overflow: Option<Overflow>,
    outline: Option<Outline>,
    material: Option<UiMaterialRequest>,
    material_fallback: Option<UiMaterialFallback>,
    budget: UiEffectBudget,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct UiEffectBudget {
    requested_draw_call_upper_bound: u8,
    applied_draw_call_upper_bound: u8,
    overdraw_layers: u8,
    shadow_layers: u8,
    gradient_stops: u8,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub(crate) struct UiEffectBudgetSnapshot {
    pub requested_draw_call_upper_bound: u8,
    pub applied_draw_call_upper_bound: u8,
    pub overdraw_layers: u8,
    pub shadow_layers: u8,
    pub gradient_stops: u8,
}

impl From<UiEffectBudget> for UiEffectBudgetSnapshot {
    fn from(value: UiEffectBudget) -> Self {
        Self {
            requested_draw_call_upper_bound: value.requested_draw_call_upper_bound,
            applied_draw_call_upper_bound: value.applied_draw_call_upper_bound,
            overdraw_layers: value.overdraw_layers,
            shadow_layers: value.shadow_layers,
            gradient_stops: value.gradient_stops,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct UiEffectCatalog {
    presets: HashMap<String, UiCompiledEffectPreset>,
}

impl UiEffectCatalog {
    pub(crate) fn built_in() -> Self {
        Self::compile(UiEffectCatalogConfig::default())
            .expect("built-in UI effect presets must remain valid")
    }

    pub(super) fn compile(config: UiEffectCatalogConfig) -> Result<Self, UiEffectError> {
        let mut presets = HashMap::with_capacity(config.presets.len());
        for preset in config.presets {
            if preset.name.trim().is_empty() {
                return Err(UiEffectError::new(
                    UiEffectErrorCode::InvalidValue,
                    "effect preset name must not be empty",
                ));
            }
            if presets.contains_key(&preset.name) {
                return Err(UiEffectError::new(
                    UiEffectErrorCode::DuplicatePreset,
                    format!("effect preset '{}' is declared more than once", preset.name),
                ));
            }
            let compiled = compile_effect_preset(preset)?;
            presets.insert(compiled.name.clone(), compiled);
        }
        Ok(Self { presets })
    }

    #[cfg(test)]
    pub(crate) fn contains_preset(&self, id: &UiEffectPresetId) -> bool {
        self.presets.contains_key(id.as_str())
    }

    fn get(&self, id: &UiEffectPresetId) -> Option<&UiCompiledEffectPreset> {
        self.presets.get(id.as_str())
    }
}

#[derive(Clone, Debug, Component, PartialEq)]
pub(super) struct UiResolvedEffectStyle {
    preset: String,
    box_shadow: Option<BoxShadow>,
    text_shadow: Option<TextShadow>,
    background_gradient: Option<BackgroundGradient>,
    border_gradient: Option<BorderGradient>,
    border: Option<UiRect>,
    radius: Option<BorderRadius>,
    overflow: Option<Overflow>,
    outline: Option<Outline>,
    fallback_background: Option<BackgroundColor>,
    fallback_border: Option<BorderColor>,
}

#[derive(Clone, Debug, Component, PartialEq, Serialize)]
pub(crate) struct UiResolvedEffectDebugSnapshot {
    pub request: String,
    pub resolved_preset: String,
    pub applied_components: Vec<String>,
    pub material: Option<UiResolvedMaterialDebug>,
    pub budget: UiEffectBudgetSnapshot,
    pub fallback: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct UiResolvedMaterialDebug {
    pub id: String,
    pub shader_path: String,
    pub platform: UiMaterialPlatform,
    pub outcome: String,
    pub reason: Option<String>,
    pub scalar_count: usize,
    pub color_count: usize,
    pub texture_count: u8,
}

#[derive(Clone, Debug, Component, PartialEq)]
pub(super) struct UiEffectBaseline {
    border: UiRect,
    radius: BorderRadius,
    overflow: Overflow,
    box_shadow: Option<BoxShadow>,
    text_shadow: Option<TextShadow>,
    background_gradient: Option<BackgroundGradient>,
    border_gradient: Option<BorderGradient>,
    outline: Option<Outline>,
    background_color: Option<BackgroundColor>,
    border_color: Option<BorderColor>,
}

#[derive(Clone, Debug, Default, Component, PartialEq)]
pub(super) struct UiEffectLastApplied {
    border: Option<UiRect>,
    radius: Option<BorderRadius>,
    overflow: Option<Overflow>,
    box_shadow: Option<BoxShadow>,
    text_shadow: Option<TextShadow>,
    background_gradient: Option<BackgroundGradient>,
    border_gradient: Option<BorderGradient>,
    outline: Option<Outline>,
    background_color: Option<BackgroundColor>,
    border_color: Option<BorderColor>,
}

#[derive(Clone, Debug)]
struct UiResolvedEffect {
    style: UiResolvedEffectStyle,
    debug: UiResolvedEffectDebugSnapshot,
}

pub(super) fn resolve_ui_effect_bindings(
    mut commands: Commands,
    theme: Res<UiTheme>,
    material_runtime: Res<UiMaterialRuntime>,
    bindings: Query<(
        Entity,
        &UiEffectBinding,
        Option<&UiResolvedEffectStyle>,
        Option<&UiResolvedEffectDebugSnapshot>,
    )>,
) {
    for (entity, binding, current_style, current_debug) in &bindings {
        let resolved = resolve_effect_binding(binding, &theme.effects, &material_runtime);
        if current_style != Some(&resolved.style) {
            commands.entity(entity).insert(resolved.style);
        }
        if current_debug != Some(&resolved.debug) {
            commands.entity(entity).insert(resolved.debug);
        }
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn apply_resolved_ui_effects(
    mut commands: Commands,
    mut effects: Query<(
        Entity,
        &UiResolvedEffectStyle,
        &mut Node,
        Option<&UiEffectBaseline>,
        Option<&UiEffectLastApplied>,
        Option<&BoxShadow>,
        Option<&TextShadow>,
        Option<&BackgroundGradient>,
        Option<&BorderGradient>,
        Option<&Outline>,
        Option<&BackgroundColor>,
        Option<&BorderColor>,
    )>,
) {
    for (
        entity,
        resolved,
        mut node,
        baseline,
        last_applied,
        box_shadow,
        text_shadow,
        background_gradient,
        border_gradient,
        outline,
        background_color,
        border_color,
    ) in &mut effects
    {
        let previous_baseline = baseline.cloned();
        let mut baseline = previous_baseline
            .clone()
            .unwrap_or_else(|| UiEffectBaseline {
                border: node.border,
                radius: node.border_radius,
                overflow: node.overflow,
                box_shadow: box_shadow.cloned(),
                text_shadow: text_shadow.copied(),
                background_gradient: background_gradient.cloned(),
                border_gradient: border_gradient.cloned(),
                outline: outline.copied(),
                background_color: background_color.copied(),
                border_color: border_color.cloned(),
            });
        let previous_last_applied = last_applied.cloned();
        let mut next_last_applied = previous_last_applied.clone().unwrap_or_default();

        apply_owned_value(
            &mut node.border,
            resolved.border,
            &mut baseline.border,
            &mut next_last_applied.border,
        );
        apply_owned_value(
            &mut node.border_radius,
            resolved.radius,
            &mut baseline.radius,
            &mut next_last_applied.radius,
        );
        apply_owned_value(
            &mut node.overflow,
            resolved.overflow,
            &mut baseline.overflow,
            &mut next_last_applied.overflow,
        );

        apply_owned_component(
            &mut commands,
            entity,
            box_shadow,
            resolved.box_shadow.clone(),
            &mut baseline.box_shadow,
            &mut next_last_applied.box_shadow,
        );
        apply_owned_component(
            &mut commands,
            entity,
            text_shadow,
            resolved.text_shadow,
            &mut baseline.text_shadow,
            &mut next_last_applied.text_shadow,
        );
        apply_owned_component(
            &mut commands,
            entity,
            background_gradient,
            resolved.background_gradient.clone(),
            &mut baseline.background_gradient,
            &mut next_last_applied.background_gradient,
        );
        apply_owned_component(
            &mut commands,
            entity,
            border_gradient,
            resolved.border_gradient.clone(),
            &mut baseline.border_gradient,
            &mut next_last_applied.border_gradient,
        );
        apply_owned_component(
            &mut commands,
            entity,
            outline,
            resolved.outline,
            &mut baseline.outline,
            &mut next_last_applied.outline,
        );
        apply_owned_component(
            &mut commands,
            entity,
            background_color,
            resolved.fallback_background,
            &mut baseline.background_color,
            &mut next_last_applied.background_color,
        );
        apply_owned_component(
            &mut commands,
            entity,
            border_color,
            resolved.fallback_border.clone(),
            &mut baseline.border_color,
            &mut next_last_applied.border_color,
        );

        if previous_baseline.as_ref() != Some(&baseline) {
            commands.entity(entity).insert(baseline);
        }
        if previous_last_applied.as_ref() != Some(&next_last_applied) {
            commands.entity(entity).insert(next_last_applied);
        }
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn cleanup_removed_ui_effect_bindings(
    mut commands: Commands,
    mut removed: RemovedComponents<UiEffectBinding>,
    still_bound: Query<(), With<UiEffectBinding>>,
    mut entities: Query<(
        &mut Node,
        &UiEffectBaseline,
        &UiEffectLastApplied,
        Option<&BoxShadow>,
        Option<&TextShadow>,
        Option<&BackgroundGradient>,
        Option<&BorderGradient>,
        Option<&Outline>,
        Option<&BackgroundColor>,
        Option<&BorderColor>,
    )>,
) {
    for entity in removed.read() {
        if still_bound.contains(entity) {
            continue;
        }
        let Ok((
            mut node,
            baseline,
            last_applied,
            box_shadow,
            text_shadow,
            background,
            border,
            outline,
            bg,
            bc,
        )) = entities.get_mut(entity)
        else {
            continue;
        };
        restore_owned_value(&mut node.border, baseline.border, last_applied.border);
        restore_owned_value(
            &mut node.border_radius,
            baseline.radius,
            last_applied.radius,
        );
        restore_owned_value(&mut node.overflow, baseline.overflow, last_applied.overflow);
        restore_owned_component(
            &mut commands,
            entity,
            box_shadow,
            baseline.box_shadow.clone(),
            last_applied.box_shadow.as_ref(),
        );
        restore_owned_component(
            &mut commands,
            entity,
            text_shadow,
            baseline.text_shadow,
            last_applied.text_shadow.as_ref(),
        );
        restore_owned_component(
            &mut commands,
            entity,
            background,
            baseline.background_gradient.clone(),
            last_applied.background_gradient.as_ref(),
        );
        restore_owned_component(
            &mut commands,
            entity,
            border,
            baseline.border_gradient.clone(),
            last_applied.border_gradient.as_ref(),
        );
        restore_owned_component(
            &mut commands,
            entity,
            outline,
            baseline.outline,
            last_applied.outline.as_ref(),
        );
        restore_owned_component(
            &mut commands,
            entity,
            bg,
            baseline.background_color,
            last_applied.background_color.as_ref(),
        );
        restore_owned_component(
            &mut commands,
            entity,
            bc,
            baseline.border_color.clone(),
            last_applied.border_color.as_ref(),
        );
        commands.entity(entity).remove::<(
            UiEffectBaseline,
            UiEffectLastApplied,
            UiResolvedEffectStyle,
            UiResolvedEffectDebugSnapshot,
        )>();
    }
}

fn apply_owned_value<T: Copy + PartialEq>(
    current: &mut T,
    effect_value: Option<T>,
    baseline: &mut T,
    last_applied: &mut Option<T>,
) {
    let previous = *last_applied;
    let current_matches_previous = previous.is_some_and(|value| *current == value);
    if previous.is_none() || !current_matches_previous {
        *baseline = *current;
    }

    match effect_value {
        Some(value) => {
            set_if_different(current, value);
            *last_applied = Some(value);
        }
        None => {
            if previous.is_some() && current_matches_previous {
                set_if_different(current, *baseline);
            }
            *last_applied = None;
        }
    }
}

fn apply_owned_component<T: Component + Clone + PartialEq>(
    commands: &mut Commands,
    entity: Entity,
    current: Option<&T>,
    effect_value: Option<T>,
    baseline: &mut Option<T>,
    last_applied: &mut Option<T>,
) {
    let previous = last_applied.clone();
    let current_matches_previous = previous
        .as_ref()
        .is_some_and(|value| current == Some(value));
    if previous.is_none() || !current_matches_previous {
        *baseline = current.cloned();
    }

    match effect_value {
        Some(value) => {
            sync_optional_component(commands, entity, current, Some(value.clone()));
            *last_applied = Some(value);
        }
        None => {
            if previous.is_some() && current_matches_previous {
                sync_optional_component(commands, entity, current, baseline.clone());
            }
            *last_applied = None;
        }
    }
}

fn restore_owned_value<T: Copy + PartialEq>(current: &mut T, baseline: T, last_applied: Option<T>) {
    if last_applied.is_some_and(|value| *current == value) {
        set_if_different(current, baseline);
    }
}

fn restore_owned_component<T: Component + Clone + PartialEq>(
    commands: &mut Commands,
    entity: Entity,
    current: Option<&T>,
    baseline: Option<T>,
    last_applied: Option<&T>,
) {
    if last_applied.is_some_and(|value| current == Some(value)) {
        sync_optional_component(commands, entity, current, baseline);
    }
}

fn sync_optional_component<T: Component + Clone + PartialEq>(
    commands: &mut Commands,
    entity: Entity,
    current: Option<&T>,
    desired: Option<T>,
) {
    match (current, desired) {
        (Some(current), Some(desired)) if current != &desired => {
            commands.entity(entity).insert(desired);
        }
        (None, Some(desired)) => {
            commands.entity(entity).insert(desired);
        }
        (Some(_), None) => {
            commands.entity(entity).remove::<T>();
        }
        _ => {}
    }
}

fn set_if_different<T: PartialEq>(current: &mut T, desired: T) {
    if *current != desired {
        *current = desired;
    }
}

fn resolve_effect_binding(
    binding: &UiEffectBinding,
    catalog: &UiEffectCatalog,
    runtime: &UiMaterialRuntime,
) -> UiResolvedEffect {
    let (preset, unknown_error) = match catalog.get(&binding.preset) {
        Some(preset) => (preset.clone(), None),
        None => (
            unknown_effect_fallback_preset(),
            Some(UiEffectErrorCode::UnknownPreset),
        ),
    };

    let mut fallback = unknown_error.is_some();
    let mut error = unknown_error.map(|code| code.as_str().to_owned());
    let mut fallback_background = None;
    let mut fallback_border = None;
    let mut material_debug = None;
    let mut budget = preset.budget;

    if let Some(request) = &preset.material {
        let resolution = resolve_material_request(request, runtime);
        let code = resolution.fallback_reason;
        fallback = true;
        error = Some(code.as_str().to_owned());
        if let Some(material_fallback) = &preset.material_fallback {
            fallback_background = Some(BackgroundColor(material_fallback.background));
            fallback_border = Some(BorderColor::all(material_fallback.border));
        }
        budget.applied_draw_call_upper_bound =
            budget.applied_draw_call_upper_bound.saturating_sub(1);
        material_debug = Some(UiResolvedMaterialDebug {
            id: request.id.as_str().to_owned(),
            shader_path: resolution.shader_path.to_owned(),
            platform: runtime.platform,
            outcome: "fallback".to_owned(),
            reason: Some(code.as_str().to_owned()),
            scalar_count: request.scalars.len(),
            color_count: request.colors.len(),
            texture_count: request.texture_count,
        });
    }

    let style = UiResolvedEffectStyle {
        preset: preset.name.clone(),
        box_shadow: preset.box_shadow.clone(),
        text_shadow: preset.text_shadow,
        background_gradient: preset.background_gradient.clone(),
        border_gradient: preset.border_gradient.clone(),
        border: preset.border,
        radius: preset.radius,
        overflow: preset.overflow,
        outline: preset.outline,
        fallback_background,
        fallback_border,
    };
    let debug = UiResolvedEffectDebugSnapshot {
        request: binding.preset.as_str().to_owned(),
        resolved_preset: preset.name,
        applied_components: applied_component_names(&style),
        material: material_debug,
        budget: budget.into(),
        fallback,
        error,
    };
    UiResolvedEffect { style, debug }
}

fn resolve_material_request(
    request: &UiMaterialRequest,
    runtime: &UiMaterialRuntime,
) -> UiMaterialResolution {
    let policy = material_policy(request.id);
    let fallback = |code| UiMaterialResolution {
        shader_path: policy.shader_path,
        fallback_reason: code,
    };

    if request.scalars.len() > policy.max_scalars
        || request.colors.len() > policy.max_colors
        || request.texture_count > policy.max_textures
        || request.scalars.iter().any(|value| !value.is_finite())
        || request
            .colors
            .iter()
            .any(|color| validate_color(*color).is_err())
    {
        return fallback(UiEffectErrorCode::MaterialInvalidParameters);
    }
    if !policy.platforms.contains(&runtime.platform) {
        return fallback(UiEffectErrorCode::MaterialPlatformUnsupported);
    }
    if !runtime.gpu_custom_materials_supported {
        return fallback(UiEffectErrorCode::MaterialGpuUnsupported);
    }
    match runtime.shader_states.get(&policy.id).copied() {
        None => fallback(UiEffectErrorCode::MaterialShaderUnavailable),
        Some(UiMaterialShaderState::Loading) => fallback(UiEffectErrorCode::MaterialShaderLoading),
        Some(UiMaterialShaderState::Failed) => {
            fallback(UiEffectErrorCode::MaterialShaderLoadFailed)
        }
        Some(UiMaterialShaderState::Loaded) => {
            fallback(UiEffectErrorCode::MaterialAdapterUnavailable)
        }
    }
}

fn applied_component_names(style: &UiResolvedEffectStyle) -> Vec<String> {
    let mut values = Vec::new();
    if style.box_shadow.is_some() {
        values.push("box_shadow".to_owned());
    }
    if style.text_shadow.is_some() {
        values.push("text_shadow".to_owned());
    }
    if style.background_gradient.is_some() {
        values.push("background_gradient".to_owned());
    }
    if style.border_gradient.is_some() {
        values.push("border_gradient".to_owned());
    }
    if style.border.is_some() {
        values.push("independent_border_widths".to_owned());
    }
    if style.radius.is_some() {
        values.push("independent_corner_radii".to_owned());
    }
    if style.overflow.is_some() {
        values.push("rounded_clip".to_owned());
    }
    if style.outline.is_some() {
        values.push("outline".to_owned());
    }
    if style.fallback_background.is_some() {
        values.push("material_fallback_background".to_owned());
    }
    if style.fallback_border.is_some() {
        values.push("material_fallback_border".to_owned());
    }
    values
}

fn compile_effect_preset(
    config: UiEffectPresetConfig,
) -> Result<UiCompiledEffectPreset, UiEffectError> {
    if config.box_shadows.len() > MAX_BOX_SHADOW_LAYERS {
        return Err(UiEffectError::new(
            UiEffectErrorCode::TooManyShadowLayers,
            format!(
                "effect preset '{}' has {} box shadows; maximum is {MAX_BOX_SHADOW_LAYERS}",
                config.name,
                config.box_shadows.len()
            ),
        ));
    }
    if config.text_shadows.len() > MAX_TEXT_SHADOW_LAYERS {
        return Err(UiEffectError::new(
            UiEffectErrorCode::TextShadowUnsupported,
            format!(
                "effect preset '{}' requests {} text shadows; Bevy 0.18.1 supports one",
                config.name,
                config.text_shadows.len()
            ),
        ));
    }

    let mut box_layers = Vec::with_capacity(config.box_shadows.len());
    for layer in config.box_shadows {
        box_layers.push(compile_box_shadow_layer(layer)?);
    }
    let box_shadow = (!box_layers.is_empty()).then_some(BoxShadow(box_layers));

    let text_shadow = config
        .text_shadows
        .into_iter()
        .next()
        .map(compile_text_shadow_layer)
        .transpose()?;
    let background_gradient = config
        .background_gradient
        .map(compile_linear_gradient)
        .transpose()?
        .map(BackgroundGradient::from);
    let border_gradient = config
        .border_gradient
        .map(compile_linear_gradient)
        .transpose()?
        .map(BorderGradient::from);
    let border = config.border.map(compile_border_widths).transpose()?;
    let radius = config.radius.map(compile_corner_radii).transpose()?;
    let outline = config.outline.map(compile_outline).transpose()?;
    let overflow = config.clip.then_some(Overflow::clip());

    let material = config.material.map(compile_material_request).transpose()?;
    let material_fallback = config
        .material_fallback
        .map(compile_material_fallback)
        .transpose()?;
    if material.is_some() && material_fallback.is_none() {
        return Err(UiEffectError::new(
            UiEffectErrorCode::MaterialFallbackMissing,
            format!(
                "effect preset '{}' requests a material without a visible fallback",
                config.name
            ),
        ));
    }

    let gradient_stops = background_gradient
        .as_ref()
        .map_or(0, |gradient| linear_gradient_stop_count(&gradient.0))
        + border_gradient
            .as_ref()
            .map_or(0, |gradient| linear_gradient_stop_count(&gradient.0));
    let shadow_layers =
        box_shadow.as_ref().map_or(0, |shadow| shadow.0.len()) + usize::from(text_shadow.is_some());
    let requested_draw_call_upper_bound = shadow_layers
        + usize::from(background_gradient.is_some())
        + usize::from(border_gradient.is_some())
        + usize::from(outline.is_some())
        + usize::from(material.is_some());
    let overdraw_layers = shadow_layers
        + usize::from(background_gradient.is_some())
        + usize::from(material.is_some());
    if requested_draw_call_upper_bound > usize::from(MAX_EFFECT_DRAW_CALL_UPPER_BOUND)
        || overdraw_layers > usize::from(MAX_EFFECT_OVERDRAW_LAYERS)
    {
        return Err(UiEffectError::new(
            UiEffectErrorCode::BudgetExceeded,
            format!(
                "effect preset '{}' exceeds draw/overdraw planning budget ({requested_draw_call_upper_bound}/{overdraw_layers})",
                config.name
            ),
        ));
    }

    let budget = UiEffectBudget {
        requested_draw_call_upper_bound: requested_draw_call_upper_bound as u8,
        applied_draw_call_upper_bound: requested_draw_call_upper_bound as u8,
        overdraw_layers: overdraw_layers as u8,
        shadow_layers: shadow_layers as u8,
        gradient_stops: gradient_stops as u8,
    };
    Ok(UiCompiledEffectPreset {
        name: config.name,
        box_shadow,
        text_shadow,
        background_gradient,
        border_gradient,
        border,
        radius,
        overflow,
        outline,
        material,
        material_fallback,
        budget,
    })
}

fn compile_box_shadow_layer(config: UiShadowLayerConfig) -> Result<ShadowStyle, UiEffectError> {
    validate_range(
        "shadow x_offset",
        config.x_offset,
        -MAX_SHADOW_OFFSET_PX,
        MAX_SHADOW_OFFSET_PX,
    )?;
    validate_range(
        "shadow y_offset",
        config.y_offset,
        -MAX_SHADOW_OFFSET_PX,
        MAX_SHADOW_OFFSET_PX,
    )?;
    validate_range(
        "shadow spread",
        config.spread,
        MIN_SHADOW_SPREAD_PX,
        MAX_SHADOW_SPREAD_PX,
    )?;
    validate_range("shadow blur", config.blur, 0.0, MAX_SHADOW_BLUR_PX)?;
    Ok(ShadowStyle {
        color: config.color.try_color()?,
        x_offset: px(config.x_offset),
        y_offset: px(config.y_offset),
        spread_radius: px(config.spread),
        blur_radius: px(config.blur),
    })
}

fn compile_text_shadow_layer(config: UiShadowLayerConfig) -> Result<TextShadow, UiEffectError> {
    if config.spread != 0.0 || config.blur != 0.0 {
        return Err(UiEffectError::new(
            UiEffectErrorCode::TextShadowUnsupported,
            "Bevy 0.18.1 TextShadow supports color and offset only; spread and blur must be zero",
        ));
    }
    validate_range(
        "text shadow x_offset",
        config.x_offset,
        -MAX_SHADOW_OFFSET_PX,
        MAX_SHADOW_OFFSET_PX,
    )?;
    validate_range(
        "text shadow y_offset",
        config.y_offset,
        -MAX_SHADOW_OFFSET_PX,
        MAX_SHADOW_OFFSET_PX,
    )?;
    Ok(TextShadow {
        offset: Vec2::new(config.x_offset, config.y_offset),
        color: config.color.try_color()?,
    })
}

fn compile_linear_gradient(
    config: UiLinearGradientConfig,
) -> Result<LinearGradient, UiEffectError> {
    if !config.angle_degrees.is_finite() {
        return Err(UiEffectError::new(
            UiEffectErrorCode::InvalidValue,
            "gradient angle must be finite",
        ));
    }
    if !(2..=MAX_GRADIENT_STOPS).contains(&config.stops.len()) {
        return Err(UiEffectError::new(
            UiEffectErrorCode::InvalidGradientStops,
            format!(
                "linear gradient requires 2..={MAX_GRADIENT_STOPS} color stops, got {}",
                config.stops.len()
            ),
        ));
    }
    let mut previous = -1.0;
    let mut stops = Vec::with_capacity(config.stops.len());
    for stop in config.stops {
        if !stop.position.is_finite()
            || !(0.0..=1.0).contains(&stop.position)
            || stop.position < previous
        {
            return Err(UiEffectError::new(
                UiEffectErrorCode::InvalidGradientStops,
                "gradient stop positions must be finite, ordered, and inside 0..=1",
            ));
        }
        previous = stop.position;
        stops.push(ColorStop::percent(
            stop.color.try_color()?,
            stop.position * 100.0,
        ));
    }
    Ok(LinearGradient::new(
        config.angle_degrees.rem_euclid(360.0).to_radians(),
        stops,
    ))
}

fn compile_border_widths(config: UiBorderWidthsConfig) -> Result<UiRect, UiEffectError> {
    for (name, value) in [
        ("border left", config.left),
        ("border right", config.right),
        ("border top", config.top),
        ("border bottom", config.bottom),
    ] {
        validate_range(name, value, 0.0, MAX_BORDER_WIDTH_PX)?;
    }
    Ok(UiRect {
        left: px(config.left),
        right: px(config.right),
        top: px(config.top),
        bottom: px(config.bottom),
    })
}

fn compile_corner_radii(config: UiCornerRadiiConfig) -> Result<BorderRadius, UiEffectError> {
    for (name, value) in [
        ("radius top_left", config.top_left),
        ("radius top_right", config.top_right),
        ("radius bottom_right", config.bottom_right),
        ("radius bottom_left", config.bottom_left),
    ] {
        validate_range(name, value, 0.0, MAX_BORDER_RADIUS_PX)?;
    }
    Ok(BorderRadius {
        top_left: px(config.top_left),
        top_right: px(config.top_right),
        bottom_right: px(config.bottom_right),
        bottom_left: px(config.bottom_left),
    })
}

fn compile_outline(config: UiOutlineConfig) -> Result<Outline, UiEffectError> {
    validate_range("outline width", config.width, 0.0, MAX_OUTLINE_WIDTH_PX)?;
    validate_range(
        "outline offset",
        config.offset,
        -MAX_OUTLINE_OFFSET_PX,
        MAX_OUTLINE_OFFSET_PX,
    )?;
    Ok(Outline::new(
        px(config.width),
        px(config.offset),
        config.color.try_color()?,
    ))
}

fn compile_material_request(
    config: UiMaterialRequestConfig,
) -> Result<UiMaterialRequest, UiEffectError> {
    Ok(UiMaterialRequest {
        id: UiMaterialId::parse(&config.id)?,
        scalars: config.scalars,
        colors: config
            .colors
            .into_iter()
            .map(UiEffectColorConfig::unchecked_color)
            .collect(),
        texture_count: config.texture_count,
    })
}

fn compile_material_fallback(
    config: UiMaterialFallbackConfig,
) -> Result<UiMaterialFallback, UiEffectError> {
    Ok(UiMaterialFallback {
        background: config.background.try_color()?,
        border: config.border.try_color()?,
    })
}

fn linear_gradient_stop_count(gradients: &[Gradient]) -> usize {
    gradients
        .iter()
        .map(|gradient| match gradient {
            Gradient::Linear(linear) => linear.stops.len(),
            Gradient::Radial(_) | Gradient::Conic(_) => 0,
        })
        .sum()
}

impl UiEffectColorConfig {
    fn try_color(self) -> Result<Color, UiEffectError> {
        let color = self.unchecked_color();
        validate_color(color)?;
        Ok(color)
    }

    fn unchecked_color(self) -> Color {
        Color::srgba(self.r, self.g, self.b, self.a)
    }
}

fn validate_color(color: Color) -> Result<(), UiEffectError> {
    let value = color.to_srgba();
    if [value.red, value.green, value.blue, value.alpha]
        .into_iter()
        .all(|channel| channel.is_finite() && (0.0..=1.0).contains(&channel))
    {
        Ok(())
    } else {
        Err(UiEffectError::new(
            UiEffectErrorCode::InvalidColor,
            "effect colors require finite RGBA channels inside 0..=1",
        ))
    }
}

fn validate_range(name: &str, value: f32, minimum: f32, maximum: f32) -> Result<(), UiEffectError> {
    if value.is_finite() && (minimum..=maximum).contains(&value) {
        Ok(())
    } else {
        Err(UiEffectError::new(
            UiEffectErrorCode::InvalidValue,
            format!("{name} must be finite and inside {minimum}..={maximum}, got {value}"),
        ))
    }
}

const fn default_alpha() -> f32 {
    1.0
}

fn built_in_effect_catalog_config() -> UiEffectCatalogConfig {
    UiEffectCatalogConfig {
        presets: vec![
            UiEffectPresetConfig {
                name: UI_EFFECT_PRESET_GALLERY_SHADOW.to_owned(),
                box_shadows: vec![
                    shadow_config((0.0, 0.0, 0.0, 0.34), 0.0, 4.0, 0.0, 10.0),
                    shadow_config((0.0, 0.0, 0.0, 0.20), 0.0, 12.0, -2.0, 24.0),
                ],
                ..empty_preset_config()
            },
            UiEffectPresetConfig {
                name: UI_EFFECT_PRESET_GALLERY_TEXT_SHADOW.to_owned(),
                text_shadows: vec![shadow_config((0.0, 0.0, 0.0, 0.82), 2.0, 2.0, 0.0, 0.0)],
                ..empty_preset_config()
            },
            UiEffectPresetConfig {
                name: UI_EFFECT_PRESET_GALLERY_GRADIENT.to_owned(),
                background_gradient: Some(gradient_config(
                    112.0,
                    &[
                        (0.0, (0.06, 0.42, 0.39, 1.0)),
                        (0.52, (0.12, 0.25, 0.34, 0.96)),
                        (1.0, (0.40, 0.17, 0.24, 0.92)),
                    ],
                )),
                border_gradient: Some(gradient_config(
                    90.0,
                    &[
                        (0.0, (0.30, 0.94, 0.80, 1.0)),
                        (1.0, (0.98, 0.66, 0.27, 0.88)),
                    ],
                )),
                border: Some(UiBorderWidthsConfig {
                    left: 2.0,
                    right: 2.0,
                    top: 2.0,
                    bottom: 2.0,
                }),
                radius: Some(UiCornerRadiiConfig {
                    top_left: 8.0,
                    top_right: 8.0,
                    bottom_right: 8.0,
                    bottom_left: 8.0,
                }),
                ..empty_preset_config()
            },
            UiEffectPresetConfig {
                name: UI_EFFECT_PRESET_GALLERY_COMPOSITE.to_owned(),
                box_shadows: vec![shadow_config((0.0, 0.0, 0.0, 0.38), 0.0, 8.0, 1.0, 18.0)],
                background_gradient: Some(gradient_config(
                    145.0,
                    &[
                        (0.0, (0.11, 0.31, 0.37, 1.0)),
                        (1.0, (0.20, 0.12, 0.22, 1.0)),
                    ],
                )),
                border_gradient: Some(gradient_config(
                    35.0,
                    &[
                        (0.0, (0.34, 0.92, 0.79, 1.0)),
                        (1.0, (0.96, 0.64, 0.28, 1.0)),
                    ],
                )),
                border: Some(UiBorderWidthsConfig {
                    left: 4.0,
                    right: 1.0,
                    top: 2.0,
                    bottom: 5.0,
                }),
                radius: Some(UiCornerRadiiConfig {
                    top_left: 18.0,
                    top_right: 5.0,
                    bottom_right: 16.0,
                    bottom_left: 7.0,
                }),
                outline: Some(UiOutlineConfig {
                    width: 1.0,
                    offset: 2.0,
                    color: color_config((0.72, 0.78, 0.82, 0.72)),
                }),
                clip: true,
                ..empty_preset_config()
            },
            UiEffectPresetConfig {
                name: UI_EFFECT_PRESET_GALLERY_MATERIAL_FALLBACK.to_owned(),
                border: Some(UiBorderWidthsConfig {
                    left: 2.0,
                    right: 2.0,
                    top: 2.0,
                    bottom: 2.0,
                }),
                radius: Some(UiCornerRadiiConfig {
                    top_left: 8.0,
                    top_right: 8.0,
                    bottom_right: 8.0,
                    bottom_left: 8.0,
                }),
                outline: Some(UiOutlineConfig {
                    width: 1.0,
                    offset: 1.0,
                    color: color_config((1.0, 0.62, 0.36, 0.72)),
                }),
                material: Some(UiMaterialRequestConfig {
                    id: UiMaterialId::FrostedPanelV1.as_str().to_owned(),
                    scalars: vec![0.35, 0.82],
                    colors: vec![color_config((0.20, 0.76, 0.68, 0.82))],
                    texture_count: 0,
                }),
                material_fallback: Some(UiMaterialFallbackConfig {
                    background: color_config((0.24, 0.12, 0.10, 1.0)),
                    border: color_config((0.96, 0.52, 0.28, 1.0)),
                }),
                ..empty_preset_config()
            },
        ],
    }
}

fn empty_preset_config() -> UiEffectPresetConfig {
    UiEffectPresetConfig {
        name: String::new(),
        box_shadows: Vec::new(),
        text_shadows: Vec::new(),
        background_gradient: None,
        border_gradient: None,
        border: None,
        radius: None,
        outline: None,
        clip: false,
        material: None,
        material_fallback: None,
    }
}

fn shadow_config(
    color: (f32, f32, f32, f32),
    x_offset: f32,
    y_offset: f32,
    spread: f32,
    blur: f32,
) -> UiShadowLayerConfig {
    UiShadowLayerConfig {
        color: color_config(color),
        x_offset,
        y_offset,
        spread,
        blur,
    }
}

fn gradient_config(
    angle_degrees: f32,
    stops: &[(f32, (f32, f32, f32, f32))],
) -> UiLinearGradientConfig {
    UiLinearGradientConfig {
        angle_degrees,
        stops: stops
            .iter()
            .map(|(position, color)| UiGradientStopConfig {
                position: *position,
                color: color_config(*color),
            })
            .collect(),
    }
}

fn color_config((r, g, b, a): (f32, f32, f32, f32)) -> UiEffectColorConfig {
    UiEffectColorConfig { r, g, b, a }
}

fn unknown_effect_fallback_preset() -> UiCompiledEffectPreset {
    UiCompiledEffectPreset {
        name: "framework.unknown_effect_fallback".to_owned(),
        box_shadow: None,
        text_shadow: None,
        background_gradient: None,
        border_gradient: None,
        border: Some(UiRect::all(px(2))),
        radius: Some(BorderRadius::all(px(4))),
        overflow: None,
        outline: Some(Outline::new(
            px(1),
            px(1),
            Color::srgba(1.0, 0.24, 0.32, 0.9),
        )),
        material: None,
        material_fallback: None,
        budget: UiEffectBudget {
            requested_draw_call_upper_bound: 1,
            applied_draw_call_upper_bound: 1,
            overdraw_layers: 0,
            shadow_layers: 0,
            gradient_stops: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effect_app(runtime: UiMaterialRuntime) -> App {
        let mut app = App::new();
        app.insert_resource(UiTheme::default())
            .insert_resource(runtime)
            .add_systems(
                Update,
                (
                    resolve_ui_effect_bindings,
                    apply_resolved_ui_effects,
                    cleanup_removed_ui_effect_bindings,
                )
                    .chain(),
            );
        app
    }

    fn parse_catalog(source: &str) -> Result<UiEffectCatalog, UiEffectError> {
        let config: UiEffectCatalogConfig = ron::from_str(source).unwrap();
        UiEffectCatalog::compile(config)
    }

    fn assert_material_fallback_is_visible(
        runtime: UiMaterialRuntime,
        invalid_parameters: bool,
        expected: UiEffectErrorCode,
    ) {
        let mut app = effect_app(runtime);
        if invalid_parameters {
            let mut theme = app.world_mut().resource_mut::<UiTheme>();
            theme
                .effects
                .presets
                .get_mut(UI_EFFECT_PRESET_GALLERY_MATERIAL_FALLBACK)
                .unwrap()
                .material
                .as_mut()
                .unwrap()
                .scalars = vec![0.0; 5];
        }
        let entity = app
            .world_mut()
            .spawn((
                Node::default(),
                BackgroundColor(Color::BLACK),
                BorderColor::all(Color::BLACK),
                UiEffectBinding::new(UI_EFFECT_PRESET_GALLERY_MATERIAL_FALLBACK),
            ))
            .id();

        app.update();
        app.update();

        assert_eq!(
            app.world().get::<BackgroundColor>(entity).unwrap().0,
            Color::srgba(0.24, 0.12, 0.10, 1.0)
        );
        assert_eq!(
            app.world().get::<BorderColor>(entity).unwrap().top,
            Color::srgba(0.96, 0.52, 0.28, 1.0)
        );
        let debug = app
            .world()
            .get::<UiResolvedEffectDebugSnapshot>(entity)
            .unwrap();
        assert!(debug.fallback);
        assert_eq!(debug.error.as_deref(), Some(expected.as_str()));
        assert_eq!(
            debug.material.as_ref().unwrap().reason.as_deref(),
            Some(expected.as_str())
        );
        assert!(
            debug
                .applied_components
                .contains(&"material_fallback_background".to_owned())
        );
        assert!(
            debug
                .applied_components
                .contains(&"material_fallback_border".to_owned())
        );
    }

    #[test]
    fn built_in_catalog_contains_all_gallery_presets() {
        let catalog = UiEffectCatalog::built_in();
        for id in [
            UI_EFFECT_PRESET_GALLERY_SHADOW,
            UI_EFFECT_PRESET_GALLERY_TEXT_SHADOW,
            UI_EFFECT_PRESET_GALLERY_GRADIENT,
            UI_EFFECT_PRESET_GALLERY_COMPOSITE,
            UI_EFFECT_PRESET_GALLERY_MATERIAL_FALLBACK,
        ] {
            assert!(catalog.contains_preset(&UiEffectPresetId::new(id)));
        }
    }

    #[test]
    fn ron_config_compiles_bounded_shadows_gradients_and_geometry() {
        let catalog = parse_catalog(
            r#"(
                presets: [(
                    name: "test.full",
                    box_shadows: [
                        (color: (r: 0.0, g: 0.0, b: 0.0, a: 0.4), x_offset: 1.0, y_offset: 3.0, spread: -1.0, blur: 8.0),
                        (color: (r: 0.1, g: 0.2, b: 0.3, a: 0.2), x_offset: 0.0, y_offset: 9.0, blur: 20.0),
                    ],
                    background_gradient: Some((angle_degrees: 450.0, stops: [
                        (position: 0.0, color: (r: 0.1, g: 0.2, b: 0.3, a: 0.5)),
                        (position: 1.0, color: (r: 0.4, g: 0.5, b: 0.6)),
                    ])),
                    border_gradient: Some((angle_degrees: 180.0, stops: [
                        (position: 0.0, color: (r: 0.8, g: 0.2, b: 0.1)),
                        (position: 1.0, color: (r: 0.2, g: 0.8, b: 0.7, a: 0.7)),
                    ])),
                    border: Some((left: 1.0, right: 2.0, top: 3.0, bottom: 4.0)),
                    radius: Some((top_left: 5.0, top_right: 6.0, bottom_right: 7.0, bottom_left: 8.0)),
                    outline: Some((width: 2.0, offset: 1.0, color: (r: 0.9, g: 0.8, b: 0.7))),
                    clip: true,
                )],
            )"#,
        )
        .unwrap();

        let preset = catalog.get(&UiEffectPresetId::new("test.full")).unwrap();
        assert_eq!(preset.box_shadow.as_ref().unwrap().0.len(), 2);
        assert_eq!(preset.border.unwrap().left, px(1));
        assert_eq!(preset.border.unwrap().bottom, px(4));
        assert_eq!(preset.radius.unwrap().top_right, px(6));
        assert_eq!(preset.overflow, Some(Overflow::clip()));
        assert_eq!(preset.outline.unwrap().width, px(2));
        let Gradient::Linear(background) = &preset.background_gradient.as_ref().unwrap().0[0]
        else {
            panic!("background should compile to a Bevy linear gradient");
        };
        assert_eq!(background.angle, 90.0_f32.to_radians());
        assert_eq!(background.stops.len(), 2);
        assert_eq!(background.stops[0].point, percent(0));
        assert_eq!(background.stops[1].point, percent(100));
        assert_eq!(preset.budget.shadow_layers, 2);
        assert_eq!(preset.budget.gradient_stops, 4);
    }

    #[test]
    fn config_rejects_shadow_layer_overflow_and_invalid_numbers() {
        let mut too_many = empty_preset_config();
        too_many.name = "too-many".to_owned();
        too_many.box_shadows = (0..=MAX_BOX_SHADOW_LAYERS)
            .map(|_| shadow_config((0.0, 0.0, 0.0, 0.3), 0.0, 1.0, 0.0, 4.0))
            .collect();
        let error = compile_effect_preset(too_many).unwrap_err();
        assert_eq!(error.code, UiEffectErrorCode::TooManyShadowLayers);

        let mut negative_blur = empty_preset_config();
        negative_blur.name = "negative-blur".to_owned();
        negative_blur.box_shadows = vec![shadow_config((0.0, 0.0, 0.0, 0.3), 0.0, 1.0, 0.0, -1.0)];
        let error = compile_effect_preset(negative_blur).unwrap_err();
        assert_eq!(error.code, UiEffectErrorCode::InvalidValue);
    }

    #[test]
    fn text_shadow_rejects_bevy_unsupported_blur_spread_and_layers() {
        let mut blurred = empty_preset_config();
        blurred.name = "blurred-text".to_owned();
        blurred.text_shadows = vec![shadow_config((0.0, 0.0, 0.0, 0.5), 1.0, 1.0, 0.0, 2.0)];
        let error = compile_effect_preset(blurred).unwrap_err();
        assert_eq!(error.code, UiEffectErrorCode::TextShadowUnsupported);

        let mut layered = empty_preset_config();
        layered.name = "layered-text".to_owned();
        layered.text_shadows = vec![
            shadow_config((0.0, 0.0, 0.0, 0.5), 1.0, 1.0, 0.0, 0.0),
            shadow_config((0.0, 0.0, 0.0, 0.3), 2.0, 2.0, 0.0, 0.0),
        ];
        let error = compile_effect_preset(layered).unwrap_err();
        assert_eq!(error.code, UiEffectErrorCode::TextShadowUnsupported);
    }

    #[test]
    fn config_rejects_duplicate_presets_bad_stops_and_out_of_range_colors() {
        let error = parse_catalog(
            r#"(presets: [
                (name: "same"),
                (name: "same"),
            ])"#,
        )
        .unwrap_err();
        assert_eq!(error.code, UiEffectErrorCode::DuplicatePreset);

        let mut bad_stops = empty_preset_config();
        bad_stops.name = "bad-stops".to_owned();
        bad_stops.background_gradient = Some(gradient_config(
            0.0,
            &[(0.8, (0.0, 0.0, 0.0, 1.0)), (0.4, (1.0, 1.0, 1.0, 1.0))],
        ));
        let error = compile_effect_preset(bad_stops).unwrap_err();
        assert_eq!(error.code, UiEffectErrorCode::InvalidGradientStops);

        let mut bad_color = empty_preset_config();
        bad_color.name = "bad-color".to_owned();
        bad_color.outline = Some(UiOutlineConfig {
            width: 1.0,
            offset: 0.0,
            color: color_config((1.2, 0.0, 0.0, 1.0)),
        });
        let error = compile_effect_preset(bad_color).unwrap_err();
        assert_eq!(error.code, UiEffectErrorCode::InvalidColor);
    }

    #[test]
    fn material_policy_rejects_illegal_parameters_and_platform_failures() {
        let request = UiMaterialRequest {
            id: UiMaterialId::FrostedPanelV1,
            scalars: vec![0.0; 5],
            colors: Vec::new(),
            texture_count: 0,
        };
        let runtime = UiMaterialRuntime::for_test(UiMaterialPlatform::Windows, true);
        assert_eq!(
            resolve_material_request(&request, &runtime).fallback_reason,
            UiEffectErrorCode::MaterialInvalidParameters
        );

        let valid = UiMaterialRequest {
            scalars: vec![0.5],
            ..request.clone()
        };
        let runtime = UiMaterialRuntime::for_test(UiMaterialPlatform::Web, true);
        assert_eq!(
            resolve_material_request(&valid, &runtime).fallback_reason,
            UiEffectErrorCode::MaterialPlatformUnsupported
        );
        let runtime = UiMaterialRuntime::for_test(UiMaterialPlatform::Windows, false);
        assert_eq!(
            resolve_material_request(&valid, &runtime).fallback_reason,
            UiEffectErrorCode::MaterialGpuUnsupported
        );
    }

    #[test]
    fn material_policy_reports_loading_failure_missing_shader_and_adapter() {
        let request = UiMaterialRequest {
            id: UiMaterialId::FrostedPanelV1,
            scalars: vec![0.5],
            colors: Vec::new(),
            texture_count: 0,
        };
        let mut runtime = UiMaterialRuntime::for_test(UiMaterialPlatform::Windows, true);
        assert_eq!(
            resolve_material_request(&request, &runtime).fallback_reason,
            UiEffectErrorCode::MaterialShaderUnavailable
        );
        for (state, expected) in [
            (
                UiMaterialShaderState::Loading,
                UiEffectErrorCode::MaterialShaderLoading,
            ),
            (
                UiMaterialShaderState::Failed,
                UiEffectErrorCode::MaterialShaderLoadFailed,
            ),
            (
                UiMaterialShaderState::Loaded,
                UiEffectErrorCode::MaterialAdapterUnavailable,
            ),
        ] {
            runtime.report_shader_state(UiMaterialId::FrostedPanelV1, state);
            assert_eq!(
                resolve_material_request(&request, &runtime).fallback_reason,
                expected
            );
        }
    }

    #[test]
    fn ecs_application_writes_real_bevy_effect_components() {
        let mut app = effect_app(UiMaterialRuntime::for_test(
            UiMaterialPlatform::Windows,
            true,
        ));
        let shadow = app
            .world_mut()
            .spawn((
                Node::default(),
                UiEffectBinding::new(UI_EFFECT_PRESET_GALLERY_SHADOW),
            ))
            .id();
        let text = app
            .world_mut()
            .spawn((
                Node::default(),
                Text::new("shadow"),
                UiEffectBinding::new(UI_EFFECT_PRESET_GALLERY_TEXT_SHADOW),
            ))
            .id();
        let composite = app
            .world_mut()
            .spawn((
                Node::default(),
                BackgroundColor(Color::BLACK),
                BorderColor::all(Color::WHITE),
                UiEffectBinding::new(UI_EFFECT_PRESET_GALLERY_COMPOSITE),
            ))
            .id();

        app.update();
        app.update();

        let shadow_component = app.world().get::<BoxShadow>(shadow).unwrap();
        assert_eq!(shadow_component.0.len(), 2);
        assert_eq!(shadow_component.0[0].y_offset, px(4));
        assert_eq!(shadow_component.0[1].blur_radius, px(24));
        let text_shadow = app.world().get::<TextShadow>(text).unwrap();
        assert_eq!(text_shadow.offset, Vec2::splat(2.0));

        let entity = app.world().entity(composite);
        assert!(entity.contains::<BackgroundGradient>());
        assert!(entity.contains::<BorderGradient>());
        assert!(entity.contains::<BoxShadow>());
        assert_eq!(entity.get::<Outline>().unwrap().width, px(1));
        let node = entity.get::<Node>().unwrap();
        assert_eq!(node.border.left, px(4));
        assert_eq!(node.border.right, px(1));
        assert_eq!(node.border_radius.top_left, px(18));
        assert_eq!(node.border_radius.top_right, px(5));
        assert_eq!(node.overflow, Overflow::clip());
    }

    #[test]
    fn ecs_material_failure_is_visible_and_auditable() {
        let mut runtime = UiMaterialRuntime::for_test(UiMaterialPlatform::Windows, true);
        runtime.report_shader_state(UiMaterialId::FrostedPanelV1, UiMaterialShaderState::Failed);
        let mut app = effect_app(runtime);
        let entity = app
            .world_mut()
            .spawn((
                Node::default(),
                BackgroundColor(Color::BLACK),
                BorderColor::all(Color::BLACK),
                UiEffectBinding::new(UI_EFFECT_PRESET_GALLERY_MATERIAL_FALLBACK),
            ))
            .id();

        app.update();
        app.update();

        assert_eq!(
            app.world().get::<BackgroundColor>(entity).unwrap().0,
            Color::srgba(0.24, 0.12, 0.10, 1.0)
        );
        assert_eq!(
            app.world().get::<BorderColor>(entity).unwrap().top,
            Color::srgba(0.96, 0.52, 0.28, 1.0)
        );
        let debug = app
            .world()
            .get::<UiResolvedEffectDebugSnapshot>(entity)
            .unwrap();
        assert!(debug.fallback);
        assert_eq!(
            debug.error.as_deref(),
            Some(UiEffectErrorCode::MaterialShaderLoadFailed.as_str())
        );
        assert_eq!(debug.material.as_ref().unwrap().outcome, "fallback");
        assert!(
            debug
                .applied_components
                .contains(&"material_fallback_background".to_owned())
        );
        assert_eq!(debug.budget.requested_draw_call_upper_bound, 2);
        assert_eq!(debug.budget.applied_draw_call_upper_bound, 1);
    }

    #[test]
    fn every_material_failure_path_applies_the_visible_fallback() {
        assert_material_fallback_is_visible(
            UiMaterialRuntime::for_test(UiMaterialPlatform::Windows, true),
            true,
            UiEffectErrorCode::MaterialInvalidParameters,
        );
        assert_material_fallback_is_visible(
            UiMaterialRuntime::for_test(UiMaterialPlatform::Web, true),
            false,
            UiEffectErrorCode::MaterialPlatformUnsupported,
        );
        assert_material_fallback_is_visible(
            UiMaterialRuntime::for_test(UiMaterialPlatform::Windows, false),
            false,
            UiEffectErrorCode::MaterialGpuUnsupported,
        );
        assert_material_fallback_is_visible(
            UiMaterialRuntime::for_test(UiMaterialPlatform::Windows, true),
            false,
            UiEffectErrorCode::MaterialShaderUnavailable,
        );

        for (state, expected) in [
            (
                UiMaterialShaderState::Loading,
                UiEffectErrorCode::MaterialShaderLoading,
            ),
            (
                UiMaterialShaderState::Failed,
                UiEffectErrorCode::MaterialShaderLoadFailed,
            ),
            (
                UiMaterialShaderState::Loaded,
                UiEffectErrorCode::MaterialAdapterUnavailable,
            ),
        ] {
            let mut runtime = UiMaterialRuntime::for_test(UiMaterialPlatform::Windows, true);
            runtime.report_shader_state(UiMaterialId::FrostedPanelV1, state);
            assert_material_fallback_is_visible(runtime, false, expected);
        }
    }

    #[test]
    fn unknown_preset_uses_visible_non_panicking_fallback() {
        let mut app = effect_app(UiMaterialRuntime::for_test(
            UiMaterialPlatform::Windows,
            true,
        ));
        let entity = app
            .world_mut()
            .spawn((Node::default(), UiEffectBinding::new("missing.effect")))
            .id();
        app.update();
        app.update();

        assert!(app.world().entity(entity).contains::<Outline>());
        let debug = app
            .world()
            .get::<UiResolvedEffectDebugSnapshot>(entity)
            .unwrap();
        assert!(debug.fallback);
        assert_eq!(
            debug.error.as_deref(),
            Some(UiEffectErrorCode::UnknownPreset.as_str())
        );
        assert_eq!(debug.request, "missing.effect");
        assert_eq!(debug.resolved_preset, "framework.unknown_effect_fallback");
    }

    #[test]
    fn removing_binding_restores_original_components_and_node_fields() {
        let mut app = effect_app(UiMaterialRuntime::for_test(
            UiMaterialPlatform::Windows,
            true,
        ));
        let original_shadow =
            BoxShadow::new(Color::srgba(0.0, 0.0, 0.0, 0.2), px(1), px(2), px(0), px(3));
        let original_background = BackgroundColor(Color::srgb(0.2, 0.3, 0.4));
        let entity = app
            .world_mut()
            .spawn((
                Node {
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(3)),
                    overflow: Overflow::visible(),
                    ..default()
                },
                original_background,
                original_shadow.clone(),
                UiEffectBinding::new(UI_EFFECT_PRESET_GALLERY_COMPOSITE),
            ))
            .id();
        app.update();
        app.update();
        assert_eq!(app.world().get::<Node>(entity).unwrap().border.left, px(4));

        app.world_mut()
            .entity_mut(entity)
            .remove::<UiEffectBinding>();
        app.update();

        let node = app.world().get::<Node>(entity).unwrap();
        assert_eq!(node.border, UiRect::all(px(1)));
        assert_eq!(node.border_radius, BorderRadius::all(px(3)));
        assert_eq!(node.overflow, Overflow::visible());
        assert_eq!(app.world().get::<BoxShadow>(entity), Some(&original_shadow));
        assert_eq!(
            app.world().get::<BackgroundColor>(entity),
            Some(&original_background)
        );
        assert!(!app.world().entity(entity).contains::<BackgroundGradient>());
        assert!(!app.world().entity(entity).contains::<BorderGradient>());
        assert!(!app.world().entity(entity).contains::<Outline>());
        assert!(
            !app.world()
                .entity(entity)
                .contains::<UiResolvedEffectDebugSnapshot>()
        );
    }

    #[test]
    fn shadow_only_binding_does_not_freeze_unowned_node_or_color_fields() {
        let mut app = effect_app(UiMaterialRuntime::for_test(
            UiMaterialPlatform::Windows,
            true,
        ));
        let entity = app
            .world_mut()
            .spawn((
                Node {
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.1, 0.2, 0.3)),
                UiEffectBinding::new(UI_EFFECT_PRESET_GALLERY_SHADOW),
            ))
            .id();
        app.update();
        app.update();

        {
            let mut node = app.world_mut().get_mut::<Node>(entity).unwrap();
            node.border = UiRect::all(px(7));
            node.border_radius = BorderRadius::all(px(11));
        }
        app.world_mut()
            .get_mut::<BackgroundColor>(entity)
            .unwrap()
            .0 = Color::srgb(0.7, 0.3, 0.2);
        app.update();

        let node = app.world().get::<Node>(entity).unwrap();
        assert_eq!(node.border, UiRect::all(px(7)));
        assert_eq!(node.border_radius, BorderRadius::all(px(11)));
        assert_eq!(
            app.world().get::<BackgroundColor>(entity).unwrap().0,
            Color::srgb(0.7, 0.3, 0.2)
        );
        assert!(app.world().entity(entity).contains::<BoxShadow>());

        app.world_mut()
            .entity_mut(entity)
            .remove::<UiEffectBinding>();
        app.update();

        let node = app.world().get::<Node>(entity).unwrap();
        assert_eq!(node.border, UiRect::all(px(7)));
        assert_eq!(node.border_radius, BorderRadius::all(px(11)));
        assert_eq!(
            app.world().get::<BackgroundColor>(entity).unwrap().0,
            Color::srgb(0.7, 0.3, 0.2)
        );
        assert!(!app.world().entity(entity).contains::<BoxShadow>());
    }

    #[test]
    fn owned_external_updates_become_the_latest_unbind_baseline() {
        let mut app = effect_app(UiMaterialRuntime::for_test(
            UiMaterialPlatform::Windows,
            true,
        ));
        let latest_background = BackgroundColor(Color::srgb(0.16, 0.42, 0.58));
        let entity = app
            .world_mut()
            .spawn((
                Node {
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.1, 0.2, 0.3)),
                BorderColor::all(Color::srgb(0.3, 0.2, 0.1)),
                UiEffectBinding::new(UI_EFFECT_PRESET_GALLERY_MATERIAL_FALLBACK),
            ))
            .id();
        app.update();
        app.update();

        {
            let mut node = app.world_mut().get_mut::<Node>(entity).unwrap();
            node.border = UiRect::axes(px(7), px(9));
            node.border_radius = BorderRadius {
                top_left: px(11),
                top_right: px(12),
                bottom_right: px(13),
                bottom_left: px(14),
            };
        }
        *app.world_mut().get_mut::<BackgroundColor>(entity).unwrap() = latest_background;
        app.world_mut()
            .entity_mut(entity)
            .remove::<(BorderColor, Outline)>();

        app.update();

        let applied = app.world().entity(entity);
        assert_eq!(applied.get::<Node>().unwrap().border, UiRect::all(px(2)));
        assert_eq!(
            applied.get::<BackgroundColor>().unwrap().0,
            Color::srgba(0.24, 0.12, 0.10, 1.0)
        );
        assert!(applied.contains::<BorderColor>());
        assert!(applied.contains::<Outline>());

        app.world_mut()
            .entity_mut(entity)
            .remove::<UiEffectBinding>();
        app.update();

        let restored = app.world().entity(entity);
        let node = restored.get::<Node>().unwrap();
        assert_eq!(node.border, UiRect::axes(px(7), px(9)));
        assert_eq!(
            node.border_radius,
            BorderRadius {
                top_left: px(11),
                top_right: px(12),
                bottom_right: px(13),
                bottom_left: px(14),
            }
        );
        assert_eq!(restored.get::<BackgroundColor>(), Some(&latest_background));
        assert!(!restored.contains::<BorderColor>());
        assert!(!restored.contains::<Outline>());
        assert!(!restored.contains::<UiEffectLastApplied>());
    }

    #[test]
    fn switching_composite_to_shadow_restores_unowned_outputs_without_polluting_baseline() {
        let mut app = effect_app(UiMaterialRuntime::for_test(
            UiMaterialPlatform::Windows,
            true,
        ));
        let baseline_node = Node {
            border: UiRect::axes(px(3), px(5)),
            border_radius: BorderRadius::all(px(7)),
            overflow: Overflow::visible(),
            ..default()
        };
        let entity = app
            .world_mut()
            .spawn((
                baseline_node.clone(),
                UiEffectBinding::new(UI_EFFECT_PRESET_GALLERY_COMPOSITE),
            ))
            .id();
        app.update();
        app.update();

        app.world_mut()
            .get_mut::<UiEffectBinding>(entity)
            .unwrap()
            .preset = UiEffectPresetId::new(UI_EFFECT_PRESET_GALLERY_SHADOW);
        app.update();

        let switched = app.world().entity(entity);
        let node = switched.get::<Node>().unwrap();
        assert_eq!(node.border, baseline_node.border);
        assert_eq!(node.border_radius, baseline_node.border_radius);
        assert_eq!(node.overflow, baseline_node.overflow);
        assert!(!switched.contains::<BackgroundGradient>());
        assert!(!switched.contains::<BorderGradient>());
        assert!(!switched.contains::<Outline>());
        assert_eq!(switched.get::<BoxShadow>().unwrap().0.len(), 2);
        let last_applied = switched.get::<UiEffectLastApplied>().unwrap();
        assert!(last_applied.border.is_none());
        assert!(last_applied.radius.is_none());
        assert!(last_applied.overflow.is_none());
        assert!(last_applied.background_gradient.is_none());
        assert!(last_applied.border_gradient.is_none());
        assert!(last_applied.outline.is_none());
        assert_eq!(last_applied.box_shadow.as_ref().unwrap().0.len(), 2);

        app.world_mut()
            .entity_mut(entity)
            .remove::<UiEffectBinding>();
        app.update();
        assert!(!app.world().entity(entity).contains::<BoxShadow>());
    }

    #[test]
    fn switching_composite_to_unknown_clears_old_effects_and_applies_stable_fallback() {
        let mut app = effect_app(UiMaterialRuntime::for_test(
            UiMaterialPlatform::Windows,
            true,
        ));
        let entity = app
            .world_mut()
            .spawn((
                Node {
                    overflow: Overflow::visible(),
                    ..default()
                },
                UiEffectBinding::new(UI_EFFECT_PRESET_GALLERY_COMPOSITE),
            ))
            .id();
        app.update();
        app.update();

        app.world_mut()
            .get_mut::<UiEffectBinding>(entity)
            .unwrap()
            .preset = UiEffectPresetId::new("missing.effect.after.composite");
        app.update();

        let switched = app.world().entity(entity);
        assert!(!switched.contains::<BoxShadow>());
        assert!(!switched.contains::<BackgroundGradient>());
        assert!(!switched.contains::<BorderGradient>());
        let node = switched.get::<Node>().unwrap();
        assert_eq!(node.border, UiRect::all(px(2)));
        assert_eq!(node.border_radius, BorderRadius::all(px(4)));
        assert_eq!(node.overflow, Overflow::visible());
        let outline = switched.get::<Outline>().unwrap();
        assert_eq!(outline.width, px(1));
        assert_eq!(outline.color, Color::srgba(1.0, 0.24, 0.32, 0.9));
        let debug = switched.get::<UiResolvedEffectDebugSnapshot>().unwrap();
        assert_eq!(
            debug.error.as_deref(),
            Some(UiEffectErrorCode::UnknownPreset.as_str())
        );
        assert_eq!(debug.resolved_preset, "framework.unknown_effect_fallback");
    }

    #[test]
    fn stable_second_frame_does_not_mark_effect_outputs_changed() {
        let mut app = effect_app(UiMaterialRuntime::for_test(
            UiMaterialPlatform::Windows,
            true,
        ));
        let entity = app
            .world_mut()
            .spawn((
                Node::default(),
                UiEffectBinding::new(UI_EFFECT_PRESET_GALLERY_COMPOSITE),
            ))
            .id();
        app.update();
        app.update();
        app.world_mut().clear_trackers();

        app.update();

        let entity = app.world().entity(entity);
        assert!(!entity.get_ref::<Node>().unwrap().is_changed());
        assert!(!entity.get_ref::<BoxShadow>().unwrap().is_changed());
        assert!(!entity.get_ref::<BackgroundGradient>().unwrap().is_changed());
        assert!(!entity.get_ref::<BorderGradient>().unwrap().is_changed());
        assert!(!entity.get_ref::<Outline>().unwrap().is_changed());
        assert!(
            !entity
                .get_ref::<UiResolvedEffectDebugSnapshot>()
                .unwrap()
                .is_changed()
        );
    }
}
