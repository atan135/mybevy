use bevy::{ecs::hierarchy::ChildSpawnerCommands, prelude::*};

use crate::framework::ui::{
    core::{
        UI_PANEL_GLOBAL_LOADING, UiFloatingPanel, UiLayer, UiLayerRoot, UiMetrics, UiPanelCommand,
        UiPanelId, UiPanelKind, UiPanelRequest, UiPanelRoot, UiViewport, UiWidthClass,
        binding::{UiBindingValues, UiBoundDisabled, UiBoundText, UiBoundVisibility},
    },
    i18n::{UiI18n, UiI18nText},
    overlays::{
        UiConfirmModal, UiI18nTextSpec, UiLoading, UiModalActionSpec, UiModalActionStyle,
        UiOverlayCommand, UiToast,
    },
    style::{
        UI_STYLE_VARIANT_GALLERY_NESTED, UI_STYLE_VARIANT_GALLERY_PARENT, UiBorderStyleRole,
        UiButtonStyleRole, UiFontAssets, UiFontWeight, UiStyleBinding, UiStyleScope,
        UiSurfaceStyleRole, UiTextAlignment, UiTextStyleRole, UiTextStyleToken, UiTextTruncation,
        UiTextWrap, UiTheme,
        theme::{
            UiThemeBackgroundRole, UiThemeBorderRole, UiThemePanelNodeRole, UiThemeRootNodeRole,
            UiThemeTextColorRole, UiThemeTextStyleRole,
        },
        try_ui_styled_text, try_ui_text_clip_frame,
    },
    widgets::{
        DisabledTextInput, FocusedButton, ReadonlyTextInput, SelectedButton, UiAdvancedImageMode,
        UiAdvancedImageSource, UiAdvancedImageSpec, UiAlign, UiAtlasFrame, UiButtonEvent,
        UiButtonEventKind, UiButtonVisualState, UiIconId, UiIconLabelPlacement, UiImageConstraints,
        UiImageFit, UiImageFocus, UiImageLength, UiImagePivot, UiImagePixelRect, UiImagePixelSize,
        UiImageSize, UiImageTextureSource, UiImageTiling, UiJustify, UiNineSlice,
        UiResponsiveGridColumns, UiTextInputAlphanumeric, UiTextInputError, UiTextInputHelperText,
        UiTextInputMaxChars, UiTextInputRequired, UiTextInputSubmitted,
        UiTextInputValidationMessage, UiTileAxis, checkbox_key, checked_checkbox_key,
        disabled_checkbox_key, disabled_icon_button_key, disabled_primary_action_button_key,
        disabled_secondary_action_button_key, disabled_segment_option_key, disabled_slider_key,
        disabled_stepper_key, disabled_toggle_key, icon_button_key, icon_label_button_key,
        image_button_key, loading_icon_button_key, loading_primary_action_button_key,
        primary_action_button_key, screen_label, screen_label_key, screen_title_key,
        secondary_action_button_key, segment_option_key, segmented_control,
        selected_segment_option_key, slider_key, stepper_key, text_input, text_input_form_message,
        toggle_key, toggle_on_key, try_ui_advanced_image, ui_column, ui_image, ui_image_panel_node,
        ui_image_panel_node_with_radius, ui_responsive_column, ui_responsive_grid,
        ui_scroll_column, ui_thumbnail_grid,
    },
};
use crate::game::{
    navigation::{AppUiMode, game_panel_root, secondary_route_button_key},
    ui_ids::{
        ACTION_CANCEL, ACTION_CONFIRM, ANCHOR_UI_GALLERY_ICON_STATES, ANCHOR_UI_GALLERY_ICONS,
        ANCHOR_UI_GALLERY_IMAGE_ATLAS, ANCHOR_UI_GALLERY_IMAGE_MODES,
        ANCHOR_UI_GALLERY_IMAGE_TILING, ANCHOR_UI_GALLERY_STYLE_SCOPES,
        ANCHOR_UI_GALLERY_TYPOGRAPHY, ANCHOR_UI_GALLERY_TYPOGRAPHY_OVERFLOW, MODAL_GALLERY_CONFIRM,
        OWNER_UI_GALLERY, PANEL_GALLERY_FLOATING, PANEL_UI_GALLERY, SCROLL_UI_GALLERY_MAIN,
    },
};

const GALLERY_STRESS_ITEM_COUNT: usize = 96;
const GALLERY_VISUAL_FIXTURE_PATHS: [&str; 4] = [
    "ui/fixtures/visual-foundation/transparent-edge.png",
    "ui/fixtures/visual-foundation/non-square-2x1.png",
    "ui/fixtures/visual-foundation/nine-slice-12px.png",
    "ui/fixtures/visual-foundation/atlas-four-frames.png",
];
const GALLERY_IMAGE_FIT_SOURCE_PATH: &str = "ui/fixtures/visual-foundation/non-square-2x1.png";
const GALLERY_NINE_SLICE_SOURCE_PATH: &str = "ui/fixtures/visual-foundation/nine-slice-12px.png";
const GALLERY_TILE_SOURCE_PATH: &str = "ui/fixtures/visual-foundation/non-square-2x1.png";
const GALLERY_FRAME_SOURCE_PATH: &str = "ui/fixtures/visual-foundation/atlas-four-frames.png";
#[cfg(test)]
const GALLERY_VISUAL_FONT_FIXTURE_PATHS: [&str; 3] = [
    "ui/fixtures/fonts/FigtreeFixture-Regular.ttf",
    "ui/fixtures/fonts/FigtreeFixture-Medium.ttf",
    "ui/fixtures/fonts/FigtreeFixture-Bold.ttf",
];
const GALLERY_IMAGE_PATHS: [&str; 2] = [
    "ui/images/battlepass_bg_dragon01.png",
    "ui/images/battlepass_bg_dragon02.png",
];
const GALLERY_ATLAS_SOURCE_PATHS: [&str; 7] = [
    "ui/atlas/day_goal_tap.png",
    "ui/atlas/day_goal_tap2.png",
    "ui/atlas/puzzle_img1.png",
    "ui/atlas/puzzle_img_icon.png",
    "ui/atlas/puzzle_img_select.png",
    "ui/atlas/puzzle_img_select1.png",
    "ui/atlas/puzzle_img_time.png",
];
const GALLERY_BINDING_STATUS_PATH: &str = "gallery.binding.status";
const GALLERY_BINDING_NOTICE_VISIBLE_PATH: &str = "gallery.binding.notice_visible";
const GALLERY_BINDING_BUTTON_DISABLED_PATH: &str = "gallery.binding.button_disabled";
const GALLERY_TYPOGRAPHY_WEIGHTS: [(UiFontWeight, &str); 3] = [
    (UiFontWeight::Regular, "Regular 400 / Aa Bb 0123 !?,."),
    (UiFontWeight::Medium, "Medium 500 / Aa Bb 0123 !?,."),
    (UiFontWeight::Bold, "Bold 700 / Aa Bb 0123 !?,."),
];
const GALLERY_TYPOGRAPHY_MIXED_TEXT: &str = "MyBevy 中文混排 2026，标点：！？；ABC-123";
const GALLERY_TYPOGRAPHY_LONG_WORD: &str =
    "InteroperabilityWithoutWhitespaceMustStillWrapAtACharacterBoundary";
const GALLERY_TYPOGRAPHY_LONG_CJK: &str =
    "这是一段用于验证超长中文在紧凑容器中按字符安全换行且不会覆盖相邻内容的文字。";
const GALLERY_TYPOGRAPHY_CLIP_FRAME_WIDTH: f32 = 280.0;
const GALLERY_TYPOGRAPHY_CLIP_FRAME_HEIGHT: f32 = 32.0;
const GALLERY_TYPOGRAPHY_SECTION_LINE_HEIGHT: f32 = 1.25;
const GALLERY_TYPOGRAPHY_MIXED_LINE_HEIGHT: f32 = 1.25;
const GALLERY_TYPOGRAPHY_BODY_LINE_HEIGHT: f32 = 1.35;
const GALLERY_TYPOGRAPHY_OVERFLOW_CHILD_GAPS: f32 = 5.0;
// Covers the two border edges when border-box layout rounds fractional text heights.
const GALLERY_TYPOGRAPHY_BORDER_ROUNDING_ALLOWANCE: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GalleryTypographyLineBudget {
    mixed: usize,
    long_word: usize,
    long_cjk: usize,
    ellipsis: usize,
}

const GALLERY_TYPOGRAPHY_COMPACT_LINE_BUDGET: GalleryTypographyLineBudget =
    GalleryTypographyLineBudget {
        mixed: 2,
        long_word: 3,
        long_cjk: 4,
        ellipsis: 1,
    };
const GALLERY_TYPOGRAPHY_WIDE_LINE_BUDGET: GalleryTypographyLineBudget =
    GalleryTypographyLineBudget {
        mixed: 1,
        long_word: 2,
        long_cjk: 2,
        ellipsis: 1,
    };

#[derive(Clone, Copy, Component)]
pub(super) enum GalleryActionButton {
    Toast,
    ShowLoading,
    ShowCancellableLoading,
    HideLoading,
    Confirm,
    Floating,
    CloseTop,
    UpdateBinding,
}

#[derive(Resource)]
pub(super) struct GalleryLoadingPreview {
    timer: Timer,
}

#[derive(Resource)]
pub(super) struct GalleryBindingPreview {
    update_count: usize,
    notice_visible: bool,
    button_disabled: bool,
}

#[derive(Resource)]
pub(super) struct GalleryFloatingI18n {
    panel_id: UiPanelId,
    title: UiI18nTextSpec,
    body: UiI18nTextSpec,
    detail: Option<UiI18nTextSpec>,
}

enum GalleryTextInputState {
    Helper(String),
    Required(String),
    Validation(String),
    Alphanumeric {
        min_chars: usize,
        max_chars: usize,
        message: String,
    },
    Error,
    MaxChars(usize),
    Readonly,
    Disabled,
}

#[derive(Component)]
struct GalleryVisualFoundationRegion;

#[derive(Component)]
struct GalleryImageFitRegion;

#[derive(Component)]
struct GalleryImageModesRegion;

#[derive(Component)]
struct GalleryTypographyRegion;

#[derive(Component)]
struct GalleryTypographyOverflowRegion;

#[derive(Component)]
struct GalleryTypographyBoundedSamples;

#[derive(Component)]
struct GalleryIconsRegion;

#[derive(Component)]
struct GalleryIconStatesRegion;

#[derive(Component)]
struct GalleryStyleScopesRegion;

#[derive(Clone, Copy, Component)]
struct GalleryIconStatePreview(UiButtonVisualState);

#[derive(Clone, Copy)]
struct GalleryAtlasFrameSample {
    label: &'static str,
    x: u32,
    pivot: UiImagePivot,
}

const GALLERY_ATLAS_FRAME_SAMPLES: [GalleryAtlasFrameSample; 4] = [
    GalleryAtlasFrameSample {
        label: "Red circle",
        x: 0,
        pivot: UiImagePivot::new(0.5, 0.5),
    },
    GalleryAtlasFrameSample {
        label: "Green square",
        x: 32,
        pivot: UiImagePivot::new(0.5, 0.5),
    },
    GalleryAtlasFrameSample {
        label: "Blue diamond",
        x: 64,
        pivot: UiImagePivot::new(0.5, 0.5),
    },
    GalleryAtlasFrameSample {
        label: "Yellow cross",
        x: 96,
        pivot: UiImagePivot::new(0.5, 0.5),
    },
];

#[derive(Clone, Copy)]
struct GalleryImageFitSample {
    label: &'static str,
    landscape_fit: UiImageFit,
    portrait_fit: UiImageFit,
}

const GALLERY_IMAGE_FIT_SAMPLES: [GalleryImageFitSample; 4] = [
    GalleryImageFitSample {
        label: "Natural",
        landscape_fit: UiImageFit::Natural,
        portrait_fit: UiImageFit::Natural,
    },
    GalleryImageFitSample {
        label: "Stretch",
        landscape_fit: UiImageFit::Stretch,
        portrait_fit: UiImageFit::Stretch,
    },
    GalleryImageFitSample {
        label: "Contain",
        landscape_fit: UiImageFit::Contain,
        portrait_fit: UiImageFit::Contain,
    },
    GalleryImageFitSample {
        label: "Cover focus 0 / 1",
        landscape_fit: UiImageFit::cover(UiImageFocus::TOP_LEFT),
        portrait_fit: UiImageFit::cover(UiImageFocus::BOTTOM_RIGHT),
    },
];

impl GalleryLoadingPreview {
    fn new() -> Self {
        Self {
            timer: Timer::from_seconds(1.2, TimerMode::Once),
        }
    }
}

impl Default for GalleryBindingPreview {
    fn default() -> Self {
        Self {
            update_count: 0,
            notice_visible: true,
            button_disabled: false,
        }
    }
}

pub(super) fn setup_ui_gallery(
    mut commands: Commands,
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    viewport: Res<UiViewport>,
    fonts: Res<UiFontAssets>,
    i18n: Res<UiI18n>,
    asset_server: Res<AssetServer>,
    mut binding_values: ResMut<UiBindingValues>,
    mut clear_color: ResMut<ClearColor>,
) {
    let theme = theme.into_inner();
    let metrics = metrics.into_inner();
    let width_class = viewport.width_class;
    let fonts = fonts.into_inner();
    let i18n = i18n.into_inner();
    let asset_server = asset_server.into_inner();
    clear_color.0 = theme.colors.screen_background;
    commands.insert_resource(GalleryBindingPreview::default());
    binding_values.set_text(
        GALLERY_BINDING_STATUS_PATH,
        i18n.tr(
            "ui_gallery.binding.status.initial",
            "Waiting for binding update.",
        ),
    );
    binding_values.set_bool(GALLERY_BINDING_NOTICE_VISIBLE_PATH, true);
    binding_values.set_bool(GALLERY_BINDING_BUTTON_DISABLED_PATH, false);

    commands
        .spawn((
            DespawnOnExit(AppUiMode::UiGallery),
            game_panel_root(PANEL_UI_GALLERY, UiPanelKind::Page, OWNER_UI_GALLERY),
            UiLayerRoot {
                layer: UiLayer::Page,
            },
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                padding: viewport.safe_area_padding(metrics.page_padding),
                row_gap: px(theme.layout.page_gap),
                ..default()
            },
            BackgroundColor(theme.colors.screen_background),
            UiThemeBackgroundRole::Screen,
            UiThemeRootNodeRole::Screen,
        ))
        .with_children(|root| {
            root.spawn(gallery_header(theme, metrics, width_class))
                .with_children(|header| {
                    header.spawn(screen_title_key(
                        theme,
                        fonts,
                        i18n,
                        "ui_gallery.title",
                        "UI Gallery",
                        UiThemeTextStyleRole::Title,
                    ));
                    header.spawn(secondary_route_button_key(
                        theme,
                        metrics,
                        fonts,
                        i18n,
                        "nav.lobby",
                        "Lobby",
                        AppUiMode::Lobby,
                    ));
                });

            let mut scroll_body = root.spawn(ui_scroll_column(theme));
            scroll_body.insert(SCROLL_UI_GALLERY_MAIN);
            scroll_body.with_children(|body| {
                body.spawn((
                    gallery_panel(theme),
                    GalleryVisualFoundationRegion,
                    Name::new("Gallery visual foundation region"),
                ))
                .with_children(|visual_panel| {
                    visual_panel.spawn(section_label_key(
                        theme,
                        fonts,
                        i18n,
                        "ui_gallery.visual_foundation.section",
                        "Visual Foundation",
                    ));
                    visual_panel.spawn(screen_label_key(
                        theme,
                        fonts,
                        i18n,
                        "ui_gallery.visual_foundation.description",
                        "Alpha edge, 2:1 image, nine-slice, and atlas fixtures.",
                        UiThemeTextStyleRole::Body,
                        UiThemeTextColorRole::Muted,
                    ));
                    visual_panel.spawn(section_label_key(
                        theme,
                        fonts,
                        i18n,
                        "ui_gallery.image_fit.section",
                        "Image Fit",
                    ));
                    visual_panel.spawn(screen_label_key(
                        theme,
                        fonts,
                        i18n,
                        "ui_gallery.image_fit.description",
                        "Natural, Stretch, Contain, and focused Cover in landscape and portrait frames.",
                        UiThemeTextStyleRole::Body,
                        UiThemeTextColorRole::Muted,
                    ));
                    visual_panel
                        .spawn((
                            gallery_grid(
                                metrics,
                                width_class,
                                UiResponsiveGridColumns::new(2, 2, 4),
                            ),
                            GalleryImageFitRegion,
                            Name::new("Gallery image fit region"),
                        ))
                        .with_children(|samples| {
                            for sample in GALLERY_IMAGE_FIT_SAMPLES {
                                spawn_gallery_image_fit_sample(
                                    samples,
                                    theme,
                                    fonts,
                                    asset_server.load(GALLERY_IMAGE_FIT_SOURCE_PATH),
                                    sample,
                                );
                            }
                        });
                    visual_panel
                        .spawn(ui_thumbnail_grid(4, metrics.control_gap))
                        .with_children(|fixtures| {
                            for path in GALLERY_VISUAL_FIXTURE_PATHS {
                                spawn_gallery_visual_fixture(
                                    fixtures,
                                    theme,
                                    asset_server.load(path),
                                    path,
                                );
                            }
                        });
                });

                body.spawn((
                    gallery_panel(theme),
                    GalleryImageModesRegion,
                    ANCHOR_UI_GALLERY_IMAGE_MODES,
                    Name::new("Gallery advanced image modes region"),
                ))
                .with_children(|image_modes| {
                    image_modes.spawn(section_label_key(
                        theme,
                        fonts,
                        i18n,
                        "ui_gallery.image_modes.section",
                        "Nine-slice, Tiling, and Atlas Frames",
                    ));
                    image_modes.spawn(screen_label_key(
                        theme,
                        fonts,
                        i18n,
                        "ui_gallery.image_modes.description",
                        "Scalable borders, bounded texture repetition, and exact frame regions.",
                        UiThemeTextStyleRole::Body,
                        UiThemeTextColorRole::Muted,
                    ));
                    spawn_gallery_nine_slice_samples(
                        image_modes,
                        theme,
                        metrics,
                        fonts,
                        width_class,
                        asset_server,
                    );
                    spawn_gallery_tiling_samples(
                        image_modes,
                        theme,
                        metrics,
                        fonts,
                        width_class,
                        asset_server,
                    );
                    spawn_gallery_atlas_frame_samples(
                        image_modes,
                        theme,
                        metrics,
                        fonts,
                        width_class,
                        asset_server,
                    );
                });

                body.spawn((
                    gallery_panel(theme),
                    GalleryTypographyRegion,
                    ANCHOR_UI_GALLERY_TYPOGRAPHY,
                    Name::new("Gallery typography region"),
                ))
                .with_children(|typography_panel| {
                    spawn_gallery_typography(
                        typography_panel,
                        theme,
                        metrics,
                        fonts,
                        i18n,
                        width_class,
                    );
                });

                body.spawn(gallery_typography_overflow_panel(theme, width_class))
                    .with_children(|overflow_panel| {
                        spawn_gallery_typography_overflow(overflow_panel, theme, fonts, i18n);
                    });

                body.spawn((
                    gallery_typography_status_panel(theme),
                    GalleryTypographyBoundedSamples,
                    Name::new("Gallery fixed typography status panel"),
                ))
                .with_children(|status_panel| {
                    spawn_gallery_typography_status_samples(status_panel, theme, fonts, i18n);
                });

                body.spawn(gallery_panel(theme))
                    .with_children(|buttons_panel| {
                        buttons_panel.spawn(section_label_key(
                            theme,
                            fonts,
                            i18n,
                            "ui_gallery.buttons.section",
                            "Buttons",
                        ));
                        buttons_panel
                            .spawn(gallery_grid(metrics, width_class, gallery_button_columns()))
                            .with_children(|buttons| {
                                buttons.spawn(primary_action_button_key(
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    "ui_gallery.buttons.primary",
                                    "Primary",
                                ));
                                buttons.spawn(secondary_action_button_key(
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    "ui_gallery.buttons.secondary",
                                    "Secondary",
                                ));
                                buttons.spawn((
                                    primary_action_button_key(
                                        theme,
                                        metrics,
                                        fonts,
                                        i18n,
                                        "ui_gallery.buttons.focused",
                                        "Focused",
                                    ),
                                    FocusedButton,
                                ));
                                buttons.spawn((
                                    secondary_action_button_key(
                                        theme,
                                        metrics,
                                        fonts,
                                        i18n,
                                        "ui_gallery.buttons.selected",
                                        "Selected",
                                    ),
                                    SelectedButton,
                                ));
                                buttons.spawn(loading_primary_action_button_key(
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    "ui_gallery.buttons.loading",
                                    "Loading",
                                ));
                                buttons.spawn(disabled_primary_action_button_key(
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    "ui_gallery.buttons.disabled",
                                    "Disabled",
                                ));
                                buttons.spawn(disabled_secondary_action_button_key(
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    "ui_gallery.buttons.unavailable",
                                    "Unavailable",
                                ));
                                buttons.spawn(primary_route_button_sample(
                                    theme, metrics, fonts, i18n,
                                ));
                            });
                    });

                body.spawn(gallery_icons_panel(theme))
                .with_children(|icon_buttons_panel| {
                    spawn_gallery_icon_samples(
                        icon_buttons_panel,
                        theme,
                        metrics,
                        fonts,
                        i18n,
                        width_class,
                        asset_server,
                    );
                });

                body.spawn(gallery_icon_states_panel(theme))
                .with_children(|icon_states_panel| {
                    spawn_gallery_icon_state_samples(
                        icon_states_panel,
                        theme,
                        metrics,
                        fonts,
                        i18n,
                        width_class,
                        asset_server,
                    );
                });

                body.spawn(gallery_style_scopes_panel(theme))
                    .with_children(|style_panel| {
                        spawn_gallery_style_scope_samples(
                            style_panel,
                            theme,
                            metrics,
                            fonts,
                            i18n,
                            width_class,
                            asset_server,
                        );
                    });

                body.spawn(gallery_panel(theme))
                    .with_children(|selection_panel| {
                        selection_panel.spawn(section_label_key(
                            theme,
                            fonts,
                            i18n,
                            "ui_gallery.selection.section",
                            "Selection Controls",
                        ));
                        selection_panel
                            .spawn(gallery_grid(
                                metrics,
                                width_class,
                                gallery_selection_columns(),
                            ))
                            .with_children(|controls| {
                                controls.spawn(checkbox_key(
                                    theme,
                                    fonts,
                                    i18n,
                                    "ui_gallery.selection.checkbox.unchecked",
                                    "Unchecked",
                                ));
                                controls.spawn(checked_checkbox_key(
                                    theme,
                                    fonts,
                                    i18n,
                                    "ui_gallery.selection.checkbox.checked",
                                    "Checked",
                                ));
                                controls.spawn(disabled_checkbox_key(
                                    theme,
                                    fonts,
                                    i18n,
                                    "ui_gallery.selection.checkbox.disabled",
                                    "Disabled",
                                ));
                                controls.spawn(toggle_key(
                                    theme,
                                    fonts,
                                    i18n,
                                    "ui_gallery.selection.toggle.off",
                                    "Toggle Off",
                                ));
                                controls.spawn(toggle_on_key(
                                    theme,
                                    fonts,
                                    i18n,
                                    "ui_gallery.selection.toggle.on",
                                    "Toggle On",
                                ));
                                controls.spawn(disabled_toggle_key(
                                    theme,
                                    fonts,
                                    i18n,
                                    "ui_gallery.selection.toggle.disabled",
                                    "Toggle Disabled",
                                ));
                            });
                        selection_panel
                            .spawn(segmented_control(theme))
                            .with_children(|segments| {
                                segments.spawn(segment_option_key(
                                    theme,
                                    fonts,
                                    i18n,
                                    "small",
                                    "ui_gallery.selection.segment.small",
                                    "Small",
                                ));
                                segments.spawn(selected_segment_option_key(
                                    theme,
                                    fonts,
                                    i18n,
                                    "medium",
                                    "ui_gallery.selection.segment.medium",
                                    "Medium",
                                ));
                                segments.spawn(disabled_segment_option_key(
                                    theme,
                                    fonts,
                                    i18n,
                                    "large",
                                    "ui_gallery.selection.segment.large",
                                    "Large",
                                ));
                            });
                    });

                body.spawn(gallery_panel(theme))
                    .with_children(|numeric_panel| {
                        numeric_panel.spawn(section_label_key(
                            theme,
                            fonts,
                            i18n,
                            "ui_gallery.numeric.section",
                            "Numeric Controls",
                        ));
                        numeric_panel
                            .spawn(ui_responsive_column(
                                metrics,
                                UiJustify::Start,
                                UiAlign::Stretch,
                            ))
                            .with_children(|controls| {
                                controls.spawn(slider_key(
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    "ui_gallery.numeric.slider.volume",
                                    "Volume",
                                    64.0,
                                    0.0,
                                    100.0,
                                ));
                                controls.spawn(disabled_slider_key(
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    "ui_gallery.numeric.slider.disabled",
                                    "Disabled Slider",
                                    30.0,
                                    0.0,
                                    100.0,
                                ));
                                controls.spawn(stepper_key(
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    "ui_gallery.numeric.stepper.players",
                                    "Players",
                                    4,
                                    1,
                                    8,
                                    1,
                                ));
                                controls.spawn(disabled_stepper_key(
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    "ui_gallery.numeric.stepper.disabled",
                                    "Disabled Stepper",
                                    2,
                                    1,
                                    8,
                                    1,
                                ));
                            });
                    });

                body.spawn(gallery_panel(theme))
                    .with_children(|inputs_panel| {
                        inputs_panel.spawn(section_label_key(
                            theme,
                            fonts,
                            i18n,
                            "ui_gallery.inputs.section",
                            "Inputs",
                        ));
                        inputs_panel
                            .spawn(ui_column(theme.layout.row_gap))
                            .with_children(|inputs| {
                                spawn_gallery_text_input(
                                    inputs,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n.tr(
                                        "ui_gallery.inputs.placeholder.player_name",
                                        "Player name",
                                    ),
                                    "Pilot 01",
                                    [GalleryTextInputState::Helper(i18n.tr(
                                        "ui_gallery.inputs.helper.player_name",
                                        "Shown to other players.",
                                    ))],
                                );
                                spawn_gallery_text_input(
                                    inputs,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n.tr("ui_gallery.inputs.placeholder.required", "Required"),
                                    "",
                                    [
                                        GalleryTextInputState::Required(i18n.tr(
                                            "ui_gallery.inputs.validation.required",
                                            "This field is required.",
                                        )),
                                        GalleryTextInputState::Helper(i18n.tr(
                                            "ui_gallery.inputs.helper.required",
                                            "Required fields validate empty values.",
                                        )),
                                    ],
                                );
                                spawn_gallery_text_input(
                                    inputs,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n.tr("ui_gallery.inputs.placeholder.error", "Error state"),
                                    "bad-code",
                                    [GalleryTextInputState::Alphanumeric {
                                        min_chars: 4,
                                        max_chars: 8,
                                        message: i18n.tr(
                                            "ui_gallery.inputs.validation.error",
                                            "Use 4-8 letters or numbers.",
                                        ),
                                    }],
                                );
                                spawn_gallery_text_input(
                                    inputs,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n.tr("ui_gallery.inputs.placeholder.note", "Type a note"),
                                    "",
                                    [
                                        GalleryTextInputState::MaxChars(12),
                                        GalleryTextInputState::Helper(i18n.tr(
                                            "ui_gallery.inputs.helper.note",
                                            "Limited to 12 characters.",
                                        )),
                                    ],
                                );
                                spawn_gallery_text_input(
                                    inputs,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n.tr("ui_gallery.inputs.placeholder.readonly", "Read only"),
                                    "Readonly sample",
                                    [
                                        GalleryTextInputState::Readonly,
                                        GalleryTextInputState::Helper(i18n.tr(
                                            "ui_gallery.inputs.helper.readonly",
                                            "Readonly keeps focus but does not edit.",
                                        )),
                                    ],
                                );
                                spawn_gallery_text_input(
                                    inputs,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n.tr("ui_gallery.inputs.placeholder.disabled", "Disabled"),
                                    "Disabled sample",
                                    [
                                        GalleryTextInputState::Disabled,
                                        GalleryTextInputState::Error,
                                        GalleryTextInputState::Validation(i18n.tr(
                                            "ui_gallery.inputs.validation.disabled_error",
                                            "Disabled visual state wins over error.",
                                        )),
                                    ],
                                );
                                spawn_gallery_text_input(
                                    inputs,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n.tr(
                                        "ui_gallery.inputs.placeholder.short_code",
                                        "Max 6 chars",
                                    ),
                                    "ABC",
                                    [
                                        GalleryTextInputState::MaxChars(6),
                                        GalleryTextInputState::Required(i18n.tr(
                                            "ui_gallery.inputs.validation.required",
                                            "This field is required.",
                                        )),
                                        GalleryTextInputState::Helper(i18n.tr(
                                            "ui_gallery.inputs.helper.short_code",
                                            "Required, max 6 characters.",
                                        )),
                                    ],
                                );
                                spawn_gallery_text_input(
                                    inputs,
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n.tr("ui_gallery.inputs.placeholder.empty", "Empty input"),
                                    "",
                                    [GalleryTextInputState::Helper(i18n.tr(
                                        "ui_gallery.inputs.helper.empty",
                                        "Optional empty field.",
                                    ))],
                                );
                            });
                    });

                body.spawn(gallery_panel(theme))
                    .with_children(|binding_panel| {
                        binding_panel.spawn(section_label_key(
                            theme,
                            fonts,
                            i18n,
                            "ui_gallery.binding.section",
                            "Binding Sample",
                        ));
                        binding_panel.spawn(screen_label_key(
                            theme,
                            fonts,
                            i18n,
                            "ui_gallery.binding.description",
                            "The controls below are driven by UiBindingValues.",
                            UiThemeTextStyleRole::Body,
                            UiThemeTextColorRole::Muted,
                        ));
                        binding_panel
                            .spawn(ui_column(theme.layout.row_gap))
                            .with_children(|sample| {
                                sample.spawn((
                                    screen_label(
                                        theme,
                                        fonts,
                                        i18n.tr(
                                            "ui_gallery.binding.status.initial",
                                            "Waiting for binding update.",
                                        ),
                                        UiThemeTextStyleRole::Body,
                                        UiThemeTextColorRole::Primary,
                                    ),
                                    UiBoundText::with_fallback(
                                        GALLERY_BINDING_STATUS_PATH,
                                        i18n.tr(
                                            "ui_gallery.binding.status.initial",
                                            "Waiting for binding update.",
                                        ),
                                    )
                                    .expect("gallery binding path should be valid"),
                                ));
                                sample.spawn((
                                    screen_label_key(
                                        theme,
                                        fonts,
                                        i18n,
                                        "ui_gallery.binding.notice",
                                        "This prompt is controlled by a bool visibility binding.",
                                        UiThemeTextStyleRole::Body,
                                        UiThemeTextColorRole::Muted,
                                    ),
                                    Visibility::Visible,
                                    UiBoundVisibility::new(GALLERY_BINDING_NOTICE_VISIBLE_PATH)
                                        .expect("gallery binding path should be valid"),
                                ));
                                sample.spawn((
                                    secondary_action_button_key(
                                        theme,
                                        metrics,
                                        fonts,
                                        i18n,
                                        "ui_gallery.binding.bound_button",
                                        "Bound Button",
                                    ),
                                    UiBoundDisabled::new(GALLERY_BINDING_BUTTON_DISABLED_PATH)
                                        .expect("gallery binding path should be valid"),
                                ));
                                sample.spawn((
                                    secondary_action_button_key(
                                        theme,
                                        metrics,
                                        fonts,
                                        i18n,
                                        "ui_gallery.binding.action",
                                        "Update Binding",
                                    ),
                                    GalleryActionButton::UpdateBinding,
                                ));
                            });
                    });

                body.spawn(gallery_panel(theme))
                    .with_children(|overlays_panel| {
                        overlays_panel.spawn(section_label_key(
                            theme,
                            fonts,
                            i18n,
                            "ui_gallery.overlays.section",
                            "Overlays",
                        ));
                        overlays_panel
                            .spawn(gallery_grid(
                                metrics,
                                width_class,
                                gallery_overlay_columns(),
                            ))
                            .with_children(|buttons| {
                                buttons.spawn((
                                    primary_action_button_key(
                                        theme,
                                        metrics,
                                        fonts,
                                        i18n,
                                        "ui_gallery.overlays.show_toast",
                                        "Show Toast",
                                    ),
                                    GalleryActionButton::Toast,
                                ));
                                buttons.spawn((
                                    secondary_action_button_key(
                                        theme,
                                        metrics,
                                        fonts,
                                        i18n,
                                        "ui_gallery.overlays.loading",
                                        "Loading",
                                    ),
                                    GalleryActionButton::ShowLoading,
                                ));
                                buttons.spawn((
                                    secondary_action_button_key(
                                        theme,
                                        metrics,
                                        fonts,
                                        i18n,
                                        "ui_gallery.overlays.cancelable",
                                        "Cancelable",
                                    ),
                                    GalleryActionButton::ShowCancellableLoading,
                                ));
                                buttons.spawn((
                                    secondary_action_button_key(
                                        theme,
                                        metrics,
                                        fonts,
                                        i18n,
                                        "ui_gallery.overlays.hide",
                                        "Hide",
                                    ),
                                    GalleryActionButton::HideLoading,
                                ));
                                buttons.spawn((
                                    primary_action_button_key(
                                        theme,
                                        metrics,
                                        fonts,
                                        i18n,
                                        "ui_gallery.overlays.show_confirm",
                                        "Show Confirm",
                                    ),
                                    GalleryActionButton::Confirm,
                                ));
                                buttons.spawn((
                                    secondary_action_button_key(
                                        theme,
                                        metrics,
                                        fonts,
                                        i18n,
                                        "ui_gallery.overlays.show_floating",
                                        "Show Floating",
                                    ),
                                    GalleryActionButton::Floating,
                                ));
                                buttons.spawn((
                                    secondary_action_button_key(
                                        theme,
                                        metrics,
                                        fonts,
                                        i18n,
                                        "ui_gallery.overlays.close_top",
                                        "Close Top",
                                    ),
                                    GalleryActionButton::CloseTop,
                                ));
                            });
                    });

                body.spawn(gallery_panel(theme))
                    .with_children(|images_panel| {
                        images_panel.spawn(section_label_key(
                            theme,
                            fonts,
                            i18n,
                            "ui_gallery.images.section",
                            "Images",
                        ));
                        images_panel.spawn(screen_label_key(
                            theme,
                            fonts,
                            i18n,
                            "ui_gallery.images.description",
                            "Regular packaged UI images loaded from assets/ui/images.",
                            UiThemeTextStyleRole::Body,
                            UiThemeTextColorRole::Muted,
                        ));
                        images_panel
                            .spawn(gallery_grid(metrics, width_class, gallery_image_columns()))
                            .with_children(|images| {
                                for path in GALLERY_IMAGE_PATHS {
                                    spawn_gallery_image_card(
                                        images,
                                        theme,
                                        fonts,
                                        asset_server.load(path),
                                        path,
                                    );
                                }
                            });

                        images_panel.spawn(section_label_key(
                            theme,
                            fonts,
                            i18n,
                            "ui_gallery.images.atlas_sources",
                            "Atlas Source Images",
                        ));
                        images_panel.spawn(screen_label_key(
                            theme,
                            fonts,
                            i18n,
                            "ui_gallery.images.atlas_sources.description",
                            "Source PNGs only; this is not a formal atlas frame preview.",
                            UiThemeTextStyleRole::Body,
                            UiThemeTextColorRole::Muted,
                        ));
                        images_panel
                            .spawn(ui_thumbnail_grid(
                                gallery_atlas_source_columns().for_width_class(width_class),
                                metrics.control_gap,
                            ))
                            .with_children(|atlas_sources| {
                                for path in GALLERY_ATLAS_SOURCE_PATHS {
                                    spawn_gallery_atlas_source_thumbnail(
                                        atlas_sources,
                                        theme,
                                        fonts,
                                        asset_server.load(path),
                                        path,
                                    );
                                }
                            });
                    });

                body.spawn(gallery_panel(theme))
                    .with_children(|stress_panel| {
                        stress_panel.spawn(section_label_key(
                            theme,
                            fonts,
                            i18n,
                            "ui_gallery.stress.section",
                            "Stress Sample",
                        ));
                        stress_panel.spawn(screen_label_key(
                            theme,
                            fonts,
                            i18n,
                            "ui_gallery.stress.description",
                            "Static list for observing node and text counts in F3.",
                            UiThemeTextStyleRole::Body,
                            UiThemeTextColorRole::Muted,
                        ));
                        stress_panel
                            .spawn(gallery_grid(metrics, width_class, gallery_stress_columns()))
                            .with_children(|items| {
                                for index in 0..GALLERY_STRESS_ITEM_COUNT {
                                    spawn_gallery_stress_item(
                                        items, theme, metrics, fonts, i18n, index,
                                    );
                                }
                            });
                    });
            });
        });
}

pub(super) fn handle_ui_gallery_buttons(
    mut commands: Commands,
    i18n: Res<UiI18n>,
    mut binding_values: ResMut<UiBindingValues>,
    mut binding_preview: ResMut<GalleryBindingPreview>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
    mut overlay_commands: MessageWriter<UiOverlayCommand>,
    action_buttons: Query<&GalleryActionButton>,
    mut button_events: MessageReader<UiButtonEvent>,
) {
    for event in button_events.read() {
        if event.kind != UiButtonEventKind::Click {
            continue;
        }

        let Ok(action) = action_buttons.get(event.entity) else {
            continue;
        };

        match action {
            GalleryActionButton::Toast => {
                overlay_commands.write(UiOverlayCommand::ShowToast(UiToast::new_key(
                    &i18n,
                    "ui_gallery.toast.preview",
                    "Toast from UI Gallery",
                )));
            }
            GalleryActionButton::ShowLoading => {
                commands.insert_resource(GalleryLoadingPreview::new());
                panel_commands.write(UiPanelCommand::Open(UiPanelRequest::Loading(
                    UiLoading::new_key(&i18n, "ui_gallery.loading.preview", "Loading preview"),
                )));
            }
            GalleryActionButton::ShowCancellableLoading => {
                commands.insert_resource(GalleryLoadingPreview::new());
                panel_commands.write(UiPanelCommand::Open(UiPanelRequest::Loading(
                    UiLoading::new_key(
                        &i18n,
                        "ui_gallery.loading.cancelable",
                        "Cancelable loading",
                    )
                    .cancellable(),
                )));
            }
            GalleryActionButton::HideLoading => {
                commands.remove_resource::<GalleryLoadingPreview>();
                panel_commands.write(UiPanelCommand::Close(UI_PANEL_GLOBAL_LOADING));
            }
            GalleryActionButton::Confirm => {
                panel_commands.write(UiPanelCommand::Open(UiPanelRequest::Confirm(
                    gallery_confirm_modal(&i18n),
                )));
            }
            GalleryActionButton::Floating => {
                commands.insert_resource(gallery_floating_i18n(&i18n));
                panel_commands.write(UiPanelCommand::Open(UiPanelRequest::Floating(
                    gallery_floating_panel(&i18n),
                )));
            }
            GalleryActionButton::CloseTop => {
                panel_commands.write(UiPanelCommand::CloseTop);
            }
            GalleryActionButton::UpdateBinding => {
                binding_preview.update_count += 1;
                binding_preview.notice_visible = !binding_preview.notice_visible;
                binding_preview.button_disabled = !binding_preview.button_disabled;
                binding_values.set_text(
                    GALLERY_BINDING_STATUS_PATH,
                    format!(
                        "{} {}",
                        i18n.tr("ui_gallery.binding.status.updated", "Bound text updated"),
                        binding_preview.update_count
                    ),
                );
                binding_values.set_bool(
                    GALLERY_BINDING_NOTICE_VISIBLE_PATH,
                    binding_preview.notice_visible,
                );
                binding_values.set_bool(
                    GALLERY_BINDING_BUTTON_DISABLED_PATH,
                    binding_preview.button_disabled,
                );
            }
        }
    }
}

pub(super) fn log_ui_gallery_text_input_submissions(
    mut submissions: MessageReader<UiTextInputSubmitted>,
) {
    for submission in submissions.read() {
        info!(
            entity = ?submission.entity,
            value = %submission.value,
            "ui gallery text input submitted"
        );
    }
}

pub(super) fn tick_ui_gallery_loading_preview(
    mut commands: Commands,
    time: Res<Time>,
    preview: Option<ResMut<GalleryLoadingPreview>>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
) {
    let Some(mut preview) = preview else {
        return;
    };

    preview.timer.tick(time.delta());
    if preview.timer.is_finished() {
        commands.remove_resource::<GalleryLoadingPreview>();
        panel_commands.write(UiPanelCommand::Close(UI_PANEL_GLOBAL_LOADING));
    }
}

pub(super) fn clear_ui_gallery_loading_preview(mut commands: Commands) {
    commands.remove_resource::<GalleryLoadingPreview>();
    commands.remove_resource::<GalleryBindingPreview>();
    commands.remove_resource::<GalleryFloatingI18n>();
}

pub(super) fn apply_gallery_icon_state_previews(world: &mut World) {
    let previews = {
        let mut query = world.query::<(Entity, &GalleryIconStatePreview)>();
        query
            .iter(world)
            .map(|(entity, preview)| (entity, preview.0))
            .collect::<Vec<_>>()
    };

    for (entity, state) in previews {
        let mut entity = world.entity_mut(entity);
        let desired_interaction = match state {
            UiButtonVisualState::Hovered => Interaction::Hovered,
            UiButtonVisualState::Pressed => Interaction::Pressed,
            _ => Interaction::None,
        };
        if entity.get::<Interaction>() != Some(&desired_interaction) {
            entity.insert(desired_interaction);
        }

        let focused = state == UiButtonVisualState::Focused;
        if focused != entity.contains::<FocusedButton>() {
            if focused {
                entity.insert(FocusedButton);
            } else {
                entity.remove::<FocusedButton>();
            }
        }
        let selected = state == UiButtonVisualState::Selected;
        if selected != entity.contains::<SelectedButton>() {
            if selected {
                entity.insert(SelectedButton);
            } else {
                entity.remove::<SelectedButton>();
            }
        }
        let disabled = state == UiButtonVisualState::Disabled;
        if disabled != entity.contains::<crate::framework::ui::widgets::DisabledButton>() {
            if disabled {
                entity.insert(crate::framework::ui::widgets::DisabledButton);
            } else {
                entity.remove::<crate::framework::ui::widgets::DisabledButton>();
            }
        }
        let loading = state == UiButtonVisualState::Loading;
        if loading != entity.contains::<crate::framework::ui::widgets::LoadingButton>() {
            if loading {
                entity.insert(crate::framework::ui::widgets::LoadingButton);
            } else {
                entity.remove::<crate::framework::ui::widgets::LoadingButton>();
            }
        }
    }
}

pub(super) fn tag_gallery_floating_i18n_texts(
    mut commands: Commands,
    floating_i18n: Option<Res<GalleryFloatingI18n>>,
    panel_roots: Query<(Entity, &UiPanelRoot)>,
    children: Query<&Children>,
    texts: Query<(Entity, &Text), Without<UiI18nText>>,
) {
    let Some(floating_i18n) = floating_i18n else {
        return;
    };

    let Some(panel_root_entity) = panel_roots
        .iter()
        .find_map(|(entity, panel)| (panel.id == floating_i18n.panel_id).then_some(entity))
    else {
        return;
    };

    for entity in children.iter_descendants(panel_root_entity) {
        let Ok((text_entity, text)) = texts.get(entity) else {
            continue;
        };

        let marker = if text.0 == floating_i18n.title.text {
            Some(floating_i18n.title.i18n_text.clone())
        } else if text.0 == floating_i18n.body.text {
            Some(floating_i18n.body.i18n_text.clone())
        } else {
            floating_i18n
                .detail
                .as_ref()
                .filter(|detail| text.0 == detail.text)
                .map(|detail| detail.i18n_text.clone())
        };

        if let Some(marker) = marker {
            commands.entity(text_entity).insert(marker);
        }
    }
}

fn gallery_header(theme: &UiTheme, metrics: &UiMetrics, width_class: UiWidthClass) -> impl Bundle {
    Node {
        width: percent(100),
        max_width: px(theme.layout.content_width.min(metrics.content_max_width)),
        align_self: AlignSelf::Center,
        align_items: if width_class == UiWidthClass::Compact {
            AlignItems::Stretch
        } else {
            UiAlign::Center.to_align_items()
        },
        justify_content: if width_class == UiWidthClass::Compact {
            JustifyContent::FlexStart
        } else {
            UiJustify::SpaceBetween.to_justify_content()
        },
        column_gap: px(metrics.control_gap),
        row_gap: px(metrics.control_gap),
        flex_wrap: FlexWrap::Wrap,
        ..default()
    }
}

fn gallery_panel(theme: &UiTheme) -> impl Bundle {
    (
        UiThemePanelNodeRole::Content,
        gallery_panel_node(theme),
        BackgroundColor(theme.colors.panel_background),
        BorderColor::all(theme.colors.panel_border),
        UiThemeBackgroundRole::Panel,
        UiThemeBorderRole::Panel,
    )
}

fn gallery_panel_node(theme: &UiTheme) -> Node {
    Node {
        width: percent(100),
        max_width: px(theme.layout.content_width),
        align_self: AlignSelf::Center,
        flex_direction: FlexDirection::Column,
        row_gap: px(theme.layout.card_gap),
        padding: UiRect::all(px(theme.layout.panel_gap)),
        border: UiRect::all(px(theme.panel.border)),
        border_radius: BorderRadius::all(px(theme.panel.radius)),
        ..default()
    }
}

fn gallery_icons_panel(theme: &UiTheme) -> impl Bundle {
    (
        gallery_panel(theme),
        GalleryIconsRegion,
        ANCHOR_UI_GALLERY_ICONS,
        Name::new("Gallery icon and image button region"),
    )
}

fn gallery_icon_states_panel(theme: &UiTheme) -> impl Bundle {
    (
        gallery_panel(theme),
        GalleryIconStatesRegion,
        ANCHOR_UI_GALLERY_ICON_STATES,
        Name::new("Gallery icon button state matrix"),
    )
}

fn gallery_style_scopes_panel(theme: &UiTheme) -> impl Bundle {
    (
        gallery_panel(theme),
        GalleryStyleScopesRegion,
        ANCHOR_UI_GALLERY_STYLE_SCOPES,
        Name::new("Gallery scoped style comparison"),
    )
}

fn gallery_typography_overflow_panel(theme: &UiTheme, width_class: UiWidthClass) -> impl Bundle {
    (
        UiThemePanelNodeRole::Content,
        gallery_typography_overflow_panel_node(theme, width_class),
        BackgroundColor(theme.colors.panel_background),
        BorderColor::all(theme.colors.panel_border),
        UiThemeBackgroundRole::Panel,
        UiThemeBorderRole::Panel,
        GalleryTypographyOverflowRegion,
        ANCHOR_UI_GALLERY_TYPOGRAPHY_OVERFLOW,
        Name::new("Gallery typography overflow panel"),
    )
}

fn gallery_typography_overflow_panel_node(theme: &UiTheme, width_class: UiWidthClass) -> Node {
    let mut node = gallery_panel_node(theme);
    node.height = px(gallery_typography_overflow_panel_height(theme, width_class));
    node.flex_shrink = 0.0;
    node.overflow = Overflow::clip();
    node
}

fn gallery_typography_overflow_panel_height(theme: &UiTheme, width_class: UiWidthClass) -> f32 {
    let budget = gallery_typography_overflow_line_budget(width_class);
    let wrapped_lines = budget.long_word + budget.long_cjk + budget.ellipsis;

    theme.layout.panel_gap * 2.0
        + theme.text.section_label * GALLERY_TYPOGRAPHY_SECTION_LINE_HEIGHT
        + budget.mixed as f32 * theme.text.body * GALLERY_TYPOGRAPHY_MIXED_LINE_HEIGHT
        + wrapped_lines as f32 * theme.text.body * GALLERY_TYPOGRAPHY_BODY_LINE_HEIGHT
        + GALLERY_TYPOGRAPHY_CLIP_FRAME_HEIGHT
        + GALLERY_TYPOGRAPHY_OVERFLOW_CHILD_GAPS * theme.layout.card_gap
        + GALLERY_TYPOGRAPHY_BORDER_ROUNDING_ALLOWANCE
}

fn gallery_typography_overflow_line_budget(
    width_class: UiWidthClass,
) -> GalleryTypographyLineBudget {
    match width_class {
        UiWidthClass::Compact => GALLERY_TYPOGRAPHY_COMPACT_LINE_BUDGET,
        UiWidthClass::Medium | UiWidthClass::Expanded => GALLERY_TYPOGRAPHY_WIDE_LINE_BUDGET,
    }
}

fn gallery_typography_clip_frame() -> impl Bundle {
    try_ui_text_clip_frame(
        GALLERY_TYPOGRAPHY_CLIP_FRAME_WIDTH,
        GALLERY_TYPOGRAPHY_CLIP_FRAME_HEIGHT,
    )
    .expect("Gallery clip frame bounds must be valid")
}

fn gallery_grid(
    metrics: &UiMetrics,
    width_class: UiWidthClass,
    columns: UiResponsiveGridColumns,
) -> impl Bundle {
    ui_responsive_grid(metrics, width_class, columns)
}

fn gallery_button_columns() -> UiResponsiveGridColumns {
    UiResponsiveGridColumns::new(1, 2, 4)
}

fn gallery_icon_button_columns() -> UiResponsiveGridColumns {
    UiResponsiveGridColumns::new(3, 4, 5)
}

fn gallery_icon_state_columns() -> UiResponsiveGridColumns {
    UiResponsiveGridColumns::new(3, 5, 7)
}

fn gallery_selection_columns() -> UiResponsiveGridColumns {
    UiResponsiveGridColumns::new(1, 2, 3)
}

fn gallery_overlay_columns() -> UiResponsiveGridColumns {
    UiResponsiveGridColumns::new(1, 3, 5)
}

fn gallery_image_columns() -> UiResponsiveGridColumns {
    UiResponsiveGridColumns::new(1, 2, 2)
}

fn gallery_atlas_source_columns() -> UiResponsiveGridColumns {
    UiResponsiveGridColumns::new(2, 4, 6)
}

fn gallery_stress_columns() -> UiResponsiveGridColumns {
    UiResponsiveGridColumns::new(1, 2, 3)
}

#[allow(clippy::too_many_arguments)]
fn spawn_gallery_icon_samples(
    panel: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    width_class: UiWidthClass,
    asset_server: &AssetServer,
) {
    panel.spawn(section_label_key(
        theme,
        fonts,
        i18n,
        "ui_gallery.icon_buttons.section",
        "Icon and Image Buttons",
    ));
    panel.spawn(screen_label_key(
        theme,
        fonts,
        i18n,
        "ui_gallery.icon_buttons.description",
        "Asset icons, labeled placement, tint policy, and a visible missing placeholder.",
        UiThemeTextStyleRole::Body,
        UiThemeTextColorRole::Muted,
    ));
    panel.spawn(screen_label_key(
        theme,
        fonts,
        i18n,
        "ui_gallery.icon_buttons.icon_only",
        "Icon only",
        UiThemeTextStyleRole::Caption,
        UiThemeTextColorRole::Muted,
    ));
    panel
        .spawn(gallery_grid(
            metrics,
            width_class,
            gallery_icon_button_columns(),
        ))
        .with_children(|buttons| {
            for (icon, key, fallback) in [
                (UiIconId::ADD, "ui_gallery.icon_buttons.add", "Add"),
                (UiIconId::REMOVE, "ui_gallery.icon_buttons.remove", "Remove"),
                (UiIconId::HELP, "ui_gallery.icon_buttons.help", "Help"),
                (UiIconId::CLOSE, "ui_gallery.icon_buttons.close", "Close"),
                (
                    UiIconId::LOADING,
                    "ui_gallery.icon_buttons.loading",
                    "Loading",
                ),
            ] {
                buttons.spawn(icon_button_key(
                    theme,
                    metrics,
                    fonts,
                    asset_server,
                    i18n,
                    icon,
                    key,
                    fallback,
                ));
            }
        });

    panel.spawn(screen_label_key(
        theme,
        fonts,
        i18n,
        "ui_gallery.icon_buttons.labeled",
        "Icon and label",
        UiThemeTextStyleRole::Caption,
        UiThemeTextColorRole::Muted,
    ));
    panel
        .spawn(gallery_grid(
            metrics,
            width_class,
            UiResponsiveGridColumns::new(1, 2, 2),
        ))
        .with_children(|buttons| {
            buttons.spawn(icon_label_button_key(
                theme,
                metrics,
                fonts,
                asset_server,
                i18n,
                UiIconId::ARROW_LEFT,
                UiIconLabelPlacement::Leading,
                "ui_gallery.icon_buttons.previous",
                "Previous",
            ));
            buttons.spawn(icon_label_button_key(
                theme,
                metrics,
                fonts,
                asset_server,
                i18n,
                UiIconId::ARROW_RIGHT,
                UiIconLabelPlacement::Trailing,
                "ui_gallery.icon_buttons.next",
                "Next",
            ));
        });

    panel
        .spawn(gallery_grid(
            metrics,
            width_class,
            UiResponsiveGridColumns::new(3, 3, 3),
        ))
        .with_children(|samples| {
            spawn_gallery_icon_sample(
                samples,
                theme,
                fonts,
                i18n,
                icon_button_key(
                    theme,
                    metrics,
                    fonts,
                    asset_server,
                    i18n,
                    UiIconId::ADD,
                    "ui_gallery.icon_buttons.tintable",
                    "Tintable",
                ),
                "ui_gallery.icon_buttons.tintable",
                "Tintable",
            );
            spawn_gallery_icon_sample(
                samples,
                theme,
                fonts,
                i18n,
                image_button_key(
                    theme,
                    metrics,
                    fonts,
                    asset_server,
                    i18n,
                    UiIconId::FULL_COLOR_BADGE,
                    72.0,
                    56.0,
                    40.0,
                    "ui_gallery.icon_buttons.full_color",
                    "Full color",
                ),
                "ui_gallery.icon_buttons.full_color",
                "Full color",
            );
            spawn_gallery_icon_sample(
                samples,
                theme,
                fonts,
                i18n,
                icon_button_key(
                    theme,
                    metrics,
                    fonts,
                    asset_server,
                    i18n,
                    UiIconId::new("gallery_missing_icon"),
                    "ui_gallery.icon_buttons.missing",
                    "Missing",
                ),
                "ui_gallery.icon_buttons.missing",
                "Missing",
            );
        });
}

fn spawn_gallery_icon_sample(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    button: impl Bundle,
    label_key: &'static str,
    label_fallback: &'static str,
) {
    parent
        .spawn(gallery_icon_sample_node(theme))
        .with_children(|sample| {
            sample.spawn(button);
            sample.spawn(screen_label_key(
                theme,
                fonts,
                i18n,
                label_key,
                label_fallback,
                UiThemeTextStyleRole::Caption,
                UiThemeTextColorRole::Muted,
            ));
        });
}

fn gallery_icon_sample_node(theme: &UiTheme) -> Node {
    Node {
        min_width: px(76),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: px(theme.layout.row_gap),
        ..default()
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_gallery_icon_state_samples(
    panel: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    width_class: UiWidthClass,
    asset_server: &AssetServer,
) {
    panel.spawn(section_label_key(
        theme,
        fonts,
        i18n,
        "ui_gallery.icon_states.section",
        "Icon Button States",
    ));
    panel.spawn(screen_label_key(
        theme,
        fonts,
        i18n,
        "ui_gallery.icon_states.description",
        "Pointer, focus, selection, disabled, and loading use one state priority.",
        UiThemeTextStyleRole::Body,
        UiThemeTextColorRole::Muted,
    ));
    panel
        .spawn(gallery_grid(
            metrics,
            width_class,
            gallery_icon_state_columns(),
        ))
        .with_children(|states| {
            let base = || {
                icon_button_key(
                    theme,
                    metrics,
                    fonts,
                    asset_server,
                    i18n,
                    UiIconId::HELP,
                    "ui_gallery.icon_buttons.help",
                    "Help",
                )
            };
            spawn_gallery_icon_state_sample(
                states,
                theme,
                fonts,
                i18n,
                (base(), GalleryIconStatePreview(UiButtonVisualState::Idle)),
                "ui_gallery.icon_states.idle",
                "Idle",
            );
            spawn_gallery_icon_state_sample(
                states,
                theme,
                fonts,
                i18n,
                (
                    base(),
                    GalleryIconStatePreview(UiButtonVisualState::Hovered),
                ),
                "ui_gallery.icon_states.hovered",
                "Hovered",
            );
            spawn_gallery_icon_state_sample(
                states,
                theme,
                fonts,
                i18n,
                (
                    base(),
                    GalleryIconStatePreview(UiButtonVisualState::Pressed),
                ),
                "ui_gallery.icon_states.pressed",
                "Pressed",
            );
            spawn_gallery_icon_state_sample(
                states,
                theme,
                fonts,
                i18n,
                (
                    base(),
                    GalleryIconStatePreview(UiButtonVisualState::Focused),
                ),
                "ui_gallery.icon_states.focused",
                "Focused",
            );
            spawn_gallery_icon_state_sample(
                states,
                theme,
                fonts,
                i18n,
                (
                    base(),
                    GalleryIconStatePreview(UiButtonVisualState::Selected),
                ),
                "ui_gallery.icon_states.selected",
                "Selected",
            );
            spawn_gallery_icon_state_sample(
                states,
                theme,
                fonts,
                i18n,
                (
                    disabled_icon_button_key(
                        theme,
                        metrics,
                        fonts,
                        asset_server,
                        i18n,
                        UiIconId::HELP,
                        "ui_gallery.icon_states.disabled",
                        "Disabled",
                    ),
                    GalleryIconStatePreview(UiButtonVisualState::Disabled),
                ),
                "ui_gallery.icon_states.disabled",
                "Disabled",
            );
            spawn_gallery_icon_state_sample(
                states,
                theme,
                fonts,
                i18n,
                (
                    loading_icon_button_key(
                        theme,
                        metrics,
                        fonts,
                        asset_server,
                        i18n,
                        UiIconId::HELP,
                        "ui_gallery.icon_states.loading",
                        "Loading",
                    ),
                    GalleryIconStatePreview(UiButtonVisualState::Loading),
                ),
                "ui_gallery.icon_states.loading",
                "Loading",
            );
        });
}

fn spawn_gallery_icon_state_sample(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    button: impl Bundle,
    label_key: &'static str,
    label_fallback: &'static str,
) {
    spawn_gallery_icon_sample(
        parent,
        theme,
        fonts,
        i18n,
        button,
        label_key,
        label_fallback,
    );
}

#[allow(clippy::too_many_arguments)]
fn spawn_gallery_style_scope_samples(
    panel: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    width_class: UiWidthClass,
    asset_server: &AssetServer,
) {
    panel.spawn(section_label_key(
        theme,
        fonts,
        i18n,
        "ui_gallery.style_scopes.section",
        "Scoped Styles",
    ));
    panel.spawn(screen_label_key(
        theme,
        fonts,
        i18n,
        "ui_gallery.style_scopes.description",
        "Global, inherited, nested, and restored style resolution.",
        UiThemeTextStyleRole::Body,
        UiThemeTextColorRole::Muted,
    ));
    panel
        .spawn(gallery_grid(
            metrics,
            width_class,
            UiResponsiveGridColumns::new(1, 2, 4),
        ))
        .with_children(|samples| {
            spawn_gallery_style_tile(
                samples,
                theme,
                fonts,
                i18n,
                "ui_gallery.style_scopes.global",
                "Global default",
                None,
            );
            spawn_gallery_style_tile(
                samples,
                theme,
                fonts,
                i18n,
                "ui_gallery.style_scopes.parent",
                "Parent scope",
                Some(UI_STYLE_VARIANT_GALLERY_PARENT),
            );
            samples
                .spawn((
                    Node {
                        width: percent(100),
                        ..default()
                    },
                    UiStyleScope::new(UI_STYLE_VARIANT_GALLERY_PARENT),
                    Name::new("Gallery parent scope host"),
                ))
                .with_children(|parent_scope| {
                    spawn_gallery_style_tile(
                        parent_scope,
                        theme,
                        fonts,
                        i18n,
                        "ui_gallery.style_scopes.nested",
                        "Nested scope",
                        Some(UI_STYLE_VARIANT_GALLERY_NESTED),
                    );
                });
            spawn_gallery_style_tile(
                samples,
                theme,
                fonts,
                i18n,
                "ui_gallery.style_scopes.restored",
                "Outside scope / restored",
                None,
            );
        });

    panel
        .spawn((
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                column_gap: px(metrics.control_gap),
                row_gap: px(metrics.control_gap),
                flex_wrap: FlexWrap::Wrap,
                ..default()
            },
            UiStyleScope::new(UI_STYLE_VARIANT_GALLERY_PARENT),
            Name::new("Gallery scoped selected button host"),
        ))
        .with_children(|buttons| {
            buttons.spawn((
                secondary_action_button_key(
                    theme,
                    metrics,
                    fonts,
                    i18n,
                    "ui_gallery.style_scopes.selected_button",
                    "Selected persists",
                ),
                SelectedButton,
                UiStyleBinding::new().with_button(UiButtonStyleRole::Secondary),
                Name::new("Gallery scoped selected text button"),
            ));
            buttons.spawn((
                icon_button_key(
                    theme,
                    metrics,
                    fonts,
                    asset_server,
                    i18n,
                    UiIconId::HELP,
                    "ui_gallery.style_scopes.selected_icon",
                    "Selected scoped icon",
                ),
                SelectedButton,
                UiStyleBinding::new().with_button(UiButtonStyleRole::Secondary),
                Name::new("Gallery scoped selected icon button"),
            ));
        });
}

fn spawn_gallery_style_tile(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    label_key: &'static str,
    label_fallback: &'static str,
    scope: Option<&'static str>,
) {
    let mut tile = parent.spawn((
        Node {
            width: percent(100),
            min_height: px(76),
            justify_content: JustifyContent::Center,
            padding: UiRect::all(px(12)),
            border: UiRect::all(px(theme.panel.border)),
            border_radius: BorderRadius::all(px(theme.panel.radius)),
            ..default()
        },
        BackgroundColor(theme.colors.panel_background),
        BorderColor::all(theme.colors.panel_border),
        UiStyleBinding::new()
            .with_surface(UiSurfaceStyleRole::Panel)
            .with_border(UiBorderStyleRole::Panel),
        Name::new(label_fallback),
    ));
    if let Some(scope) = scope {
        tile.insert(UiStyleScope::new(scope));
    }
    tile.with_children(|content| {
        content.spawn((
            screen_label_key(
                theme,
                fonts,
                i18n,
                label_key,
                label_fallback,
                UiThemeTextStyleRole::Caption,
                UiThemeTextColorRole::Primary,
            ),
            UiStyleBinding::new().with_text(UiTextStyleRole::Caption),
        ));
    });
}

fn spawn_gallery_typography(
    panel: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    width_class: UiWidthClass,
) {
    panel.spawn(section_label_key(
        theme,
        fonts,
        i18n,
        "ui_gallery.typography.section",
        "Typography",
    ));
    panel.spawn(screen_label_key(
        theme,
        fonts,
        i18n,
        "ui_gallery.typography.description",
        "Theme roles, real Latin fixture weights, mixed text, wrapping, and bounded overflow.",
        UiThemeTextStyleRole::Body,
        UiThemeTextColorRole::Muted,
    ));

    panel
        .spawn(ui_column(theme.layout.row_gap))
        .with_children(|samples| {
            samples.spawn(screen_title_key(
                theme,
                fonts,
                i18n,
                "ui_gallery.typography.large_title",
                "Large Title",
                UiThemeTextStyleRole::TitleLarge,
            ));
            samples.spawn(screen_title_key(
                theme,
                fonts,
                i18n,
                "ui_gallery.typography.section_title",
                "Section Title",
                UiThemeTextStyleRole::Title,
            ));
            samples.spawn(screen_label_key(
                theme,
                fonts,
                i18n,
                "ui_gallery.typography.subtitle",
                "Subtitle text",
                UiThemeTextStyleRole::Subtitle,
                UiThemeTextColorRole::Muted,
            ));
            samples.spawn(screen_label_key(
                theme,
                fonts,
                i18n,
                "ui_gallery.typography.body",
                "Body text",
                UiThemeTextStyleRole::Body,
                UiThemeTextColorRole::Primary,
            ));
            samples.spawn(screen_label_key(
                theme,
                fonts,
                i18n,
                "ui_gallery.typography.caption",
                "Caption text",
                UiThemeTextStyleRole::Caption,
                UiThemeTextColorRole::Muted,
            ));
            samples.spawn(screen_label_key(
                theme,
                fonts,
                i18n,
                "ui_gallery.typography.button",
                "Button label role",
                UiThemeTextStyleRole::Button,
                UiThemeTextColorRole::Primary,
            ));
        });

    panel.spawn(screen_label_key(
        theme,
        fonts,
        i18n,
        "ui_gallery.typography.weights",
        "Latin fixture weights",
        UiThemeTextStyleRole::SectionLabel,
        UiThemeTextColorRole::Muted,
    ));
    panel
        .spawn(gallery_grid(
            metrics,
            width_class,
            UiResponsiveGridColumns::new(1, 3, 3),
        ))
        .with_children(|weights| {
            for (weight, text) in GALLERY_TYPOGRAPHY_WEIGHTS {
                let style = UiTextStyleToken::latin_fixture(weight, theme.text.body);
                weights.spawn((
                    try_ui_styled_text(fonts, text, style, theme.colors.text_primary)
                        .expect("Gallery fixture weight style must be valid"),
                    Node {
                        width: percent(100),
                        min_height: px(36),
                        ..default()
                    },
                    Name::new(format!("Gallery typography {weight:?}")),
                ));
            }
        });
}

fn spawn_gallery_typography_overflow(
    panel: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
) {
    panel.spawn(screen_label_key(
        theme,
        fonts,
        i18n,
        "ui_gallery.typography.overflow",
        "Mixed text and overflow states",
        UiThemeTextStyleRole::SectionLabel,
        UiThemeTextColorRole::Muted,
    ));

    let mixed_style = UiTextStyleToken::latin_fixture(UiFontWeight::Bold, theme.text.body);
    panel.spawn((
        try_ui_styled_text(
            fonts,
            GALLERY_TYPOGRAPHY_MIXED_TEXT,
            mixed_style,
            theme.colors.text_primary,
        )
        .expect("Gallery mixed text style must be valid"),
        Node {
            width: percent(100),
            ..default()
        },
        Name::new("Gallery whole-node CJK fallback sample"),
    ));

    let mut long_word_style = UiTextStyleToken::for_theme_role(theme, UiThemeTextStyleRole::Body);
    long_word_style.wrap = UiTextWrap::WordOrCharacter;
    panel.spawn((
        try_ui_styled_text(
            fonts,
            GALLERY_TYPOGRAPHY_LONG_WORD,
            long_word_style,
            theme.colors.text_primary,
        )
        .expect("Gallery long word style must be valid"),
        Node {
            width: percent(100),
            max_width: px(420),
            ..default()
        },
        Name::new("Gallery long English word sample"),
    ));

    panel.spawn((
        screen_label(
            theme,
            fonts,
            GALLERY_TYPOGRAPHY_LONG_CJK,
            UiThemeTextStyleRole::Body,
            UiThemeTextColorRole::Primary,
        ),
        Node {
            width: percent(100),
            max_width: px(520),
            ..default()
        },
        Name::new("Gallery long Chinese sample"),
    ));

    let mut clip_style = UiTextStyleToken::for_theme_role(theme, UiThemeTextStyleRole::Body);
    clip_style.wrap = UiTextWrap::NoWrap;
    clip_style.truncation = UiTextTruncation::Clip;
    panel
                .spawn((
                    gallery_typography_clip_frame(),
                    Name::new("Gallery clipped text frame"),
                ))
                .with_children(|clip_frame| {
                    clip_frame.spawn((
                        try_ui_styled_text(
                            fonts,
                            "Clip / 0123456789 / This text stays intact and the constrained parent clips it.",
                            clip_style,
                            theme.colors.text_primary,
                        )
                        .expect("Gallery clip style must be valid"),
                        Name::new("Gallery clipped text sample"),
                    ));
                });

    let mut ellipsis_style = UiTextStyleToken::for_theme_role(theme, UiThemeTextStyleRole::Body);
    ellipsis_style.wrap = UiTextWrap::NoWrap;
    ellipsis_style.truncation = UiTextTruncation::Ellipsis { max_graphemes: 22 };
    panel.spawn((
        try_ui_styled_text(
            fonts,
            "Ellipsis / 中文和English按字素簇安全截断 / 0123456789",
            ellipsis_style,
            theme.colors.text_primary,
        )
        .expect("Gallery ellipsis style must be valid"),
        Node {
            width: percent(100),
            ..default()
        },
        Name::new("Gallery grapheme ellipsis sample"),
    ));
}

fn gallery_typography_status_panel(theme: &UiTheme) -> impl Bundle {
    (
        UiThemePanelNodeRole::Content,
        gallery_typography_status_panel_node(theme),
        BackgroundColor(theme.colors.panel_background),
        BorderColor::all(theme.colors.panel_border),
        UiThemeBackgroundRole::Panel,
        UiThemeBorderRole::Panel,
    )
}

fn gallery_typography_status_panel_node(theme: &UiTheme) -> Node {
    let height = theme.layout.panel_gap * 2.0
        + theme.text.section_label * 1.25
        + theme.layout.card_gap * 2.0
        + theme.text.caption * 1.4
        + theme.text.caption * 2.7;
    Node {
        width: percent(100),
        max_width: px(theme.layout.content_width),
        height: px(height),
        align_self: AlignSelf::Center,
        flex_shrink: 0.0,
        flex_direction: FlexDirection::Column,
        row_gap: px(theme.layout.card_gap),
        padding: UiRect::all(px(theme.layout.panel_gap)),
        border: UiRect::all(px(theme.panel.border)),
        border_radius: BorderRadius::all(px(theme.panel.radius)),
        overflow: Overflow::clip(),
        ..default()
    }
}

fn spawn_gallery_typography_status_samples(
    panel: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
) {
    panel.spawn(section_label_key(
        theme,
        fonts,
        i18n,
        "ui_gallery.typography.boundary",
        "Alignment and missing glyph",
    ));

    let mut centered_style = UiTextStyleToken::for_theme_role(theme, UiThemeTextStyleRole::Caption);
    centered_style.alignment = UiTextAlignment::Center;
    panel.spawn((
        try_ui_styled_text(
            fonts,
            "Centered / punctuation ！？，. / 2026",
            centered_style,
            theme.colors.text_muted,
        )
        .expect("Gallery centered style must be valid"),
        Node {
            width: percent(100),
            max_width: px(420),
            min_height: px(theme.text.caption * 1.4),
            ..default()
        },
        Name::new("Gallery centered text sample"),
    ));

    let missing_style = UiTextStyleToken::latin_fixture(UiFontWeight::Regular, theme.text.caption);
    panel.spawn((
        try_ui_styled_text(
            fonts,
            "Missing glyph sample: 🙂 becomes explicit question mark",
            missing_style,
            theme.colors.text_muted,
        )
        .expect("Gallery missing glyph style must be valid"),
        Node {
            width: percent(100),
            min_height: px(theme.text.caption * 2.7),
            ..default()
        },
        Name::new("Gallery missing glyph replacement sample"),
    ));
}

fn gallery_stress_item(theme: &UiTheme, index: usize) -> impl Bundle {
    (
        Node {
            width: percent(100),
            min_height: px(82),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::SpaceBetween,
            row_gap: px(theme.layout.row_gap * 0.5),
            padding: UiRect::all(px(theme.layout.row_gap)),
            border: UiRect::all(px(theme.panel.border)),
            border_radius: BorderRadius::all(px(theme.button.radius)),
            ..default()
        },
        BackgroundColor(theme.colors.secondary_button.idle),
        BorderColor::all(theme.colors.panel_border),
        Name::new(format!("Gallery stress item {}", index + 1)),
    )
}

fn spawn_gallery_stress_item(
    items: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    index: usize,
) {
    let title = format!(
        "{} {:02}",
        i18n.tr("ui_gallery.stress.item", "Item"),
        index + 1
    );
    let state = if index % 3 == 0 {
        i18n.tr("ui_gallery.stress.state.ready", "Ready")
    } else if index % 3 == 1 {
        i18n.tr("ui_gallery.stress.state.waiting", "Waiting")
    } else {
        i18n.tr("ui_gallery.stress.state.done", "Done")
    };

    items
        .spawn(gallery_stress_item(theme, index))
        .with_children(|item| {
            item.spawn(screen_label(
                theme,
                fonts,
                title,
                UiThemeTextStyleRole::Caption,
                UiThemeTextColorRole::Primary,
            ));
            item.spawn(screen_label(
                theme,
                fonts,
                state,
                UiThemeTextStyleRole::Caption,
                UiThemeTextColorRole::Muted,
            ));
            item.spawn(secondary_action_button_key(
                theme,
                metrics,
                fonts,
                i18n,
                "ui_gallery.stress.action",
                "Inspect",
            ));
        });
}

fn spawn_gallery_image_card(
    images: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    fonts: &UiFontAssets,
    image: Handle<Image>,
    path: &'static str,
) {
    images
        .spawn(gallery_image_card_node(theme))
        .with_children(|card| {
            card.spawn(ui_image_panel_node(UiImageSize::FullWidthAspect {
                aspect_ratio: 16.0 / 9.0,
            }))
            .with_children(|panel| {
                panel.spawn(ui_image(
                    image,
                    UiImageFit::Stretch,
                    UiImageSize::PercentBox {
                        width: 100.0,
                        height: 100.0,
                    },
                ));
            });
            card.spawn(screen_label(
                theme,
                fonts,
                path,
                UiThemeTextStyleRole::Caption,
                UiThemeTextColorRole::Muted,
            ));
        });
}

fn spawn_gallery_image_fit_sample(
    samples: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    fonts: &UiFontAssets,
    image: Handle<Image>,
    sample: GalleryImageFitSample,
) {
    samples
        .spawn(gallery_image_card_node(theme))
        .with_children(|card| {
            card.spawn(Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: px(theme.layout.row_gap),
                ..default()
            })
            .with_children(|previews| {
                spawn_gallery_image_fit_preview(
                    previews,
                    image.clone(),
                    sample.landscape_fit,
                    UiImageSize::constrained(
                        UiImageConstraints::new(UiImageLength::Px(72.0), UiImageLength::Auto)
                            .with_aspect_ratio(72.0 / 44.0)
                            .with_min_width(UiImageLength::Px(64.0))
                            .with_max_width(UiImageLength::Px(80.0))
                            .with_min_height(UiImageLength::Px(40.0))
                            .with_max_height(UiImageLength::Px(52.0)),
                    ),
                );
                spawn_gallery_image_fit_preview(
                    previews,
                    image,
                    sample.portrait_fit,
                    UiImageSize::FixedBox {
                        width: 44.0,
                        height: 72.0,
                    },
                );
            });
            card.spawn(screen_label(
                theme,
                fonts,
                sample.label,
                UiThemeTextStyleRole::Caption,
                UiThemeTextColorRole::Muted,
            ));
        });
}

fn spawn_gallery_image_fit_preview(
    previews: &mut ChildSpawnerCommands,
    image: Handle<Image>,
    fit: UiImageFit,
    frame_size: UiImageSize,
) {
    previews
        .spawn(ui_image_panel_node_with_radius(frame_size, 8.0))
        .with_children(|frame| {
            frame.spawn(ui_image(
                image,
                fit,
                UiImageSize::PercentBox {
                    width: 100.0,
                    height: 100.0,
                },
            ));
        });
}

fn spawn_gallery_nine_slice_samples(
    image_modes: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    width_class: UiWidthClass,
    asset_server: &AssetServer,
) {
    image_modes.spawn(screen_label(
        theme,
        fonts,
        "Nine-slice",
        UiThemeTextStyleRole::Caption,
        UiThemeTextColorRole::Muted,
    ));
    image_modes
        .spawn(gallery_image_card_node(theme))
        .with_children(|card| {
            spawn_gallery_advanced_preview(
                card,
                asset_server,
                gallery_nine_slice_spec(),
                UiImageSize::FixedBox {
                    width: 184.0,
                    height: 84.0,
                },
                "Gallery nine-slice panel",
            );
            card.spawn(screen_label(
                theme,
                fonts,
                "Panel border 184 x 84",
                UiThemeTextStyleRole::Caption,
                UiThemeTextColorRole::Muted,
            ));
        });
    image_modes
        .spawn(gallery_grid(
            metrics,
            width_class,
            UiResponsiveGridColumns::new(1, 3, 3),
        ))
        .with_children(|buttons| {
            for (label, width, height) in [
                ("Small 72 x 32", 72.0, 32.0),
                ("Medium 104 x 40", 104.0, 40.0),
                ("Large 144 x 48", 144.0, 48.0),
            ] {
                buttons
                    .spawn(gallery_image_card_node(theme))
                    .with_children(|card| {
                        spawn_gallery_advanced_preview(
                            card,
                            asset_server,
                            gallery_nine_slice_spec(),
                            UiImageSize::FixedBox { width, height },
                            "Gallery nine-slice button border",
                        );
                        card.spawn(screen_label(
                            theme,
                            fonts,
                            label,
                            UiThemeTextStyleRole::Caption,
                            UiThemeTextColorRole::Muted,
                        ));
                    });
            }
        });
}

fn spawn_gallery_tiling_samples(
    image_modes: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    width_class: UiWidthClass,
    asset_server: &AssetServer,
) {
    image_modes.spawn((
        screen_label(
            theme,
            fonts,
            "Bounded tiling",
            UiThemeTextStyleRole::Caption,
            UiThemeTextColorRole::Muted,
        ),
        ANCHOR_UI_GALLERY_IMAGE_TILING,
        Name::new("Gallery tiling audit anchor"),
    ));
    image_modes
        .spawn(gallery_grid(
            metrics,
            width_class,
            UiResponsiveGridColumns::new(1, 3, 3),
        ))
        .with_children(|tiles| {
            for (label, axis, width, height) in [
                ("Tile X", UiTileAxis::X, 184.0, 52.0),
                ("Tile Y", UiTileAxis::Y, 92.0, 116.0),
                ("Tile Both", UiTileAxis::Both, 184.0, 116.0),
            ] {
                tiles
                    .spawn(gallery_image_card_node(theme))
                    .with_children(|card| {
                        spawn_gallery_advanced_preview(
                            card,
                            asset_server,
                            gallery_tiling_spec(axis),
                            UiImageSize::FixedBox { width, height },
                            "Gallery bounded tile preview",
                        );
                        card.spawn(screen_label(
                            theme,
                            fonts,
                            label,
                            UiThemeTextStyleRole::Caption,
                            UiThemeTextColorRole::Muted,
                        ));
                    });
            }
        });
}

fn spawn_gallery_atlas_frame_samples(
    image_modes: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    width_class: UiWidthClass,
    asset_server: &AssetServer,
) {
    image_modes.spawn((
        screen_label(
            theme,
            fonts,
            "Atlas frames",
            UiThemeTextStyleRole::Caption,
            UiThemeTextColorRole::Muted,
        ),
        ANCHOR_UI_GALLERY_IMAGE_ATLAS,
        Name::new("Gallery atlas audit anchor"),
    ));
    image_modes
        .spawn(gallery_grid(
            metrics,
            width_class,
            UiResponsiveGridColumns::new(2, 4, 4),
        ))
        .with_children(|frames| {
            for sample in GALLERY_ATLAS_FRAME_SAMPLES {
                frames
                    .spawn(gallery_image_card_node(theme))
                    .with_children(|card| {
                        spawn_gallery_advanced_preview(
                            card,
                            asset_server,
                            gallery_atlas_frame_spec(sample),
                            UiImageSize::FixedBox {
                                width: 56.0,
                                height: 56.0,
                            },
                            "Gallery atlas frame preview",
                        );
                        card.spawn(screen_label(
                            theme,
                            fonts,
                            sample.label,
                            UiThemeTextStyleRole::Caption,
                            UiThemeTextColorRole::Muted,
                        ));
                    });
            }
        });
}

fn spawn_gallery_advanced_preview(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    spec: UiAdvancedImageSpec,
    size: UiImageSize,
    name: &'static str,
) {
    parent
        .spawn(ui_image_panel_node(size))
        .insert(Name::new(name))
        .with_children(|frame| {
            frame.spawn(
                try_ui_advanced_image(
                    asset_server,
                    spec,
                    UiImageSize::PercentBox {
                        width: 100.0,
                        height: 100.0,
                    },
                )
                .expect("Gallery advanced image fixture must be valid"),
            );
        });
}

fn gallery_nine_slice_spec() -> UiAdvancedImageSpec {
    UiAdvancedImageSpec {
        source: UiAdvancedImageSource::Texture(UiImageTextureSource::new(
            GALLERY_NINE_SLICE_SOURCE_PATH,
            UiImagePixelSize::new(48, 48),
        )),
        mode: UiAdvancedImageMode::NineSlice(UiNineSlice::uniform(12.0)),
    }
}

fn gallery_tiling_spec(axis: UiTileAxis) -> UiAdvancedImageSpec {
    let mut tiling = UiImageTiling::new(axis);
    tiling.stretch_value = 0.5;
    tiling.max_repeats = 32;
    UiAdvancedImageSpec {
        source: UiAdvancedImageSource::Texture(UiImageTextureSource::new(
            GALLERY_TILE_SOURCE_PATH,
            UiImagePixelSize::new(128, 64),
        )),
        mode: UiAdvancedImageMode::Tiled(tiling),
    }
}

fn gallery_atlas_frame_spec(sample: GalleryAtlasFrameSample) -> UiAdvancedImageSpec {
    UiAdvancedImageSpec {
        source: UiAdvancedImageSource::AtlasFrame(UiAtlasFrame {
            source: UiImageTextureSource::new(
                GALLERY_FRAME_SOURCE_PATH,
                UiImagePixelSize::new(128, 32),
            ),
            rect: UiImagePixelRect::new(sample.x, 0, 32, 32),
            original_size: UiImagePixelSize::new(32, 32),
            pivot: Some(sample.pivot),
        }),
        mode: UiAdvancedImageMode::Stretch,
    }
}

fn spawn_gallery_visual_fixture(
    fixtures: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    image: Handle<Image>,
    path: &'static str,
) {
    let mut fixture = fixtures.spawn(gallery_image_card_node(theme));
    fixture.insert(Name::new(format!("Gallery visual fixture: {path}")));
    fixture.with_children(|card| {
        card.spawn(ui_image_panel_node(UiImageSize::FullWidthAspect {
            aspect_ratio: 1.0,
        }))
        .with_children(|panel| {
            panel.spawn(ui_image(
                image,
                UiImageFit::Stretch,
                UiImageSize::PercentBox {
                    width: 100.0,
                    height: 100.0,
                },
            ));
        });
    });
}

fn spawn_gallery_atlas_source_thumbnail(
    thumbnails: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    fonts: &UiFontAssets,
    image: Handle<Image>,
    path: &'static str,
) {
    thumbnails
        .spawn(gallery_image_card_node(theme))
        .with_children(|card| {
            card.spawn(ui_image_panel_node(UiImageSize::FullWidthAspect {
                aspect_ratio: 1.0,
            }))
            .with_children(|panel| {
                panel.spawn(ui_image(
                    image,
                    UiImageFit::Stretch,
                    UiImageSize::PercentBox {
                        width: 100.0,
                        height: 100.0,
                    },
                ));
            });
            card.spawn(screen_label(
                theme,
                fonts,
                gallery_image_file_name(path),
                UiThemeTextStyleRole::Caption,
                UiThemeTextColorRole::Muted,
            ));
        });
}

fn gallery_image_card_node(theme: &UiTheme) -> impl Bundle {
    (
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: px(theme.layout.row_gap * 0.5),
            padding: UiRect::all(px(theme.layout.row_gap)),
            border: UiRect::all(px(theme.panel.border)),
            border_radius: BorderRadius::all(px(theme.button.radius)),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(theme.colors.secondary_button.idle),
        BorderColor::all(theme.colors.panel_border),
        Name::new("Gallery image card"),
    )
}

fn gallery_image_file_name(path: &'static str) -> &'static str {
    path.rsplit('/').next().unwrap_or(path)
}

fn section_label_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    screen_label_key(
        theme,
        fonts,
        i18n,
        key,
        fallback,
        UiThemeTextStyleRole::SectionLabel,
        UiThemeTextColorRole::Muted,
    )
}

fn primary_route_button_sample(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
) -> impl Bundle {
    (
        primary_action_button_key(
            theme,
            metrics,
            fonts,
            i18n,
            "ui_gallery.buttons.action",
            "Action",
        ),
        Name::new("Gallery action sample"),
    )
}

fn spawn_gallery_text_input<const N: usize>(
    inputs: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    placeholder: String,
    value: impl Into<String>,
    states: [GalleryTextInputState; N],
) {
    inputs
        .spawn(ui_column(theme.layout.row_gap * 0.5))
        .with_children(|field| {
            let mut input = field.spawn(text_input(theme, metrics, fonts, placeholder, value));
            for state in states {
                match state {
                    GalleryTextInputState::Helper(message) => {
                        input.insert(UiTextInputHelperText(message));
                    }
                    GalleryTextInputState::Required(message) => {
                        input.insert(UiTextInputRequired::new(message));
                    }
                    GalleryTextInputState::Validation(message) => {
                        input.insert(UiTextInputValidationMessage(message));
                    }
                    GalleryTextInputState::Alphanumeric {
                        min_chars,
                        max_chars,
                        message,
                    } => {
                        input.insert(UiTextInputAlphanumeric::new(min_chars, max_chars, message));
                    }
                    GalleryTextInputState::Error => {
                        input.insert(UiTextInputError);
                    }
                    GalleryTextInputState::MaxChars(max_chars) => {
                        input.insert(UiTextInputMaxChars(max_chars));
                    }
                    GalleryTextInputState::Readonly => {
                        input.insert(ReadonlyTextInput);
                    }
                    GalleryTextInputState::Disabled => {
                        input.insert(DisabledTextInput);
                    }
                }
            }

            let input_entity = input.id();
            field.spawn(text_input_form_message(theme, fonts, input_entity));
        });
}

fn gallery_confirm_modal(i18n: &UiI18n) -> UiConfirmModal {
    let title = UiI18nTextSpec::new(i18n, "ui_gallery.confirm.title", "Gallery Confirm");
    let body = UiI18nTextSpec::new(
        i18n,
        "ui_gallery.confirm.body",
        "This confirms modal layering and input blocking.",
    );
    let detail = UiI18nTextSpec::new(
        i18n,
        "ui_gallery.confirm.detail",
        "The page buttons below should not react while this is open.",
    );
    let cancel = UiI18nTextSpec::new(i18n, "common.cancel", "Cancel");
    let confirm = UiI18nTextSpec::new(i18n, "common.confirm", "Confirm");

    UiConfirmModal {
        id: MODAL_GALLERY_CONFIRM,
        title: title.text,
        body: body.text,
        detail: Some(detail.text),
        title_i18n_text: Some(title.i18n_text),
        body_i18n_text: Some(body.i18n_text),
        detail_i18n_text: Some(detail.i18n_text),
        actions: vec![
            UiModalActionSpec {
                label: cancel.text,
                action: ACTION_CANCEL,
                style: UiModalActionStyle::Secondary,
                i18n_text: Some(cancel.i18n_text),
            },
            UiModalActionSpec {
                label: confirm.text,
                action: ACTION_CONFIRM,
                style: UiModalActionStyle::Primary,
                i18n_text: Some(confirm.i18n_text),
            },
        ],
    }
}

fn gallery_floating_panel(i18n: &UiI18n) -> UiFloatingPanel {
    UiFloatingPanel {
        id: PANEL_GALLERY_FLOATING,
        title: i18n.tr("ui_gallery.floating.title", "Floating Panel"),
        body: i18n.tr(
            "ui_gallery.floating.body",
            "This panel does not cover the whole page.",
        ),
        detail: Some(i18n.tr(
            "ui_gallery.floating.detail",
            "Use Close Top or Esc to close it.",
        )),
    }
}

fn gallery_floating_i18n(i18n: &UiI18n) -> GalleryFloatingI18n {
    GalleryFloatingI18n {
        panel_id: PANEL_GALLERY_FLOATING,
        title: UiI18nTextSpec::new(i18n, "ui_gallery.floating.title", "Floating Panel"),
        body: UiI18nTextSpec::new(
            i18n,
            "ui_gallery.floating.body",
            "This panel does not cover the whole page.",
        ),
        detail: Some(UiI18nTextSpec::new(
            i18n,
            "ui_gallery.floating.detail",
            "Use Close Top or Esc to close it.",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Component)]
    struct GalleryStyleTileTestRoot;

    #[test]
    fn gallery_button_columns_are_single_column_on_compact() {
        assert_eq!(
            gallery_button_columns().for_width_class(UiWidthClass::Compact),
            1
        );
        assert_eq!(
            gallery_selection_columns().for_width_class(UiWidthClass::Compact),
            1
        );
        assert_eq!(
            gallery_stress_columns().for_width_class(UiWidthClass::Compact),
            1
        );
        assert_eq!(
            gallery_overlay_columns().for_width_class(UiWidthClass::Compact),
            1
        );
        assert_eq!(
            gallery_image_columns().for_width_class(UiWidthClass::Compact),
            1
        );
        assert_eq!(
            gallery_atlas_source_columns().for_width_class(UiWidthClass::Compact),
            2
        );
        assert_eq!(
            gallery_icon_state_columns().for_width_class(UiWidthClass::Compact),
            3
        );
    }

    #[test]
    fn gallery_columns_expand_on_expanded_width() {
        assert_eq!(
            gallery_button_columns().for_width_class(UiWidthClass::Expanded),
            4
        );
        assert_eq!(
            gallery_icon_button_columns().for_width_class(UiWidthClass::Expanded),
            5
        );
        assert_eq!(
            gallery_selection_columns().for_width_class(UiWidthClass::Expanded),
            3
        );
        assert_eq!(
            gallery_stress_columns().for_width_class(UiWidthClass::Expanded),
            3
        );
        assert_eq!(
            gallery_overlay_columns().for_width_class(UiWidthClass::Expanded),
            5
        );
        assert_eq!(
            gallery_image_columns().for_width_class(UiWidthClass::Expanded),
            2
        );
        assert_eq!(
            gallery_atlas_source_columns().for_width_class(UiWidthClass::Expanded),
            6
        );
        assert_eq!(
            gallery_icon_state_columns().for_width_class(UiWidthClass::Expanded),
            7
        );
    }

    #[test]
    fn icon_gallery_panels_own_stable_child_audit_anchors() {
        let theme = UiTheme::default();
        let mut app = App::new();
        let icons = app.world_mut().spawn(gallery_icons_panel(&theme)).id();
        let states = app
            .world_mut()
            .spawn(gallery_icon_states_panel(&theme))
            .id();

        assert!(app.world().entity(icons).contains::<GalleryIconsRegion>());
        assert_eq!(
            app.world()
                .entity(icons)
                .get::<crate::framework::ui::widgets::UiScrollAuditAnchorId>()
                .copied(),
            Some(ANCHOR_UI_GALLERY_ICONS)
        );
        assert!(
            app.world()
                .entity(states)
                .contains::<GalleryIconStatesRegion>()
        );
        assert_eq!(
            app.world()
                .entity(states)
                .get::<crate::framework::ui::widgets::UiScrollAuditAnchorId>()
                .copied(),
            Some(ANCHOR_UI_GALLERY_ICON_STATES)
        );
    }

    #[test]
    fn style_scope_gallery_panel_owns_stable_child_audit_anchor() {
        let theme = UiTheme::default();
        let mut app = App::new();
        let styles = app
            .world_mut()
            .spawn(gallery_style_scopes_panel(&theme))
            .id();

        assert!(
            app.world()
                .entity(styles)
                .contains::<GalleryStyleScopesRegion>()
        );
        assert_eq!(
            app.world()
                .entity(styles)
                .get::<crate::framework::ui::widgets::UiScrollAuditAnchorId>()
                .copied(),
            Some(ANCHOR_UI_GALLERY_STYLE_SCOPES)
        );
    }

    #[test]
    fn style_scope_tile_uses_caption_binding_and_theme_caption_size() {
        let theme = UiTheme::default();
        let fonts = UiFontAssets::test_registry();
        let i18n = UiI18n::test_with_texts(
            "en_us",
            &[("ui_gallery.style_scopes.parent", "Parent scope")],
        );
        let caption_size = theme.text.caption;
        let mut app = App::new();
        app.insert_resource(theme)
            .insert_resource(fonts)
            .insert_resource(i18n)
            .add_systems(
                Update,
                |mut commands: Commands,
                 theme: Res<UiTheme>,
                 fonts: Res<UiFontAssets>,
                 i18n: Res<UiI18n>| {
                    commands
                        .spawn((Node::default(), GalleryStyleTileTestRoot))
                        .with_children(|parent| {
                            spawn_gallery_style_tile(
                                parent,
                                &theme,
                                &fonts,
                                &i18n,
                                "ui_gallery.style_scopes.parent",
                                "Parent scope",
                                Some(UI_STYLE_VARIANT_GALLERY_PARENT),
                            );
                        });
                },
            );
        app.update();
        let mut roots = app
            .world_mut()
            .query_filtered::<Entity, With<GalleryStyleTileTestRoot>>();
        let root = roots.single(app.world()).unwrap();
        let tile = app.world().get::<Children>(root).unwrap()[0];
        let label = app.world().get::<Children>(tile).unwrap()[0];

        assert_eq!(
            app.world()
                .get::<UiStyleBinding>(label)
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .role,
            UiTextStyleRole::Caption
        );
        assert_eq!(
            app.world()
                .get::<UiTextStyleToken>(label)
                .unwrap()
                .font_size,
            caption_size
        );
    }

    #[test]
    fn icon_state_preview_writes_existing_interaction_and_marker_sources() {
        let mut world = World::new();
        let hovered = world
            .spawn(GalleryIconStatePreview(UiButtonVisualState::Hovered))
            .id();
        let focused = world
            .spawn(GalleryIconStatePreview(UiButtonVisualState::Focused))
            .id();
        let disabled = world
            .spawn(GalleryIconStatePreview(UiButtonVisualState::Disabled))
            .id();
        let loading = world
            .spawn(GalleryIconStatePreview(UiButtonVisualState::Loading))
            .id();

        apply_gallery_icon_state_previews(&mut world);

        assert_eq!(
            world.get::<Interaction>(hovered),
            Some(&Interaction::Hovered)
        );
        assert!(world.get::<FocusedButton>(focused).is_some());
        assert!(
            world
                .get::<crate::framework::ui::widgets::DisabledButton>(disabled)
                .is_some()
        );
        assert!(
            world
                .get::<crate::framework::ui::widgets::LoadingButton>(loading)
                .is_some()
        );
        assert!(world.get::<SelectedButton>(focused).is_none());
    }

    #[test]
    fn visual_foundation_fixture_paths_are_stable_and_unique() {
        assert_eq!(GALLERY_VISUAL_FIXTURE_PATHS.len(), 4);
        assert!(
            GALLERY_VISUAL_FIXTURE_PATHS
                .iter()
                .all(|path| path.starts_with("ui/fixtures/visual-foundation/"))
        );
        let mut paths = GALLERY_VISUAL_FIXTURE_PATHS.to_vec();
        paths.sort_unstable();
        paths.dedup();
        assert_eq!(paths.len(), GALLERY_VISUAL_FIXTURE_PATHS.len());
    }

    #[test]
    fn image_fit_gallery_covers_every_mode_and_both_frame_orientations() {
        assert_eq!(GALLERY_IMAGE_FIT_SAMPLES.len(), 4);
        assert!(matches!(
            GALLERY_IMAGE_FIT_SAMPLES[0].landscape_fit,
            UiImageFit::Natural
        ));
        assert!(matches!(
            GALLERY_IMAGE_FIT_SAMPLES[1].landscape_fit,
            UiImageFit::Stretch
        ));
        assert!(matches!(
            GALLERY_IMAGE_FIT_SAMPLES[2].landscape_fit,
            UiImageFit::Contain
        ));
        assert!(matches!(
            GALLERY_IMAGE_FIT_SAMPLES[3].landscape_fit,
            UiImageFit::Cover { .. }
        ));
        assert!(matches!(
            GALLERY_IMAGE_FIT_SAMPLES[3].portrait_fit,
            UiImageFit::Cover { .. }
        ));
        assert!(GALLERY_IMAGE_FIT_SOURCE_PATH.ends_with("non-square-2x1.png"));
    }

    #[test]
    fn advanced_image_gallery_specs_cover_slice_all_tile_axes_and_four_frames() {
        assert_eq!(gallery_nine_slice_spec().validate(), Ok(()));
        for axis in [UiTileAxis::X, UiTileAxis::Y, UiTileAxis::Both] {
            assert_eq!(gallery_tiling_spec(axis).validate(), Ok(()));
        }

        assert_eq!(GALLERY_ATLAS_FRAME_SAMPLES.len(), 4);
        let mut frame_starts = Vec::new();
        for sample in GALLERY_ATLAS_FRAME_SAMPLES {
            let spec = gallery_atlas_frame_spec(sample);
            assert_eq!(spec.validate(), Ok(()));
            let UiAdvancedImageSource::AtlasFrame(frame) = spec.source else {
                panic!("atlas sample should use a formal frame descriptor");
            };
            frame_starts.push(frame.rect.x);
        }
        assert_eq!(frame_starts, vec![0, 32, 64, 96]);
    }

    #[test]
    fn typography_gallery_covers_three_real_weights_and_boundary_strings() {
        assert_eq!(GALLERY_TYPOGRAPHY_WEIGHTS.len(), 3);
        assert_eq!(
            GALLERY_TYPOGRAPHY_WEIGHTS
                .iter()
                .map(|(weight, _)| *weight)
                .collect::<Vec<_>>(),
            vec![
                UiFontWeight::Regular,
                UiFontWeight::Medium,
                UiFontWeight::Bold,
            ]
        );
        assert!(GALLERY_TYPOGRAPHY_MIXED_TEXT.contains("MyBevy"));
        assert!(GALLERY_TYPOGRAPHY_MIXED_TEXT.contains("中文"));
        assert!(GALLERY_TYPOGRAPHY_MIXED_TEXT.contains("2026"));
        assert!(!GALLERY_TYPOGRAPHY_LONG_WORD.contains(char::is_whitespace));
        assert!(GALLERY_TYPOGRAPHY_LONG_CJK.chars().count() > 30);
    }

    #[test]
    fn typography_status_panel_is_an_explicit_fixed_height_scroll_sibling() {
        let theme = UiTheme::default();
        let node = gallery_typography_status_panel_node(&theme);
        let expected_height = theme.layout.panel_gap * 2.0
            + theme.text.section_label * 1.25
            + theme.layout.card_gap * 2.0
            + theme.text.caption * 1.4
            + theme.text.caption * 2.7;

        assert_eq!(node.height, px(expected_height));
        assert_eq!(node.min_height, Val::Auto);
        assert_eq!(node.flex_shrink, 0.0);
        assert_eq!(node.flex_direction, FlexDirection::Column);
        assert_eq!(node.overflow, Overflow::clip());
    }

    #[test]
    fn typography_overflow_anchor_is_on_its_own_panel_bundle() {
        let theme = UiTheme::default();
        let mut app = App::new();
        let entity = app
            .world_mut()
            .spawn(gallery_typography_overflow_panel(
                &theme,
                UiWidthClass::Expanded,
            ))
            .id();
        let entity_ref = app.world().entity(entity);

        assert!(entity_ref.contains::<GalleryTypographyOverflowRegion>());
        assert_eq!(
            entity_ref
                .get::<crate::framework::ui::widgets::UiScrollAuditAnchorId>()
                .copied(),
            Some(ANCHOR_UI_GALLERY_TYPOGRAPHY_OVERFLOW)
        );
        let node = entity_ref.get::<Node>().unwrap();
        assert_eq!(node.flex_direction, FlexDirection::Column);
        assert_eq!(node.width, percent(100));
    }

    #[test]
    fn typography_overflow_panel_uses_width_specific_explicit_line_budgets() {
        let theme = UiTheme::default();
        assert_eq!(
            gallery_typography_overflow_line_budget(UiWidthClass::Compact),
            GALLERY_TYPOGRAPHY_COMPACT_LINE_BUDGET
        );
        assert_eq!(
            gallery_typography_overflow_line_budget(UiWidthClass::Medium),
            GALLERY_TYPOGRAPHY_WIDE_LINE_BUDGET
        );
        assert_eq!(
            gallery_typography_overflow_line_budget(UiWidthClass::Expanded),
            GALLERY_TYPOGRAPHY_WIDE_LINE_BUDGET
        );

        let expected_height = |budget: GalleryTypographyLineBudget| {
            theme.layout.panel_gap * 2.0
                + theme.text.section_label * GALLERY_TYPOGRAPHY_SECTION_LINE_HEIGHT
                + budget.mixed as f32 * theme.text.body * GALLERY_TYPOGRAPHY_MIXED_LINE_HEIGHT
                + (budget.long_word + budget.long_cjk + budget.ellipsis) as f32
                    * theme.text.body
                    * GALLERY_TYPOGRAPHY_BODY_LINE_HEIGHT
                + GALLERY_TYPOGRAPHY_CLIP_FRAME_HEIGHT
                + GALLERY_TYPOGRAPHY_OVERFLOW_CHILD_GAPS * theme.layout.card_gap
                + GALLERY_TYPOGRAPHY_BORDER_ROUNDING_ALLOWANCE
        };

        for (width_class, budget) in [
            (
                UiWidthClass::Compact,
                GALLERY_TYPOGRAPHY_COMPACT_LINE_BUDGET,
            ),
            (UiWidthClass::Medium, GALLERY_TYPOGRAPHY_WIDE_LINE_BUDGET),
            (UiWidthClass::Expanded, GALLERY_TYPOGRAPHY_WIDE_LINE_BUDGET),
        ] {
            let node = gallery_typography_overflow_panel_node(&theme, width_class);
            assert_eq!(node.height, px(expected_height(budget)));
            assert_eq!(node.flex_shrink, 0.0);
            assert_eq!(node.overflow, Overflow::clip());
        }

        let compact_height =
            gallery_typography_overflow_panel_height(&theme, UiWidthClass::Compact);
        let expanded_height =
            gallery_typography_overflow_panel_height(&theme, UiWidthClass::Expanded);
        assert!((compact_height - 473.2).abs() < 0.001);
        assert!((expanded_height - 346.0).abs() < 0.001);
        assert!(compact_height > expanded_height);
    }

    #[test]
    fn typography_overflow_budget_and_clip_frame_share_named_height() {
        let theme = UiTheme::default();
        let mut app = App::new();
        let clip_frame = app.world_mut().spawn(gallery_typography_clip_frame()).id();
        let node = app.world().entity(clip_frame).get::<Node>().unwrap();

        assert_eq!(node.height, px(GALLERY_TYPOGRAPHY_CLIP_FRAME_HEIGHT));
        assert_eq!(node.width, px(GALLERY_TYPOGRAPHY_CLIP_FRAME_WIDTH));
        assert_eq!(node.overflow, Overflow::clip());
        assert_eq!(
            gallery_typography_overflow_panel_height(&theme, UiWidthClass::Expanded),
            344.0 + GALLERY_TYPOGRAPHY_BORDER_ROUNDING_ALLOWANCE
        );
    }

    #[test]
    fn visual_foundation_manifest_is_parseable_and_assets_exist() {
        let manifest = include_str!("../../../../assets/ui/fixtures/manifest.ron");
        ron::de::from_str::<ron::Value>(manifest).expect("fixture manifest should be valid RON");

        for path in GALLERY_VISUAL_FIXTURE_PATHS
            .iter()
            .chain(GALLERY_VISUAL_FONT_FIXTURE_PATHS.iter())
        {
            assert!(manifest.contains(path), "manifest should contain {path}");
            assert!(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("assets")
                    .join(path)
                    .is_file(),
                "fixture asset should exist: {path}"
            );
        }
    }
}
