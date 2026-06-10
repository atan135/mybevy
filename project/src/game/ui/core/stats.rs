use bevy::prelude::*;

use crate::game::ui::core::{UiPanelKind, UiPanelRoot};

pub(in crate::game) struct UiStatsPlugin;

impl Plugin for UiStatsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiStats>()
            .configure_sets(Update, UiStatsSystems::Collect)
            .add_systems(Update, collect_ui_stats.in_set(UiStatsSystems::Collect));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, SystemSet)]
pub(in crate::game) enum UiStatsSystems {
    Collect,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Resource)]
pub(in crate::game) struct UiStats {
    pub ui_node_count: usize,
    pub visible_ui_node_count: usize,
    pub panel_count: usize,
    pub panel_kind_counts: UiPanelKindCounts,
    pub text_node_count: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::game) struct UiPanelKindCounts {
    pub page: usize,
    pub hud: usize,
    pub floating: usize,
    pub modal: usize,
    pub blocking_overlay: usize,
}

impl UiPanelKindCounts {
    pub(in crate::game) fn add(&mut self, kind: UiPanelKind) {
        match kind {
            UiPanelKind::Page => self.page += 1,
            UiPanelKind::Hud => self.hud += 1,
            UiPanelKind::Floating => self.floating += 1,
            UiPanelKind::Modal => self.modal += 1,
            UiPanelKind::BlockingOverlay => self.blocking_overlay += 1,
        }
    }

    #[cfg(test)]
    fn count(self, kind: UiPanelKind) -> usize {
        match kind {
            UiPanelKind::Page => self.page,
            UiPanelKind::Hud => self.hud,
            UiPanelKind::Floating => self.floating,
            UiPanelKind::Modal => self.modal,
            UiPanelKind::BlockingOverlay => self.blocking_overlay,
        }
    }
}

fn collect_ui_stats(
    mut stats: ResMut<UiStats>,
    ui_nodes: Query<(Option<&Visibility>, Option<&InheritedVisibility>), With<Node>>,
    panels: Query<&UiPanelRoot>,
    text_nodes: Query<(), With<Text>>,
) {
    let mut next_stats = UiStats {
        text_node_count: text_nodes.iter().count(),
        ..default()
    };

    for (visibility, inherited_visibility) in &ui_nodes {
        next_stats.ui_node_count += 1;
        if is_ui_node_visible(visibility, inherited_visibility) {
            next_stats.visible_ui_node_count += 1;
        }
    }

    for panel in &panels {
        next_stats.panel_count += 1;
        next_stats.panel_kind_counts.add(panel.kind);
    }

    if *stats != next_stats {
        *stats = next_stats;
    }
}

pub(in crate::game) fn is_ui_node_visible(
    visibility: Option<&Visibility>,
    inherited_visibility: Option<&InheritedVisibility>,
) -> bool {
    visibility.is_none_or(|visibility| *visibility != Visibility::Hidden)
        && inherited_visibility.is_none_or(|visibility| visibility.get())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_kind_counts_aggregate_by_kind() {
        let mut counts = UiPanelKindCounts::default();

        counts.add(UiPanelKind::Page);
        counts.add(UiPanelKind::Hud);
        counts.add(UiPanelKind::Hud);
        counts.add(UiPanelKind::Floating);
        counts.add(UiPanelKind::Modal);
        counts.add(UiPanelKind::Modal);
        counts.add(UiPanelKind::BlockingOverlay);

        assert_eq!(counts.count(UiPanelKind::Page), 1);
        assert_eq!(counts.count(UiPanelKind::Hud), 2);
        assert_eq!(counts.count(UiPanelKind::Floating), 1);
        assert_eq!(counts.count(UiPanelKind::Modal), 2);
        assert_eq!(counts.count(UiPanelKind::BlockingOverlay), 1);
    }

    #[test]
    fn ui_node_visibility_treats_missing_components_as_visible() {
        assert!(is_ui_node_visible(None, None));
        assert!(is_ui_node_visible(
            Some(&Visibility::Visible),
            Some(&InheritedVisibility::VISIBLE),
        ));
    }

    #[test]
    fn ui_node_visibility_rejects_hidden_local_or_inherited_state() {
        assert!(!is_ui_node_visible(Some(&Visibility::Hidden), None));
        assert!(!is_ui_node_visible(
            Some(&Visibility::Visible),
            Some(&InheritedVisibility::HIDDEN),
        ));
    }
}
