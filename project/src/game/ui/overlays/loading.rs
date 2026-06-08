use bevy::prelude::*;

use crate::game::navigation::AppUiMode;
use crate::game::ui::{
    core::{UiLayer, UiLayerRoot, UiPanelId, UiPanelKind, UiPanelRoot},
    style::UiTheme,
    widgets::screen_label,
};

#[derive(Clone, Debug)]
pub(in crate::game) struct UiLoading {
    pub text: String,
}

impl UiLoading {
    #[allow(dead_code)]
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

pub(in crate::game) fn spawn_loading(
    commands: &mut Commands,
    theme: &UiTheme,
    loading: &UiLoading,
    owner_mode: Option<AppUiMode>,
) {
    commands.spawn((
        UiPanelRoot {
            id: UiPanelId::GlobalLoading,
            kind: UiPanelKind::BlockingOverlay,
            owner_mode,
        },
        UiLayerRoot {
            layer: UiLayer::Loading,
        },
        Button,
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::all(px(theme.layout.screen_padding)),
            ..default()
        },
        ZIndex(150),
        BackgroundColor(Color::srgba(0.01, 0.02, 0.03, 0.56)),
        children![(
            Node {
                min_width: px(260),
                max_width: px(420),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(22), px(16)),
                border: UiRect::all(px(theme.panel.border)),
                border_radius: BorderRadius::all(px(theme.panel.radius)),
                ..default()
            },
            BackgroundColor(theme.colors.panel_background),
            BorderColor::all(theme.colors.panel_border),
            children![screen_label(
                loading.text.clone(),
                theme.text.body,
                theme.colors.text_primary,
            )],
        )],
    ));
}
