use bevy::prelude::*;

pub(in crate::game) struct UiThemePlugin;

impl Plugin for UiThemePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiTheme>();
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

#[derive(Clone, Debug)]
pub(in crate::game) struct UiTextTheme {
    pub title_large: f32,
    pub title: f32,
    pub subtitle: f32,
    pub section_label: f32,
    pub body: f32,
    pub caption: f32,
    pub button: f32,
}

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
pub(in crate::game) struct UiButtonTheme {
    pub min_width: f32,
    pub height: f32,
    pub padding_x: f32,
    pub radius: f32,
}

#[derive(Clone, Debug)]
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
