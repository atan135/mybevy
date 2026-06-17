use bevy::{ecs::hierarchy::ChildSpawnerCommands, prelude::*};

use crate::game::{
    navigation::{AppUiMode, secondary_route_button_key},
    ui::{
        core::{
            UI_PANEL_GLOBAL_LOADING, UiFloatingPanel, UiLayer, UiLayerRoot, UiMetrics,
            UiPanelCommand, UiPanelId, UiPanelKind, UiPanelRequest, UiPanelRoot, UiViewport,
            UiWidthClass,
            binding::{UiBindingValues, UiBoundDisabled, UiBoundText, UiBoundVisibility},
        },
        i18n::{UiI18n, UiI18nText},
        overlays::{
            UiConfirmModal, UiI18nTextSpec, UiLoading, UiModalActionSpec, UiModalActionStyle,
            UiRouteCommand, UiToast,
        },
        style::{
            UiFontAssets, UiTheme,
            theme::{
                UiThemeBackgroundRole, UiThemeBorderRole, UiThemePanelNodeRole,
                UiThemeRootNodeRole, UiThemeTextColorRole, UiThemeTextStyleRole,
            },
        },
        widgets::{
            DisabledButton, DisabledTextInput, FocusedButton, LoadingButton, ReadonlyTextInput,
            SelectedButton, UiAlign, UiImageFit, UiImageSize, UiJustify, UiResponsiveGridColumns,
            UiTextInputAlphanumeric, UiTextInputError, UiTextInputHelperText, UiTextInputMaxChars,
            UiTextInputRequired, UiTextInputSubmitted, UiTextInputValidationMessage, checkbox_key,
            checked_checkbox_key, disabled_checkbox_key, disabled_icon_button_key,
            disabled_primary_action_button_key, disabled_secondary_action_button_key,
            disabled_segment_option_key, disabled_slider_key, disabled_stepper_key,
            disabled_toggle_key, icon_button_key, loading_icon_button_key,
            loading_primary_action_button_key, primary_action_button_key, screen_label,
            screen_label_key, screen_title_key, secondary_action_button_key, segment_option_key,
            segmented_control, selected_segment_option_key, slider_key, stepper_key, text_input,
            text_input_form_message, toggle_key, toggle_on_key, ui_column, ui_image,
            ui_image_panel_node, ui_responsive_column, ui_responsive_grid, ui_scroll_column,
            ui_thumbnail_grid,
        },
    },
    ui_ids::{
        MODAL_ACTION_CANCEL, MODAL_ACTION_CONFIRM, MODAL_GALLERY_CONFIRM, OWNER_UI_GALLERY,
        PANEL_GALLERY_FLOATING, PANEL_UI_GALLERY_PAGE,
    },
};

const GALLERY_STRESS_ITEM_COUNT: usize = 96;
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
            UiPanelRoot {
                id: PANEL_UI_GALLERY_PAGE,
                kind: UiPanelKind::Page,
                owner: Some(OWNER_UI_GALLERY),
            },
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

            root.spawn(ui_scroll_column(theme)).with_children(|body| {
                body.spawn(gallery_panel(theme))
                    .with_children(|typography_panel| {
                        typography_panel.spawn(section_label_key(
                            theme,
                            fonts,
                            i18n,
                            "ui_gallery.typography.section",
                            "Typography",
                        ));
                        typography_panel
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
                            });
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

                body.spawn(gallery_panel(theme))
                    .with_children(|icon_buttons_panel| {
                        icon_buttons_panel.spawn(section_label_key(
                            theme,
                            fonts,
                            i18n,
                            "ui_gallery.icon_buttons.section",
                            "Icon Buttons",
                        ));
                        icon_buttons_panel
                            .spawn(gallery_grid(
                                metrics,
                                width_class,
                                gallery_icon_button_columns(),
                            ))
                            .with_children(|buttons| {
                                buttons.spawn(icon_button_key(
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    "+",
                                    "ui_gallery.icon_buttons.add",
                                    "Add",
                                ));
                                buttons.spawn((
                                    icon_button_key(
                                        theme,
                                        metrics,
                                        fonts,
                                        i18n,
                                        "-",
                                        "ui_gallery.icon_buttons.remove",
                                        "Remove",
                                    ),
                                    FocusedButton,
                                ));
                                buttons.spawn((
                                    icon_button_key(
                                        theme,
                                        metrics,
                                        fonts,
                                        i18n,
                                        "?",
                                        "ui_gallery.icon_buttons.help",
                                        "Help",
                                    ),
                                    SelectedButton,
                                ));
                                buttons.spawn(disabled_icon_button_key(
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    "x",
                                    "ui_gallery.icon_buttons.close",
                                    "Close",
                                ));
                                buttons.spawn(loading_icon_button_key(
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    "...",
                                    "ui_gallery.icon_buttons.loading",
                                    "Loading",
                                ));
                            });
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
    mut route_commands: MessageWriter<UiRouteCommand>,
    buttons: Query<
        (&Interaction, &GalleryActionButton),
        (
            Changed<Interaction>,
            With<Button>,
            Without<DisabledButton>,
            Without<LoadingButton>,
        ),
    >,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match action {
            GalleryActionButton::Toast => {
                route_commands.write(UiRouteCommand::ShowToast(UiToast::new_key(
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
        },
        BackgroundColor(theme.colors.panel_background),
        BorderColor::all(theme.colors.panel_border),
        UiThemeBackgroundRole::Panel,
        UiThemeBorderRole::Panel,
    )
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
                action: MODAL_ACTION_CANCEL,
                style: UiModalActionStyle::Secondary,
                i18n_text: Some(cancel.i18n_text),
            },
            UiModalActionSpec {
                label: confirm.text,
                action: MODAL_ACTION_CONFIRM,
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
    }
}
