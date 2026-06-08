use bevy::prelude::*;

use crate::game::ui::style::UiTheme;

pub(in crate::game) fn ui_column(gap: f32) -> impl Bundle {
    Node {
        width: percent(100),
        flex_direction: FlexDirection::Column,
        row_gap: px(gap),
        ..default()
    }
}

#[allow(dead_code)]
pub(in crate::game) fn ui_row(theme: &UiTheme) -> impl Bundle {
    Node {
        width: percent(100),
        align_items: AlignItems::Center,
        column_gap: px(theme.layout.row_column_gap),
        row_gap: px(theme.layout.row_gap),
        ..default()
    }
}

#[allow(dead_code)]
pub(in crate::game) fn ui_wrap_row(theme: &UiTheme) -> impl Bundle {
    Node {
        width: percent(100),
        align_items: AlignItems::Center,
        column_gap: px(theme.layout.row_column_gap),
        row_gap: px(theme.layout.row_gap),
        flex_wrap: FlexWrap::Wrap,
        ..default()
    }
}

pub(in crate::game) fn ui_grid(theme: &UiTheme, columns: u16) -> impl Bundle {
    Node {
        width: percent(100),
        display: Display::Grid,
        grid_template_columns: RepeatedGridTrack::flex(columns, 1.0),
        grid_auto_rows: vec![GridTrack::auto()],
        column_gap: px(theme.layout.row_column_gap),
        row_gap: px(theme.layout.row_gap),
        align_items: AlignItems::Center,
        justify_items: JustifyItems::Stretch,
        ..default()
    }
}
