use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]
pub(in crate::game) enum UiImageFit {
    Stretch,
    Natural,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]
pub(in crate::game) enum UiImageSize {
    FixedBox { width: f32, height: f32 },
    PercentBox { width: f32, height: f32 },
    FullWidthAspect { aspect_ratio: f32 },
}

impl UiImageFit {
    pub(in crate::game) const fn to_node_image_mode(self) -> NodeImageMode {
        match self {
            Self::Natural => NodeImageMode::Auto,
            Self::Stretch => NodeImageMode::Stretch,
        }
    }

    pub(in crate::game) const fn to_align_self(self) -> AlignSelf {
        match self {
            Self::Natural => AlignSelf::Center,
            Self::Stretch => AlignSelf::Stretch,
        }
    }
}

impl UiImageSize {
    pub(in crate::game) fn to_node(self) -> Node {
        match self {
            Self::FixedBox { width, height } => Node {
                width: px(width),
                height: px(height),
                flex_shrink: 0.0,
                ..default()
            },
            Self::PercentBox { width, height } => Node {
                width: percent(width),
                height: percent(height),
                max_width: percent(100),
                ..default()
            },
            Self::FullWidthAspect { aspect_ratio } => Node {
                width: percent(100),
                max_width: percent(100),
                aspect_ratio: Some(aspect_ratio.max(0.01)),
                ..default()
            },
        }
    }
}

pub(in crate::game) fn ui_image(
    image: Handle<Image>,
    fit: UiImageFit,
    size: UiImageSize,
) -> impl Bundle {
    let mut node = size.to_node();
    node.align_self = fit.to_align_self();

    (
        node,
        ImageNode::new(image).with_mode(fit.to_node_image_mode()),
        Name::new("UI image"),
    )
}

pub(in crate::game) fn ui_image_panel_node(size: UiImageSize) -> impl Bundle {
    let mut node = size.to_node();
    node.overflow = Overflow::clip();
    node.align_items = AlignItems::Center;
    node.justify_content = JustifyContent::Center;

    node
}

pub(in crate::game) fn ui_thumbnail_grid(columns: u16, gap: f32) -> impl Bundle {
    Node {
        width: percent(100),
        display: Display::Grid,
        grid_template_columns: RepeatedGridTrack::flex(columns.max(1), 1.0),
        grid_auto_rows: vec![GridTrack::auto()],
        column_gap: px(gap),
        row_gap: px(gap),
        align_items: AlignItems::Center,
        justify_items: JustifyItems::Stretch,
        ..default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_fit_maps_to_node_image_mode() {
        assert_eq!(
            UiImageFit::Natural.to_node_image_mode(),
            NodeImageMode::Auto
        );
        assert_eq!(
            UiImageFit::Stretch.to_node_image_mode(),
            NodeImageMode::Stretch
        );
    }

    #[test]
    fn image_size_maps_to_node_dimensions() {
        let fixed = UiImageSize::FixedBox {
            width: 64.0,
            height: 48.0,
        }
        .to_node();
        assert_eq!(fixed.width, px(64));
        assert_eq!(fixed.height, px(48));

        let percent_box = UiImageSize::PercentBox {
            width: 50.0,
            height: 100.0,
        }
        .to_node();
        assert_eq!(percent_box.width, percent(50));
        assert_eq!(percent_box.height, percent(100));
        assert_eq!(percent_box.max_width, percent(100));

        let aspect = UiImageSize::FullWidthAspect { aspect_ratio: 1.5 }.to_node();
        assert_eq!(aspect.width, percent(100));
        assert_eq!(aspect.max_width, percent(100));
        assert_eq!(aspect.aspect_ratio, Some(1.5));
    }
}
