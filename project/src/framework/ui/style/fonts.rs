use std::{
    collections::HashMap,
    fmt,
    path::{Component as PathComponent, Path},
};

use bevy::{asset::LoadState, prelude::*};
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

use crate::framework::ui::{
    i18n::UiI18nSystems,
    style::theme::{UiTheme, UiThemeSystems, UiThemeTextStyleRole},
};

const UI_FONT_CJK_REGULAR_PATH: &str = "ui/fonts/MyBevyUiCjk-Regular.otf";
const UI_FONT_FIGTREE_FIXTURE_REGULAR_PATH: &str = "ui/fixtures/fonts/FigtreeFixture-Regular.ttf";
const UI_FONT_FIGTREE_FIXTURE_MEDIUM_PATH: &str = "ui/fixtures/fonts/FigtreeFixture-Medium.ttf";
const UI_FONT_FIGTREE_FIXTURE_BOLD_PATH: &str = "ui/fixtures/fonts/FigtreeFixture-Bold.ttf";

const LATIN_FIXTURE_RANGES: &[UiUnicodeRange] = &[
    UiUnicodeRange::new(0x0020, 0x007e),
    UiUnicodeRange::new(0x00a0, 0x024f),
    UiUnicodeRange::new(0x2000, 0x206f),
    UiUnicodeRange::new(0x20a0, 0x20cf),
];

// This is the declared product UI contract, not a claim of full Unicode coverage.
const CJK_UI_RANGES: &[UiUnicodeRange] = &[
    UiUnicodeRange::new(0x0020, 0x007e),
    UiUnicodeRange::new(0x00a0, 0x024f),
    UiUnicodeRange::new(0x2000, 0x206f),
    UiUnicodeRange::new(0x20a0, 0x20cf),
    UiUnicodeRange::new(0x3000, 0x303f),
    UiUnicodeRange::new(0x4e00, 0x9fff),
    UiUnicodeRange::new(0xff01, 0xff60),
    UiUnicodeRange::new(0xffe0, 0xffee),
];

pub(crate) struct UiFontPlugin;

impl Plugin for UiFontPlugin {
    fn build(&self, app: &mut App) {
        let asset_server = app.world().resource::<AssetServer>();
        let cjk_regular = asset_server.load(UI_FONT_CJK_REGULAR_PATH);
        let figtree_regular = asset_server.load(UI_FONT_FIGTREE_FIXTURE_REGULAR_PATH);
        let figtree_medium = asset_server.load(UI_FONT_FIGTREE_FIXTURE_MEDIUM_PATH);
        let figtree_bold = asset_server.load(UI_FONT_FIGTREE_FIXTURE_BOLD_PATH);
        let assets = UiFontAssets::new(cjk_regular, figtree_regular, figtree_medium, figtree_bold);

        for face in assets.faces.values() {
            info!(
                path = face.asset_path,
                family = ?face.key.family,
                weight = ?face.key.weight,
                development_fixture = face.development_fixture,
                "registered ui font face"
            );
        }

        app.insert_resource(assets).add_systems(
            Update,
            sync_ui_styled_text
                .after(UiI18nSystems::Refresh)
                .after(UiThemeSystems::Refresh),
        );
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiFontFamily {
    ProductCjk,
    FigtreeFixture,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiFontWeight {
    Regular,
    Medium,
    Bold,
}

impl UiFontWeight {
    const fn bevy_weight(self) -> bevy::text::FontWeight {
        match self {
            Self::Regular => bevy::text::FontWeight::NORMAL,
            Self::Medium => bevy::text::FontWeight::MEDIUM,
            Self::Bold => bevy::text::FontWeight::BOLD,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiFontRole {
    Display,
    Heading,
    Body,
    Caption,
    Control,
    LatinFixture,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct UiFontFaceKey {
    pub family: UiFontFamily,
    pub weight: UiFontWeight,
}

impl UiFontFaceKey {
    pub(crate) const fn new(family: UiFontFamily, weight: UiFontWeight) -> Self {
        Self { family, weight }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UiUnicodeRange {
    pub start: u32,
    pub end: u32,
}

impl UiUnicodeRange {
    pub(crate) const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    fn contains(self, character: char) -> bool {
        let codepoint = character as u32;
        self.start <= codepoint && codepoint <= self.end
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiFontCoverage {
    LatinFixture,
    ProductCjkUi,
}

impl UiFontCoverage {
    pub(crate) const fn ranges(self) -> &'static [UiUnicodeRange] {
        match self {
            Self::LatinFixture => LATIN_FIXTURE_RANGES,
            Self::ProductCjkUi => CJK_UI_RANGES,
        }
    }

    pub(crate) fn supports(self, character: char) -> bool {
        character.is_whitespace() || self.ranges().iter().any(|range| range.contains(character))
    }

    pub(crate) fn supports_text(self, text: &str) -> bool {
        text.chars().all(|character| self.supports(character))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiFontLoadingBehavior {
    KeepRequestedHandle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiFontFailedBehavior {
    TryFallbackThenHide,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiFontMissingGlyphBehavior {
    ReplaceUnsupportedGraphemeWithQuestionMark,
}

#[derive(Clone, Debug)]
pub(crate) struct UiFontRoleSpec {
    pub role: UiFontRole,
    pub primary: UiFontFaceKey,
    pub fallbacks: Vec<UiFontFaceKey>,
    pub expected_coverage: UiFontCoverage,
    pub loading: UiFontLoadingBehavior,
    pub failed: UiFontFailedBehavior,
    pub missing_glyph: UiFontMissingGlyphBehavior,
}

#[derive(Clone, Debug)]
pub(crate) struct UiFontFace {
    pub key: UiFontFaceKey,
    pub asset_path: &'static str,
    pub coverage: UiFontCoverage,
    pub development_fixture: bool,
    pub handle: Handle<Font>,
}

#[derive(Clone, Debug, Resource)]
pub(crate) struct UiFontAssets {
    /// Compatibility entry point for existing pages. New text should resolve a role/style token.
    pub regular: Handle<Font>,
    faces: HashMap<UiFontFaceKey, UiFontFace>,
    roles: HashMap<UiFontRole, UiFontRoleSpec>,
}

impl UiFontAssets {
    fn new(
        cjk_regular: Handle<Font>,
        figtree_regular: Handle<Font>,
        figtree_medium: Handle<Font>,
        figtree_bold: Handle<Font>,
    ) -> Self {
        let faces = [
            UiFontFace {
                key: UiFontFaceKey::new(UiFontFamily::ProductCjk, UiFontWeight::Regular),
                asset_path: UI_FONT_CJK_REGULAR_PATH,
                coverage: UiFontCoverage::ProductCjkUi,
                development_fixture: false,
                handle: cjk_regular.clone(),
            },
            UiFontFace {
                key: UiFontFaceKey::new(UiFontFamily::FigtreeFixture, UiFontWeight::Regular),
                asset_path: UI_FONT_FIGTREE_FIXTURE_REGULAR_PATH,
                coverage: UiFontCoverage::LatinFixture,
                development_fixture: true,
                handle: figtree_regular,
            },
            UiFontFace {
                key: UiFontFaceKey::new(UiFontFamily::FigtreeFixture, UiFontWeight::Medium),
                asset_path: UI_FONT_FIGTREE_FIXTURE_MEDIUM_PATH,
                coverage: UiFontCoverage::LatinFixture,
                development_fixture: true,
                handle: figtree_medium,
            },
            UiFontFace {
                key: UiFontFaceKey::new(UiFontFamily::FigtreeFixture, UiFontWeight::Bold),
                asset_path: UI_FONT_FIGTREE_FIXTURE_BOLD_PATH,
                coverage: UiFontCoverage::LatinFixture,
                development_fixture: true,
                handle: figtree_bold,
            },
        ]
        .into_iter()
        .map(|face| (face.key, face))
        .collect();

        let cjk_regular_key = UiFontFaceKey::new(UiFontFamily::ProductCjk, UiFontWeight::Regular);
        let figtree_regular_key =
            UiFontFaceKey::new(UiFontFamily::FigtreeFixture, UiFontWeight::Regular);
        let mut roles = HashMap::new();
        for role in [
            UiFontRole::Display,
            UiFontRole::Heading,
            UiFontRole::Body,
            UiFontRole::Caption,
            UiFontRole::Control,
        ] {
            roles.insert(
                role,
                UiFontRoleSpec {
                    role,
                    primary: cjk_regular_key,
                    fallbacks: vec![figtree_regular_key],
                    expected_coverage: UiFontCoverage::ProductCjkUi,
                    loading: UiFontLoadingBehavior::KeepRequestedHandle,
                    failed: UiFontFailedBehavior::TryFallbackThenHide,
                    missing_glyph:
                        UiFontMissingGlyphBehavior::ReplaceUnsupportedGraphemeWithQuestionMark,
                },
            );
        }
        roles.insert(
            UiFontRole::LatinFixture,
            UiFontRoleSpec {
                role: UiFontRole::LatinFixture,
                primary: figtree_regular_key,
                fallbacks: vec![cjk_regular_key],
                expected_coverage: UiFontCoverage::LatinFixture,
                loading: UiFontLoadingBehavior::KeepRequestedHandle,
                failed: UiFontFailedBehavior::TryFallbackThenHide,
                missing_glyph:
                    UiFontMissingGlyphBehavior::ReplaceUnsupportedGraphemeWithQuestionMark,
            },
        );

        Self {
            regular: cjk_regular,
            faces,
            roles,
        }
    }

    #[cfg(test)]
    pub(crate) fn test_registry() -> Self {
        let handle = Handle::<Font>::default();
        Self::new(handle.clone(), handle.clone(), handle.clone(), handle)
    }

    pub(crate) fn face(&self, key: UiFontFaceKey) -> Option<&UiFontFace> {
        self.faces.get(&key)
    }

    pub(crate) fn role(&self, role: UiFontRole) -> Option<&UiFontRoleSpec> {
        self.roles.get(&role)
    }

    pub(crate) fn resolve_text_with_state(
        &self,
        style: &UiTextStyleToken,
        text: &str,
        mut state: impl FnMut(&UiFontFace) -> UiFontFaceLoadState,
    ) -> UiFontResolution {
        let role = self
            .role(style.font_role)
            .expect("built-in ui font role must be registered");
        debug_assert_eq!(role.role, style.font_role);
        debug_assert!(!role.expected_coverage.ranges().is_empty());
        let requested = UiFontFaceKey::new(style.font_family, style.font_weight);
        let candidates = self.candidate_faces(requested, role);

        if let Some(resolution) =
            self.select_candidate(&candidates, requested, text, 0, role, &mut state)
        {
            return resolution;
        }

        let (replacement_text, replacement_count) = match role.missing_glyph {
            UiFontMissingGlyphBehavior::ReplaceUnsupportedGraphemeWithQuestionMark => {
                replace_unsupported_graphemes(text, &candidates)
            }
        };
        if replacement_count > 0
            && let Some(resolution) = self.select_candidate(
                &candidates,
                requested,
                &replacement_text,
                replacement_count,
                role,
                &mut state,
            )
        {
            return resolution;
        }

        UiFontResolution {
            face: role.primary,
            rendered_source: String::new(),
            status: UiFontResolutionStatus::Unavailable,
        }
    }

    fn candidate_faces(&self, requested: UiFontFaceKey, role: &UiFontRoleSpec) -> Vec<&UiFontFace> {
        let mut keys = Vec::with_capacity(role.fallbacks.len() + 2);
        keys.push(requested);
        if !keys.contains(&role.primary) {
            keys.push(role.primary);
        }
        for fallback in &role.fallbacks {
            if !keys.contains(fallback) {
                keys.push(*fallback);
            }
        }
        keys.into_iter().filter_map(|key| self.face(key)).collect()
    }

    fn select_candidate(
        &self,
        candidates: &[&UiFontFace],
        requested: UiFontFaceKey,
        text: &str,
        replacement_count: usize,
        role: &UiFontRoleSpec,
        state: &mut impl FnMut(&UiFontFace) -> UiFontFaceLoadState,
    ) -> Option<UiFontResolution> {
        let requested_face = self.face(requested);
        let requested_supports_text =
            requested_face.is_some_and(|face| face.coverage.supports_text(text));
        let requested_failed =
            requested_face.is_some_and(|face| state(face) == UiFontFaceLoadState::Failed);

        for face in candidates {
            if !face.coverage.supports_text(text) {
                continue;
            }
            let load_state = state(face);
            if load_state == UiFontFaceLoadState::Failed {
                match role.failed {
                    UiFontFailedBehavior::TryFallbackThenHide => continue,
                }
            }
            if load_state != UiFontFaceLoadState::Loaded {
                match role.loading {
                    UiFontLoadingBehavior::KeepRequestedHandle => {}
                }
            }

            let used_fallback = face.key != requested;
            let status = if replacement_count > 0 {
                UiFontResolutionStatus::GlyphReplacement {
                    replacement_count,
                    loading: load_state != UiFontFaceLoadState::Loaded,
                    used_fallback,
                }
            } else if load_state != UiFontFaceLoadState::Loaded {
                UiFontResolutionStatus::Loading { used_fallback }
            } else if used_fallback {
                let reason = if requested_failed {
                    UiFontFallbackReason::ResourceFailed
                } else if requested_face.is_none() {
                    UiFontFallbackReason::WeightUnavailable
                } else if !requested_supports_text {
                    UiFontFallbackReason::Coverage
                } else {
                    UiFontFallbackReason::RoleFallback
                };
                UiFontResolutionStatus::Fallback(reason)
            } else {
                UiFontResolutionStatus::Ready
            };

            return Some(UiFontResolution {
                face: face.key,
                rendered_source: text.to_owned(),
                status,
            });
        }
        None
    }
}

fn replace_unsupported_graphemes(text: &str, candidates: &[&UiFontFace]) -> (String, usize) {
    let mut replacement_count = 0;
    let mut output = String::with_capacity(text.len());
    for grapheme in text.graphemes(true) {
        if candidates
            .iter()
            .any(|face| face.coverage.supports_text(grapheme))
        {
            output.push_str(grapheme);
        } else {
            output.push('?');
            replacement_count += 1;
        }
    }
    (output, replacement_count)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiFontFaceLoadState {
    NotLoaded,
    Loading,
    Loaded,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiFontFallbackReason {
    WeightUnavailable,
    Coverage,
    ResourceFailed,
    RoleFallback,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum UiFontResolutionStatus {
    Ready,
    Loading {
        used_fallback: bool,
    },
    Fallback(UiFontFallbackReason),
    GlyphReplacement {
        replacement_count: usize,
        loading: bool,
        used_fallback: bool,
    },
    InvalidStyle(UiTextStyleErrorCode),
    Unavailable,
}

#[derive(Clone, Debug, Component, Eq, PartialEq)]
pub(crate) struct UiFontResolution {
    pub face: UiFontFaceKey,
    pub rendered_source: String,
    pub status: UiFontResolutionStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiTextLineHeight {
    Relative(f32),
    Pixels(f32),
}

impl UiTextLineHeight {
    fn to_bevy(self) -> bevy::text::LineHeight {
        match self {
            Self::Relative(value) => bevy::text::LineHeight::RelativeToFont(value),
            Self::Pixels(value) => bevy::text::LineHeight::Px(value),
        }
    }

    fn value(self) -> f32 {
        match self {
            Self::Relative(value) | Self::Pixels(value) => value,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiTextAlignment {
    Left,
    Center,
    Right,
    Justified,
}

impl UiTextAlignment {
    const fn to_bevy(self) -> Justify {
        match self {
            Self::Left => Justify::Left,
            Self::Center => Justify::Center,
            Self::Right => Justify::Right,
            Self::Justified => Justify::Justified,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiTextWrap {
    Word,
    Character,
    WordOrCharacter,
    NoWrap,
}

impl UiTextWrap {
    const fn to_bevy(self) -> LineBreak {
        match self {
            Self::Word => LineBreak::WordBoundary,
            Self::Character => LineBreak::AnyCharacter,
            Self::WordOrCharacter => LineBreak::WordOrCharacter,
            Self::NoWrap => LineBreak::NoWrap,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiTextTruncation {
    #[default]
    None,
    /// Keeps the full string. A constrained parent with `Overflow::clip()` owns the hard clip.
    Clip,
    /// `max_graphemes` includes the ellipsis itself and never splits a grapheme cluster.
    Ellipsis { max_graphemes: usize },
}

#[derive(Clone, Debug, Component, Deserialize, PartialEq, Serialize)]
pub(crate) struct UiTextStyleToken {
    pub font_role: UiFontRole,
    pub font_family: UiFontFamily,
    pub font_weight: UiFontWeight,
    pub font_size: f32,
    pub line_height: UiTextLineHeight,
    pub alignment: UiTextAlignment,
    pub wrap: UiTextWrap,
    pub truncation: UiTextTruncation,
}

impl UiTextStyleToken {
    pub(crate) fn for_theme_role(theme: &UiTheme, role: UiThemeTextStyleRole) -> Self {
        let (font_role, font_weight, line_height, wrap) = match role {
            UiThemeTextStyleRole::TitleLarge => (
                UiFontRole::Display,
                UiFontWeight::Bold,
                UiTextLineHeight::Relative(1.15),
                UiTextWrap::WordOrCharacter,
            ),
            UiThemeTextStyleRole::Title => (
                UiFontRole::Heading,
                UiFontWeight::Medium,
                UiTextLineHeight::Relative(1.2),
                UiTextWrap::WordOrCharacter,
            ),
            UiThemeTextStyleRole::Subtitle => (
                UiFontRole::Body,
                UiFontWeight::Medium,
                UiTextLineHeight::Relative(1.3),
                UiTextWrap::WordOrCharacter,
            ),
            UiThemeTextStyleRole::SectionLabel => (
                UiFontRole::Heading,
                UiFontWeight::Medium,
                UiTextLineHeight::Relative(1.25),
                UiTextWrap::WordOrCharacter,
            ),
            UiThemeTextStyleRole::Body => (
                UiFontRole::Body,
                UiFontWeight::Regular,
                UiTextLineHeight::Relative(1.35),
                UiTextWrap::WordOrCharacter,
            ),
            UiThemeTextStyleRole::Caption => (
                UiFontRole::Caption,
                UiFontWeight::Regular,
                UiTextLineHeight::Relative(1.3),
                UiTextWrap::WordOrCharacter,
            ),
            UiThemeTextStyleRole::Button => (
                UiFontRole::Control,
                UiFontWeight::Medium,
                UiTextLineHeight::Relative(1.2),
                UiTextWrap::NoWrap,
            ),
        };
        Self {
            font_role,
            font_family: UiFontFamily::ProductCjk,
            font_weight,
            font_size: role.font_size(theme),
            line_height,
            alignment: UiTextAlignment::Left,
            wrap,
            truncation: UiTextTruncation::None,
        }
    }

    pub(crate) fn latin_fixture(weight: UiFontWeight, font_size: f32) -> Self {
        Self {
            font_role: UiFontRole::LatinFixture,
            font_family: UiFontFamily::FigtreeFixture,
            font_weight: weight,
            font_size,
            line_height: UiTextLineHeight::Relative(1.25),
            alignment: UiTextAlignment::Left,
            wrap: UiTextWrap::WordOrCharacter,
            truncation: UiTextTruncation::None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn parse_ron(source: &str) -> Result<Self, UiTextStyleParseError> {
        let style: Self = ron::from_str(source).map_err(|error| UiTextStyleParseError::Ron {
            detail: error.to_string(),
        })?;
        style
            .validate()
            .map_err(|error| UiTextStyleParseError::Invalid(error.code))?;
        Ok(style)
    }

    pub(crate) fn validate(&self) -> Result<(), UiTextStyleError> {
        if !self.font_size.is_finite() || self.font_size <= 0.0 {
            return Err(UiTextStyleError::new(
                UiTextStyleErrorCode::InvalidFontSize,
                "font_size must be finite and greater than zero",
            ));
        }
        if !self.line_height.value().is_finite() || self.line_height.value() <= 0.0 {
            return Err(UiTextStyleError::new(
                UiTextStyleErrorCode::InvalidLineHeight,
                "line_height must be finite and greater than zero",
            ));
        }
        if matches!(
            self.truncation,
            UiTextTruncation::Ellipsis { max_graphemes: 0 }
        ) {
            return Err(UiTextStyleError::new(
                UiTextStyleErrorCode::EmptyEllipsisBudget,
                "ellipsis max_graphemes must include at least the ellipsis",
            ));
        }
        Ok(())
    }

    fn text_layout(&self) -> TextLayout {
        TextLayout::new(self.alignment.to_bevy(), self.wrap.to_bevy())
    }

    fn truncate(&self, source: &str) -> String {
        match self.truncation {
            UiTextTruncation::None | UiTextTruncation::Clip => source.to_owned(),
            UiTextTruncation::Ellipsis { max_graphemes } => {
                truncate_with_ellipsis(source, max_graphemes)
            }
        }
    }
}

fn truncate_with_ellipsis(source: &str, max_graphemes: usize) -> String {
    let graphemes = source.graphemes(true).collect::<Vec<_>>();
    if graphemes.len() <= max_graphemes {
        return source.to_owned();
    }
    let mut output = graphemes[..max_graphemes.saturating_sub(1)].concat();
    output.push('…');
    output
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiTextStyleErrorCode {
    InvalidFontSize,
    InvalidLineHeight,
    EmptyEllipsisBudget,
    InvalidClipBounds,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UiTextStyleError {
    pub code: UiTextStyleErrorCode,
    pub detail: &'static str,
}

impl UiTextStyleError {
    const fn new(code: UiTextStyleErrorCode, detail: &'static str) -> Self {
        Self { code, detail }
    }
}

impl fmt::Display for UiTextStyleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.detail)
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum UiTextStyleParseError {
    Ron { detail: String },
    Invalid(UiTextStyleErrorCode),
}

#[derive(Clone, Debug, Component)]
pub(crate) struct UiStyledTextState {
    source: String,
    rendered: String,
}

#[derive(Bundle)]
pub(crate) struct UiStyledTextBundle {
    pub text: Text,
    pub font: TextFont,
    pub color: TextColor,
    pub layout: TextLayout,
    pub line_height: bevy::text::LineHeight,
    pub style: UiTextStyleToken,
    pub resolution: UiFontResolution,
    state: UiStyledTextState,
}

#[derive(Component)]
pub(crate) struct UiTextClipFrame;

#[derive(Bundle)]
pub(crate) struct UiTextClipFrameBundle {
    pub node: Node,
    pub marker: UiTextClipFrame,
}

pub(crate) fn try_ui_text_clip_frame(
    width: f32,
    height: f32,
) -> Result<UiTextClipFrameBundle, UiTextStyleError> {
    if !width.is_finite() || width <= 0.0 || !height.is_finite() || height <= 0.0 {
        return Err(UiTextStyleError::new(
            UiTextStyleErrorCode::InvalidClipBounds,
            "clip frame width and height must be finite and greater than zero",
        ));
    }
    Ok(UiTextClipFrameBundle {
        node: Node {
            width: px(width),
            height: px(height),
            overflow: Overflow::clip(),
            ..default()
        },
        marker: UiTextClipFrame,
    })
}

pub(crate) fn try_ui_styled_text(
    fonts: &UiFontAssets,
    text: impl Into<String>,
    style: UiTextStyleToken,
    color: Color,
) -> Result<UiStyledTextBundle, UiTextStyleError> {
    style.validate()?;
    let source = text.into();
    let resolution =
        fonts.resolve_text_with_state(&style, &source, |_| UiFontFaceLoadState::Loading);
    let rendered = style.truncate(&resolution.rendered_source);
    let face = fonts
        .face(resolution.face)
        .expect("font resolution must reference a registered face");
    Ok(UiStyledTextBundle {
        text: Text::new(rendered.clone()),
        font: TextFont {
            font: face.handle.clone(),
            font_size: style.font_size,
            weight: face.key.weight.bevy_weight(),
            ..default()
        },
        color: TextColor(color),
        layout: style.text_layout(),
        line_height: style.line_height.to_bevy(),
        style,
        resolution,
        state: UiStyledTextState { source, rendered },
    })
}

fn sync_ui_styled_text(
    fonts: Res<UiFontAssets>,
    asset_server: Res<AssetServer>,
    mut texts: Query<(
        &mut Text,
        &UiTextStyleToken,
        &mut TextFont,
        &mut TextLayout,
        &mut bevy::text::LineHeight,
        &mut UiFontResolution,
        &mut UiStyledTextState,
    )>,
) {
    for (mut text, style, mut font, mut layout, mut line_height, mut resolution, mut state) in
        &mut texts
    {
        if text.0 != state.rendered && text.0 != state.source {
            state.source.clone_from(&text.0);
        }

        if let Err(error) = style.validate() {
            let primary = fonts
                .role(style.font_role)
                .expect("built-in ui font role must be registered")
                .primary;
            let next_resolution = UiFontResolution {
                face: primary,
                rendered_source: String::new(),
                status: UiFontResolutionStatus::InvalidStyle(error.code),
            };
            if *resolution != next_resolution {
                *resolution = next_resolution;
            }
            if !text.0.is_empty() {
                text.0.clear();
            }
            if !state.rendered.is_empty() {
                state.rendered.clear();
            }
            continue;
        }

        let next_resolution = fonts.resolve_text_with_state(style, &state.source, |face| {
            match asset_server.load_state(face.handle.id()) {
                LoadState::NotLoaded => UiFontFaceLoadState::NotLoaded,
                LoadState::Loading => UiFontFaceLoadState::Loading,
                LoadState::Loaded => UiFontFaceLoadState::Loaded,
                LoadState::Failed(_) => UiFontFaceLoadState::Failed,
            }
        });
        let rendered = style.truncate(&next_resolution.rendered_source);
        let face = fonts
            .face(next_resolution.face)
            .expect("font resolution must reference a registered face");

        if text.0 != rendered {
            text.0.clone_from(&rendered);
        }
        if font.font != face.handle {
            font.font = face.handle.clone();
        }
        if font.font_size != style.font_size {
            font.font_size = style.font_size;
        }
        let weight = face.key.weight.bevy_weight();
        if font.weight != weight {
            font.weight = weight;
        }
        let next_layout = style.text_layout();
        if layout.justify != next_layout.justify || layout.linebreak != next_layout.linebreak {
            *layout = next_layout;
        }
        let next_line_height = style.line_height.to_bevy();
        if *line_height != next_line_height {
            *line_height = next_line_height;
        }
        if *resolution != next_resolution {
            *resolution = next_resolution;
        }
        if state.rendered != rendered {
            state.rendered = rendered;
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum UiRasterizedTextProvenance {
    ProjectOwned,
    Licensed { source: String, license_id: String },
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UiRasterizedTextSpec {
    pub asset_path: String,
    pub accessible_fallback: String,
    pub i18n_fallback_key: String,
    pub provenance: UiRasterizedTextProvenance,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiRasterizedTextError {
    PathOutsideUiAssets,
    MissingAccessibleFallback,
    MissingI18nFallbackKey,
    MissingProvenance,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) struct UiRasterizedTextAsset {
    pub image: Handle<Image>,
    pub accessible_fallback: String,
    pub i18n_fallback_key: String,
}

#[allow(dead_code)]
impl UiRasterizedTextSpec {
    pub(crate) fn validate(&self) -> Result<(), UiRasterizedTextError> {
        let path = self.asset_path.as_str();
        let path_ref = Path::new(path);
        let has_supported_extension = path_ref
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "png" | "jpg" | "jpeg" | "webp"
                )
            });
        let path_is_valid = !path.is_empty()
            && path.trim() == path
            && path.starts_with("ui/")
            && !path.contains('\\')
            && !path.contains(':')
            && !path
                .split('/')
                .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
            && !path_ref.is_absolute()
            && !path_ref.components().any(|component| {
                matches!(
                    component,
                    PathComponent::Prefix(_)
                        | PathComponent::RootDir
                        | PathComponent::CurDir
                        | PathComponent::ParentDir
                )
            })
            && has_supported_extension;
        if !path_is_valid {
            return Err(UiRasterizedTextError::PathOutsideUiAssets);
        }
        if self.accessible_fallback.trim().is_empty() {
            return Err(UiRasterizedTextError::MissingAccessibleFallback);
        }
        if self.i18n_fallback_key.trim().is_empty() {
            return Err(UiRasterizedTextError::MissingI18nFallbackKey);
        }
        if let UiRasterizedTextProvenance::Licensed { source, license_id } = &self.provenance
            && (source.trim().is_empty() || license_id.trim().is_empty())
        {
            return Err(UiRasterizedTextError::MissingProvenance);
        }
        Ok(())
    }

    pub(crate) fn load(
        &self,
        asset_server: &AssetServer,
    ) -> Result<UiRasterizedTextAsset, UiRasterizedTextError> {
        self.validate()?;
        Ok(UiRasterizedTextAsset {
            image: asset_server.load(self.asset_path.clone()),
            accessible_fallback: self.accessible_fallback.clone(),
            i18n_fallback_key: self.i18n_fallback_key.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;

    fn latin_style(weight: UiFontWeight) -> UiTextStyleToken {
        UiTextStyleToken::latin_fixture(weight, 20.0)
    }

    fn font_runtime_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>();
        app.finish();
        app.cleanup();
        app.insert_resource(UiFontAssets::test_registry());
        app
    }

    fn valid_rasterized_text_spec() -> UiRasterizedTextSpec {
        UiRasterizedTextSpec {
            asset_path: "ui/text/title.png".to_owned(),
            accessible_fallback: "Game title".to_owned(),
            i18n_fallback_key: "title.game".to_owned(),
            provenance: UiRasterizedTextProvenance::Licensed {
                source: "https://example.invalid/source".to_owned(),
                license_id: "OFL-1.1".to_owned(),
            },
        }
    }

    fn assert_styled_text_component_changes(world: &World, entity: Entity, expected: bool) {
        let entity = world.entity(entity);
        assert_eq!(entity.get_ref::<Text>().unwrap().is_changed(), expected);
        assert_eq!(entity.get_ref::<TextFont>().unwrap().is_changed(), expected);
        assert_eq!(
            entity.get_ref::<TextLayout>().unwrap().is_changed(),
            expected
        );
        assert_eq!(
            entity
                .get_ref::<bevy::text::LineHeight>()
                .unwrap()
                .is_changed(),
            expected
        );
        assert_eq!(
            entity.get_ref::<UiFontResolution>().unwrap().is_changed(),
            expected
        );
        assert_eq!(
            entity.get_ref::<UiStyledTextState>().unwrap().is_changed(),
            expected
        );
    }

    #[test]
    fn registry_contains_real_regular_medium_and_bold_fixture_faces() {
        let fonts = UiFontAssets::test_registry();
        for weight in [
            UiFontWeight::Regular,
            UiFontWeight::Medium,
            UiFontWeight::Bold,
        ] {
            let face = fonts
                .face(UiFontFaceKey::new(UiFontFamily::FigtreeFixture, weight))
                .unwrap();
            assert!(face.development_fixture);
            assert_eq!(face.coverage, UiFontCoverage::LatinFixture);
        }
    }

    #[test]
    fn every_font_role_declares_primary_fallback_coverage_and_failure_policy() {
        let fonts = UiFontAssets::test_registry();
        for role in [
            UiFontRole::Display,
            UiFontRole::Heading,
            UiFontRole::Body,
            UiFontRole::Caption,
            UiFontRole::Control,
            UiFontRole::LatinFixture,
        ] {
            let spec = fonts.role(role).unwrap();
            assert_eq!(spec.role, role);
            assert!(fonts.face(spec.primary).is_some());
            assert!(!spec.fallbacks.is_empty());
            assert!(!spec.expected_coverage.ranges().is_empty());
            assert_eq!(spec.loading, UiFontLoadingBehavior::KeepRequestedHandle);
            assert_eq!(spec.failed, UiFontFailedBehavior::TryFallbackThenHide);
        }
    }

    #[test]
    fn resolves_each_fixture_weight_without_synthetic_weight() {
        let fonts = UiFontAssets::test_registry();
        for weight in [
            UiFontWeight::Regular,
            UiFontWeight::Medium,
            UiFontWeight::Bold,
        ] {
            let resolution =
                fonts.resolve_text_with_state(&latin_style(weight), "Weight 123!?", |_| {
                    UiFontFaceLoadState::Loaded
                });
            assert_eq!(
                resolution.face,
                UiFontFaceKey::new(UiFontFamily::FigtreeFixture, weight)
            );
            assert_eq!(resolution.status, UiFontResolutionStatus::Ready);
        }
    }

    #[test]
    fn mixed_chinese_uses_whole_node_cjk_fallback() {
        let fonts = UiFontAssets::test_registry();
        let resolution = fonts.resolve_text_with_state(
            &latin_style(UiFontWeight::Bold),
            "MyBevy 混排 2026，OK!",
            |_| UiFontFaceLoadState::Loaded,
        );

        assert_eq!(
            resolution.face,
            UiFontFaceKey::new(UiFontFamily::ProductCjk, UiFontWeight::Regular)
        );
        assert_eq!(
            resolution.status,
            UiFontResolutionStatus::Fallback(UiFontFallbackReason::Coverage)
        );
    }

    #[test]
    fn failed_requested_face_uses_declared_fallback() {
        let fonts = UiFontAssets::test_registry();
        let requested = UiFontFaceKey::new(UiFontFamily::FigtreeFixture, UiFontWeight::Medium);
        let resolution =
            fonts.resolve_text_with_state(&latin_style(UiFontWeight::Medium), "fallback", |face| {
                if face.key == requested {
                    UiFontFaceLoadState::Failed
                } else {
                    UiFontFaceLoadState::Loaded
                }
            });

        assert_eq!(
            resolution.status,
            UiFontResolutionStatus::Fallback(UiFontFallbackReason::ResourceFailed)
        );
        assert_ne!(resolution.face, requested);
    }

    #[test]
    fn missing_emoji_is_replaced_explicitly_instead_of_claiming_tofu_success() {
        let fonts = UiFontAssets::test_registry();
        let resolution = fonts.resolve_text_with_state(
            &latin_style(UiFontWeight::Regular),
            "ready 🙂",
            |_| UiFontFaceLoadState::Loaded,
        );

        assert_eq!(resolution.rendered_source, "ready ?");
        assert_eq!(
            resolution.status,
            UiFontResolutionStatus::GlyphReplacement {
                replacement_count: 1,
                loading: false,
                used_fallback: false,
            }
        );
    }

    #[test]
    fn ellipsis_truncation_respects_unicode_grapheme_boundaries() {
        assert_eq!(truncate_with_ellipsis("ab中文cd", 5), "ab中文…");
        assert_eq!(truncate_with_ellipsis("A👨‍👩‍👧‍👦BC", 3), "A👨‍👩‍👧‍👦…");
        assert_eq!(truncate_with_ellipsis("短文", 4), "短文");
    }

    #[test]
    fn style_ron_parsing_maps_all_supported_layout_fields() {
        let style = UiTextStyleToken::parse_ron(
            r#"(
                font_role: latin_fixture,
                font_family: figtree_fixture,
                font_weight: medium,
                font_size: 18.0,
                line_height: relative(1.4),
                alignment: center,
                wrap: word_or_character,
                truncation: ellipsis(max_graphemes: 12),
            )"#,
        )
        .unwrap();

        assert_eq!(style.font_weight, UiFontWeight::Medium);
        assert_eq!(style.line_height, UiTextLineHeight::Relative(1.4));
        assert_eq!(style.alignment, UiTextAlignment::Center);
        assert_eq!(style.wrap, UiTextWrap::WordOrCharacter);
        assert_eq!(
            style.truncation,
            UiTextTruncation::Ellipsis { max_graphemes: 12 }
        );
    }

    #[test]
    fn style_validation_rejects_invalid_sizes_and_empty_ellipsis_budget() {
        let mut style = latin_style(UiFontWeight::Regular);
        style.font_size = f32::NAN;
        assert_eq!(
            style.validate().unwrap_err().code,
            UiTextStyleErrorCode::InvalidFontSize
        );
        style.font_size = 20.0;
        style.line_height = UiTextLineHeight::Relative(0.0);
        assert_eq!(
            style.validate().unwrap_err().code,
            UiTextStyleErrorCode::InvalidLineHeight
        );
        style.line_height = UiTextLineHeight::Relative(1.2);
        style.truncation = UiTextTruncation::Ellipsis { max_graphemes: 0 };
        assert_eq!(
            style.validate().unwrap_err().code,
            UiTextStyleErrorCode::EmptyEllipsisBudget
        );
    }

    #[test]
    fn clip_frame_uses_parent_overflow_and_rejects_invalid_bounds() {
        let frame = try_ui_text_clip_frame(280.0, 32.0).unwrap();
        assert_eq!(frame.node.width, px(280));
        assert_eq!(frame.node.height, px(32));
        assert_eq!(frame.node.overflow, Overflow::clip());
        assert_eq!(
            try_ui_text_clip_frame(0.0, 32.0).err().unwrap().code,
            UiTextStyleErrorCode::InvalidClipBounds
        );
        assert_eq!(
            try_ui_text_clip_frame(280.0, f32::NAN).err().unwrap().code,
            UiTextStyleErrorCode::InvalidClipBounds
        );
    }

    #[test]
    fn sync_styled_text_does_not_mark_stable_components_changed_again() {
        let mut app = font_runtime_test_app();
        let bundle = try_ui_styled_text(
            app.world().resource::<UiFontAssets>(),
            "Initial text",
            latin_style(UiFontWeight::Medium),
            Color::WHITE,
        )
        .unwrap();
        let entity = app.world_mut().spawn(bundle).id();
        {
            let mut entity = app.world_mut().entity_mut(entity);
            entity.get_mut::<Text>().unwrap().0 = "Updated 🙂".to_owned();
            entity.get_mut::<TextFont>().unwrap().font_size = 1.0;
            *entity.get_mut::<TextLayout>().unwrap() = TextLayout::default();
            *entity.get_mut::<bevy::text::LineHeight>().unwrap() = bevy::text::LineHeight::Px(1.0);
            let mut resolution = entity.get_mut::<UiFontResolution>().unwrap();
            resolution.rendered_source.clear();
            resolution.status = UiFontResolutionStatus::Unavailable;
        }
        let mut schedule = Schedule::default();
        schedule.add_systems(sync_ui_styled_text);

        app.world_mut().clear_trackers();
        schedule.run(app.world_mut());
        assert_styled_text_component_changes(app.world(), entity, true);
        assert_eq!(
            app.world().entity(entity).get::<Text>().unwrap().0.as_str(),
            "Updated ?"
        );

        app.world_mut().clear_trackers();
        schedule.run(app.world_mut());
        assert_styled_text_component_changes(app.world(), entity, false);
    }

    #[test]
    fn sync_styled_text_invalid_style_is_change_stable_after_first_frame() {
        let mut app = font_runtime_test_app();
        let bundle = try_ui_styled_text(
            app.world().resource::<UiFontAssets>(),
            "Invalid style source",
            latin_style(UiFontWeight::Regular),
            Color::WHITE,
        )
        .unwrap();
        let entity = app.world_mut().spawn(bundle).id();
        app.world_mut()
            .entity_mut(entity)
            .get_mut::<UiTextStyleToken>()
            .unwrap()
            .font_size = f32::NAN;
        let mut schedule = Schedule::default();
        schedule.add_systems(sync_ui_styled_text);

        app.world_mut().clear_trackers();
        schedule.run(app.world_mut());
        let entity_ref = app.world().entity(entity);
        assert!(entity_ref.get_ref::<Text>().unwrap().is_changed());
        assert!(
            entity_ref
                .get_ref::<UiFontResolution>()
                .unwrap()
                .is_changed()
        );
        assert!(
            entity_ref
                .get_ref::<UiStyledTextState>()
                .unwrap()
                .is_changed()
        );
        assert!(entity_ref.get::<Text>().unwrap().0.is_empty());
        assert!(matches!(
            &entity_ref.get::<UiFontResolution>().unwrap().status,
            UiFontResolutionStatus::InvalidStyle(UiTextStyleErrorCode::InvalidFontSize)
        ));

        app.world_mut().clear_trackers();
        schedule.run(app.world_mut());
        assert_styled_text_component_changes(app.world(), entity, false);
    }

    #[test]
    fn rasterized_text_rejects_unsafe_asset_paths() {
        let valid = valid_rasterized_text_spec();
        assert_eq!(valid.validate(), Ok(()));

        for path in [
            "",
            " ui/text.png",
            "ui/text.png ",
            "ui//text.png",
            "ui/./text.png",
            "ui/../text.png",
            "../ui/text.png",
            "/ui/text.png",
            "C:/ui/text.png",
            "ui/C:text.png",
            r"ui\text.png",
            r"\\server\share\text.png",
            "ui/text.bmp",
        ] {
            let mut invalid = valid.clone();
            invalid.asset_path = path.to_owned();
            assert_eq!(
                invalid.validate(),
                Err(UiRasterizedTextError::PathOutsideUiAssets),
                "path should be rejected: {path:?}"
            );
        }
    }

    #[test]
    fn rasterized_text_rejects_invalid_path_before_asset_load() {
        let app = font_runtime_test_app();
        let asset_server = app.world().resource::<AssetServer>();
        let path = "ui/text/invalid-rasterized-text.bmp";
        let mut invalid = valid_rasterized_text_spec();
        invalid.asset_path = path.to_owned();

        assert!(asset_server.get_handle::<Image>(path).is_none());
        assert!(matches!(
            invalid.load(asset_server),
            Err(UiRasterizedTextError::PathOutsideUiAssets)
        ));
        assert!(asset_server.get_handle::<Image>(path).is_none());
    }

    #[test]
    fn rasterized_text_requires_accessible_i18n_and_provenance() {
        let valid = valid_rasterized_text_spec();
        let mut invalid = valid.clone();
        invalid.accessible_fallback.clear();
        assert_eq!(
            invalid.validate(),
            Err(UiRasterizedTextError::MissingAccessibleFallback)
        );
        invalid = valid.clone();
        invalid.i18n_fallback_key = "  ".to_owned();
        assert_eq!(
            invalid.validate(),
            Err(UiRasterizedTextError::MissingI18nFallbackKey)
        );
        invalid = valid;
        invalid.provenance = UiRasterizedTextProvenance::Licensed {
            source: String::new(),
            license_id: String::new(),
        };
        assert_eq!(
            invalid.validate(),
            Err(UiRasterizedTextError::MissingProvenance)
        );
    }
}
