pub(crate) mod effects;
pub(crate) mod fonts;
pub(crate) mod scopes;
pub(crate) mod theme;

#[allow(unused_imports)]
pub(crate) use effects::{
    UI_EFFECT_PRESET_GALLERY_COMPOSITE, UI_EFFECT_PRESET_GALLERY_GRADIENT,
    UI_EFFECT_PRESET_GALLERY_MATERIAL_FALLBACK, UI_EFFECT_PRESET_GALLERY_SHADOW,
    UI_EFFECT_PRESET_GALLERY_TEXT_SHADOW, UiEffectBinding, UiEffectBudgetSnapshot, UiEffectError,
    UiEffectErrorCode, UiEffectPresetId, UiMaterialId, UiMaterialPlatform, UiMaterialRuntime,
    UiMaterialShaderState, UiResolvedEffectDebugSnapshot,
};

#[allow(unused_imports)]
pub(crate) use fonts::{
    UiFontAssets, UiFontFamily, UiFontPlugin, UiFontResolution, UiFontResolutionStatus, UiFontRole,
    UiFontWeight, UiRasterizedTextError, UiRasterizedTextProvenance, UiRasterizedTextSpec,
    UiTextAlignment, UiTextLineHeight, UiTextStyleError, UiTextStyleToken, UiTextTruncation,
    UiTextWrap, try_ui_styled_text, try_ui_text_clip_frame,
};
#[allow(unused_imports)]
pub(crate) use scopes::{
    UI_STYLE_VARIANT_GALLERY_NESTED, UI_STYLE_VARIANT_GALLERY_PARENT, UiBorderStyleRole,
    UiButtonStyleRole, UiCardStyleRole, UiDialogStyleRole, UiInputStyleRole, UiResolvedButtonStyle,
    UiResolvedInputStyle, UiResolvedStyleDebugSnapshot, UiStyleBinding, UiStyleError,
    UiStyleErrorCode, UiStyleRef, UiStyleScope, UiStyleVariantId, UiSurfaceStyleRole,
    UiTextStyleRole,
};
pub(crate) use theme::{UiTheme, UiThemePlugin};
