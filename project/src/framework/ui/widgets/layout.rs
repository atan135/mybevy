#![allow(dead_code)]

use bevy::prelude::*;

use crate::framework::ui::{
    core::{UiMetrics, UiWidthClass},
    style::UiTheme,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiJustify {
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiAlign {
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiAlignSelf {
    Auto,
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiContentAlign {
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UiResponsiveGridColumns {
    pub compact: u16,
    pub medium: u16,
    pub expanded: u16,
}

impl UiResponsiveGridColumns {
    pub(crate) const fn new(compact: u16, medium: u16, expanded: u16) -> Self {
        Self {
            compact,
            medium,
            expanded,
        }
    }

    pub(crate) fn for_width_class(self, width_class: UiWidthClass) -> u16 {
        let columns = match width_class {
            UiWidthClass::Compact => self.compact,
            UiWidthClass::Medium => self.medium,
            UiWidthClass::Expanded => self.expanded,
        };

        columns.max(1)
    }
}

impl UiJustify {
    pub(crate) const fn to_justify_content(self) -> JustifyContent {
        match self {
            Self::Start => JustifyContent::FlexStart,
            Self::Center => JustifyContent::Center,
            Self::End => JustifyContent::FlexEnd,
            Self::SpaceBetween => JustifyContent::SpaceBetween,
            Self::SpaceAround => JustifyContent::SpaceAround,
        }
    }
}

impl UiAlign {
    pub(crate) const fn to_align_items(self) -> AlignItems {
        match self {
            Self::Start => AlignItems::FlexStart,
            Self::Center => AlignItems::Center,
            Self::End => AlignItems::FlexEnd,
            Self::Stretch => AlignItems::Stretch,
        }
    }

    pub(crate) const fn to_justify_items(self) -> JustifyItems {
        match self {
            Self::Start => JustifyItems::Start,
            Self::Center => JustifyItems::Center,
            Self::End => JustifyItems::End,
            Self::Stretch => JustifyItems::Stretch,
        }
    }
}

impl UiAlignSelf {
    pub(crate) const fn to_align_self(self) -> AlignSelf {
        match self {
            Self::Auto => AlignSelf::Auto,
            Self::Start => AlignSelf::FlexStart,
            Self::Center => AlignSelf::Center,
            Self::End => AlignSelf::FlexEnd,
            Self::Stretch => AlignSelf::Stretch,
        }
    }
}

impl UiContentAlign {
    pub(crate) const fn to_justify_content(self) -> JustifyContent {
        match self {
            Self::Start => JustifyContent::FlexStart,
            Self::Center => JustifyContent::Center,
            Self::End => JustifyContent::FlexEnd,
        }
    }
}

pub(crate) fn ui_column(gap: f32) -> impl Bundle {
    Node {
        width: percent(100),
        flex_direction: FlexDirection::Column,
        row_gap: px(gap),
        ..default()
    }
}

#[allow(dead_code)]
pub(crate) fn ui_row(theme: &UiTheme) -> impl Bundle {
    Node {
        width: percent(100),
        align_items: AlignItems::Center,
        column_gap: px(theme.layout.row_column_gap),
        row_gap: px(theme.layout.row_gap),
        ..default()
    }
}

#[allow(dead_code)]
pub(crate) fn ui_wrap_row(theme: &UiTheme) -> impl Bundle {
    Node {
        width: percent(100),
        align_items: AlignItems::Center,
        column_gap: px(theme.layout.row_column_gap),
        row_gap: px(theme.layout.row_gap),
        flex_wrap: FlexWrap::Wrap,
        ..default()
    }
}

pub(crate) fn ui_grid(theme: &UiTheme, columns: u16) -> impl Bundle {
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

pub(crate) fn ui_responsive_row(
    metrics: &UiMetrics,
    justify: UiJustify,
    align: UiAlign,
) -> impl Bundle {
    Node {
        width: percent(100),
        align_items: align.to_align_items(),
        justify_content: justify.to_justify_content(),
        column_gap: px(metrics.control_gap),
        row_gap: px(metrics.control_gap),
        ..default()
    }
}

pub(crate) fn ui_responsive_column(
    metrics: &UiMetrics,
    justify: UiJustify,
    align: UiAlign,
) -> impl Bundle {
    Node {
        width: percent(100),
        flex_direction: FlexDirection::Column,
        align_items: align.to_align_items(),
        justify_content: justify.to_justify_content(),
        row_gap: px(metrics.control_gap),
        ..default()
    }
}

pub(crate) fn ui_responsive_wrap_row(
    metrics: &UiMetrics,
    justify: UiJustify,
    align: UiAlign,
) -> impl Bundle {
    Node {
        width: percent(100),
        align_items: align.to_align_items(),
        justify_content: justify.to_justify_content(),
        column_gap: px(metrics.control_gap),
        row_gap: px(metrics.control_gap),
        flex_wrap: FlexWrap::Wrap,
        ..default()
    }
}

pub(crate) fn ui_responsive_grid(
    metrics: &UiMetrics,
    width_class: UiWidthClass,
    columns: UiResponsiveGridColumns,
) -> impl Bundle {
    responsive_grid_node(metrics, columns.for_width_class(width_class))
}

pub(crate) fn ui_content_container(metrics: &UiMetrics) -> impl Bundle {
    content_container_node(metrics)
}

pub(crate) fn ui_action_row(metrics: &UiMetrics, width_class: UiWidthClass) -> impl Bundle {
    let is_compact = width_class == UiWidthClass::Compact;

    Node {
        width: percent(100),
        align_items: AlignItems::Center,
        justify_content: if is_compact {
            JustifyContent::FlexStart
        } else {
            JustifyContent::FlexEnd
        },
        justify_items: if is_compact {
            JustifyItems::Stretch
        } else {
            JustifyItems::End
        },
        column_gap: px(metrics.control_gap),
        row_gap: px(metrics.control_gap),
        flex_wrap: FlexWrap::Wrap,
        ..default()
    }
}

pub(crate) fn ui_metrics_scroll_column(metrics: &UiMetrics) -> impl Bundle {
    Node {
        width: percent(100),
        flex_grow: 1.0,
        flex_direction: FlexDirection::Column,
        row_gap: px(metrics.section_gap),
        overflow: Overflow::scroll_y(),
        ..default()
    }
}

fn responsive_grid_node(metrics: &UiMetrics, columns: u16) -> Node {
    Node {
        width: percent(100),
        display: Display::Grid,
        grid_template_columns: RepeatedGridTrack::flex(columns.max(1), 1.0),
        grid_auto_rows: vec![GridTrack::auto()],
        column_gap: px(metrics.control_gap),
        row_gap: px(metrics.control_gap),
        align_items: AlignItems::Center,
        justify_items: JustifyItems::Stretch,
        ..default()
    }
}

fn content_container_node(metrics: &UiMetrics) -> Node {
    Node {
        width: percent(100),
        max_width: px(metrics.content_max_width),
        align_self: AlignSelf::Center,
        ..default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::ui::style::UiTheme;

    fn metrics() -> UiMetrics {
        UiMetrics::from_viewport_and_theme(&default(), &UiTheme::default())
    }

    #[test]
    fn responsive_grid_uses_compact_columns() {
        let metrics = metrics();
        let columns = UiResponsiveGridColumns::new(1, 2, 4);
        let node = responsive_grid_node(&metrics, columns.for_width_class(UiWidthClass::Compact));
        let expected: Vec<RepeatedGridTrack> = RepeatedGridTrack::flex(1, 1.0);

        assert_eq!(columns.for_width_class(UiWidthClass::Compact), 1);
        assert_eq!(node.grid_template_columns, expected);
    }

    #[test]
    fn responsive_grid_uses_expanded_columns() {
        let metrics = metrics();
        let columns = UiResponsiveGridColumns::new(1, 2, 4);
        let node = responsive_grid_node(&metrics, columns.for_width_class(UiWidthClass::Expanded));
        let expected: Vec<RepeatedGridTrack> = RepeatedGridTrack::flex(4, 1.0);

        assert_eq!(columns.for_width_class(UiWidthClass::Expanded), 4);
        assert_eq!(node.grid_template_columns, expected);
    }

    #[test]
    fn alignment_enums_convert_to_bevy_values() {
        assert_eq!(
            UiJustify::Start.to_justify_content(),
            JustifyContent::FlexStart
        );
        assert_eq!(
            UiJustify::SpaceBetween.to_justify_content(),
            JustifyContent::SpaceBetween
        );
        assert_eq!(UiAlign::Stretch.to_align_items(), AlignItems::Stretch);
        assert_eq!(UiAlign::End.to_justify_items(), JustifyItems::End);
        assert_eq!(UiAlignSelf::Center.to_align_self(), AlignSelf::Center);
        assert_eq!(
            UiContentAlign::End.to_justify_content(),
            JustifyContent::FlexEnd
        );
    }

    #[test]
    fn content_container_uses_metrics_max_width() {
        let metrics = metrics();
        let node = content_container_node(&metrics);

        assert_eq!(node.width, percent(100));
        assert_eq!(node.max_width, px(metrics.content_max_width));
        assert_eq!(node.align_self, AlignSelf::Center);
    }
}
