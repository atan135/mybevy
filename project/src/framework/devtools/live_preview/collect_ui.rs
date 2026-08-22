use std::collections::BTreeMap;

use bevy::{ecs::system::SystemParam, prelude::*};

use crate::framework::ui::{
    core::{
        UiCurrentOwner, UiCurrentScreen, UiInputState, UiMetrics, UiPanelKind, UiPanelRoot,
        UiPanelStack, UiViewport, focus::UiFocusState, stats::UiStats,
    },
    document::{
        UiDocumentBuildState, UiDocumentNodeAuditMarker, UiDocumentRuntimeEvent,
        UiDocumentRuntimeRoot,
    },
};

use super::model::UiPanelKindPreviewCounts;
use super::{
    LivePreviewClock, LivePreviewCollectionBuffer, LivePreviewScheduler, LivePreviewSnapshotHub,
    LivePreviewTimeline, LivePreviewTimelineEvent, LivePreviewTimelineSeverity,
    LivePreviewTimelineType, PreviewSection, StablePreviewId, UiPanelPreview, UiPreviewState,
};

#[derive(Debug, Default, Resource)]
pub(crate) struct UiPreviewCollectorState {
    last_state: Option<UiPreviewState>,
    revision: u64,
    document_error: Option<String>,
}

#[derive(SystemParam)]
pub(crate) struct UiPreviewInputs<'w> {
    current_owner: Option<Res<'w, UiCurrentOwner>>,
    current_screen: Option<Res<'w, UiCurrentScreen>>,
    input_state: Option<Res<'w, UiInputState>>,
    focus_state: Option<Res<'w, UiFocusState>>,
    stats: Option<Res<'w, UiStats>>,
    viewport: Option<Res<'w, UiViewport>>,
    metrics: Option<Res<'w, UiMetrics>>,
    panel_stack: Option<Res<'w, UiPanelStack>>,
}

/// Collects the framework-owned UI facts after the normal UI update systems.
/// The collector intentionally does not inspect binding values or text input
/// contents; those remain outside the preview allowlist.
pub(crate) fn collect_ui_preview(
    clock: Res<LivePreviewClock>,
    mut scheduler: ResMut<LivePreviewScheduler>,
    inputs: UiPreviewInputs,
    panels: Query<(
        &UiPanelRoot,
        Option<&Visibility>,
        Option<&InheritedVisibility>,
        Option<&ZIndex>,
    )>,
    focus_nodes: Query<(Entity, Option<&UiDocumentNodeAuditMarker>, Option<&Name>)>,
    document_roots: Query<&UiDocumentRuntimeRoot>,
    mut document_events: Option<MessageReader<UiDocumentRuntimeEvent>>,
    mut collector_state: ResMut<UiPreviewCollectorState>,
    mut buffer: ResMut<LivePreviewCollectionBuffer>,
    hub: Res<LivePreviewSnapshotHub>,
    mut timeline: ResMut<LivePreviewTimeline>,
) {
    if let Some(events) = document_events.as_mut() {
        for event in events.read() {
            let record = &event.0;
            match record.state {
                UiDocumentBuildState::Failed => {
                    collector_state.document_error = record
                        .failure_code
                        .clone()
                        .or_else(|| Some("document_failed".to_owned()));
                }
                UiDocumentBuildState::Committed | UiDocumentBuildState::Ready => {
                    collector_state.document_error = None;
                }
                _ => {}
            }
        }
    }

    let statistics_due = scheduler.statistics_sample_due(clock.monotonic_ms());
    if statistics_due {
        scheduler.record_statistics_sample(clock.monotonic_ms());
    }

    let next_state = collect_ui_state(
        inputs.current_owner.as_deref(),
        inputs.current_screen.as_deref(),
        inputs.input_state.as_deref(),
        inputs.focus_state.as_deref(),
        inputs.stats.as_deref(),
        inputs.viewport.as_deref(),
        inputs.metrics.as_deref(),
        inputs.panel_stack.as_deref(),
        &panels,
        &focus_nodes,
        &document_roots,
        statistics_due,
        collector_state.last_state.as_ref(),
        collector_state.document_error.as_deref(),
    );

    let changed = collector_state.last_state.as_ref() != Some(&next_state);
    if !changed && collector_state.revision != 0 {
        return;
    }

    let previous = collector_state.last_state.replace(next_state.clone());
    collector_state.revision = collector_state.revision.saturating_add(1).max(1);
    buffer.set_ui(PreviewSection::available(
        collector_state.revision,
        next_state.clone(),
    ));

    if previous
        .as_ref()
        .is_some_and(|previous| ui_structure_changed(previous, &next_state))
    {
        timeline.push(LivePreviewTimelineEvent::new(
            LivePreviewTimelineType::Ui,
            LivePreviewTimelineSeverity::Info,
            clock.monotonic_ms(),
            next_publish_sequence(hub.read().sequence),
            "ui structure changed",
            next_state.canonical_screen.clone(),
        ));
    }
}

fn next_publish_sequence(current_sequence: u64) -> u64 {
    current_sequence.saturating_add(1)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn collect_ui_state(
    current_owner: Option<&UiCurrentOwner>,
    current_screen: Option<&UiCurrentScreen>,
    input_state: Option<&UiInputState>,
    focus_state: Option<&UiFocusState>,
    stats: Option<&UiStats>,
    viewport: Option<&UiViewport>,
    metrics: Option<&UiMetrics>,
    panel_stack: Option<&UiPanelStack>,
    panels: &Query<(
        &UiPanelRoot,
        Option<&Visibility>,
        Option<&InheritedVisibility>,
        Option<&ZIndex>,
    )>,
    focus_nodes: &Query<(Entity, Option<&UiDocumentNodeAuditMarker>, Option<&Name>)>,
    document_roots: &Query<&UiDocumentRuntimeRoot>,
    statistics_due: bool,
    previous: Option<&UiPreviewState>,
    document_error: Option<&str>,
) -> UiPreviewState {
    let owner = current_owner
        .and_then(|current| current.owner)
        .map(|owner| owner.as_str().to_owned());
    let canonical_screen = current_screen
        .and_then(UiCurrentScreen::canonical_screen)
        .map(str::to_owned)
        .or_else(|| owner.clone());
    let panel_stack_indices = panel_stack.map(|stack| {
        stack
            .ordered_entries()
            .enumerate()
            .map(|(index, (id, _))| (id.as_str().to_owned(), index as u32))
            .collect::<BTreeMap<_, _>>()
    });
    let mut panel_summaries = panels
        .iter()
        .map(
            |(panel, visibility, inherited_visibility, z_index)| UiPanelPreview {
                id: StablePreviewId::new(panel.id.as_str()),
                kind: Some(panel_kind_label(panel.kind).to_owned()),
                owner: panel.owner.map(|owner| owner.as_str().to_owned()),
                z_index: z_index.map(|z_index| z_index.0),
                active: Some(is_visible(visibility, inherited_visibility)),
                stack_index: panel_stack_indices
                    .as_ref()
                    .and_then(|indices| indices.get(panel.id.as_str()).copied()),
            },
        )
        .collect::<Vec<_>>();
    panel_summaries.sort_by(|left, right| left.id.cmp(&right.id));

    let focus_panel_id = input_state
        .and_then(|input| input.focused_panel)
        .map(|panel| StablePreviewId::new(panel.as_str()));
    let focus_node_id = focus_state
        .and_then(|focus| focus.focused_entity)
        .and_then(|entity| focus_nodes.get(entity).ok())
        .and_then(|(_, marker, name)| {
            marker
                .map(|marker| StablePreviewId::new(marker.node_id.as_str()))
                .or_else(|| name.map(|name| StablePreviewId::new(name.as_str())))
        });

    let (document_id, document_schema_version, document_source) = document_roots
        .iter()
        .map(|root| {
            (
                StablePreviewId::new(root.document_id.as_str()),
                u16::try_from(root.schema_version).ok(),
                root.origin.audit_source_path(),
            )
        })
        .min_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then(left.1.cmp(&right.1))
                .then(left.2.cmp(&right.2))
        })
        .map_or((None, None, None), |(id, schema, source)| {
            (Some(id), schema, Some(source))
        });
    let document_status = if document_id.is_some() {
        Some("ready".to_owned())
    } else if document_error.is_some() {
        Some("failed".to_owned())
    } else {
        None
    };

    let (ui_node_count, visible_ui_node_count, text_node_count, panel_count, panel_kind_counts) =
        if statistics_due {
            if let Some(stats) = stats {
                (
                    Some(stats.ui_node_count as u64),
                    Some(stats.visible_ui_node_count as u64),
                    Some(stats.text_node_count as u64),
                    Some(stats.panel_count as u64),
                    Some(UiPanelKindPreviewCounts {
                        page: stats.panel_kind_counts.page as u64,
                        hud: stats.panel_kind_counts.hud as u64,
                        floating: stats.panel_kind_counts.floating as u64,
                        modal: stats.panel_kind_counts.modal as u64,
                        blocking_overlay: stats.panel_kind_counts.blocking_overlay as u64,
                    }),
                )
            } else {
                (None, None, None, None, None)
            }
        } else {
            previous.map_or((None, None, None, None, None), |previous| {
                (
                    previous.ui_node_count,
                    previous.visible_ui_node_count,
                    previous.text_node_count,
                    previous.panel_count,
                    previous.panel_kind_counts.clone(),
                )
            })
        };

    UiPreviewState {
        canonical_screen: canonical_screen.clone(),
        screen_id: canonical_screen.map(StablePreviewId::new),
        owner,
        panels: panel_summaries,
        pointer_blocked: input_state.map(|input| input.pointer_blocked),
        block_reason: input_state.map(|input| input.pointer_block_reason.clone()),
        route_summary: input_state.map(|input| input.route_summary.clone()),
        blocking_reason: input_state.and_then(|input| {
            input
                .top_blocking_panel
                .map(|panel| panel.as_str().to_owned())
        }),
        focus_panel_id,
        focus_node_id,
        ui_node_count,
        visible_ui_node_count,
        text_node_count,
        panel_count,
        panel_kind_counts,
        document_id,
        document_schema_version,
        document_status,
        document_source,
        document_error: document_error.map(str::to_owned),
        viewport: viewport.map(viewport_preview),
        metrics: metrics.map(metrics_preview),
    }
}

fn viewport_preview(viewport: &UiViewport) -> super::UiViewportPreview {
    super::UiViewportPreview {
        logical_width: viewport.logical_width,
        logical_height: viewport.logical_height,
        device_scale: viewport.device_scale,
        preview_scale: viewport.preview_scale,
        width_class: format!("{:?}", viewport.width_class).to_ascii_lowercase(),
        height_class: format!("{:?}", viewport.height_class).to_ascii_lowercase(),
        orientation: format!("{:?}", viewport.orientation).to_ascii_lowercase(),
        input_mode: format!("{:?}", viewport.input_mode).to_ascii_lowercase(),
        safe_area: [
            viewport.safe_area.left,
            viewport.safe_area.right,
            viewport.safe_area.top,
            viewport.safe_area.bottom,
        ],
    }
}

fn metrics_preview(metrics: &UiMetrics) -> super::UiMetricsPreview {
    super::UiMetricsPreview {
        page_padding: metrics.page_padding,
        panel_padding: metrics.panel_padding,
        control_gap: metrics.control_gap,
        section_gap: metrics.section_gap,
        touch_target_min: metrics.touch_target_min,
        font_body: metrics.font_body,
        content_max_width: metrics.content_max_width,
    }
}

fn is_visible(
    visibility: Option<&Visibility>,
    inherited_visibility: Option<&InheritedVisibility>,
) -> bool {
    visibility.is_none_or(|visibility| *visibility != Visibility::Hidden)
        && inherited_visibility.is_none_or(|visibility| visibility.get())
}

fn panel_kind_label(kind: UiPanelKind) -> &'static str {
    match kind {
        UiPanelKind::Page => "page",
        UiPanelKind::Hud => "hud",
        UiPanelKind::Floating => "floating",
        UiPanelKind::Modal => "modal",
        UiPanelKind::BlockingOverlay => "blocking_overlay",
    }
}

fn ui_structure_changed(previous: &UiPreviewState, next: &UiPreviewState) -> bool {
    previous.canonical_screen != next.canonical_screen
        || previous.owner != next.owner
        || previous.panels != next.panels
        || previous.pointer_blocked != next.pointer_blocked
        || previous.block_reason != next.block_reason
        || previous.blocking_reason != next.blocking_reason
        || previous.focus_panel_id != next.focus_panel_id
        || previous.focus_node_id != next.focus_node_id
        || previous.document_id != next.document_id
        || previous.document_schema_version != next.document_schema_version
        || previous.document_status != next.document_status
        || previous.document_source != next.document_source
        || previous.document_error != next.document_error
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::ui::{
        core::{UiOwnerId, UiPanelId, stats::UiStats},
        document::{
            UiDocumentId, UiDocumentInstanceId, UiDocumentLayer, UiDocumentPanel,
            UiDocumentRequestId, UiDocumentSourceOrigin, UiNodeId,
        },
    };
    use bevy::ecs::system::SystemState;
    use std::str::FromStr;

    type PanelQuery = Query<
        'static,
        'static,
        (
            &'static UiPanelRoot,
            Option<&'static Visibility>,
            Option<&'static InheritedVisibility>,
            Option<&'static ZIndex>,
        ),
    >;
    type FocusNodeQuery = Query<
        'static,
        'static,
        (
            Entity,
            Option<&'static UiDocumentNodeAuditMarker>,
            Option<&'static Name>,
        ),
    >;

    fn collect_from_world(
        world: &mut World,
        statistics_due: bool,
        document_error: Option<&str>,
    ) -> UiPreviewState {
        let mut state = SystemState::<(
            Option<Res<UiCurrentOwner>>,
            Option<Res<UiCurrentScreen>>,
            Option<Res<UiInputState>>,
            Option<Res<UiFocusState>>,
            Option<Res<UiStats>>,
            Option<Res<UiViewport>>,
            Option<Res<UiMetrics>>,
            Option<Res<UiPanelStack>>,
            PanelQuery,
            FocusNodeQuery,
            Query<'static, 'static, &'static UiDocumentRuntimeRoot>,
        )>::new(world);
        let (
            owner,
            screen,
            input,
            focus,
            stats,
            viewport,
            metrics,
            stack,
            panels,
            focus_nodes,
            document_roots,
        ) = state.get(world);

        collect_ui_state(
            owner.as_deref(),
            screen.as_deref(),
            input.as_deref(),
            focus.as_deref(),
            stats.as_deref(),
            viewport.as_deref(),
            metrics.as_deref(),
            stack.as_deref(),
            &panels,
            &focus_nodes,
            &document_roots,
            statistics_due,
            None,
            document_error,
        )
    }

    #[test]
    fn no_page_is_explicitly_available_with_missing_fields() {
        let mut world = World::new();
        let state = collect_from_world(&mut world, false, None);
        assert!(state.canonical_screen.is_none());
        assert!(state.panels.is_empty());
        assert!(state.document_id.is_none());
    }

    #[test]
    fn owner_and_page_switch_updates_canonical_screen() {
        let mut world = World::new();
        world.insert_resource(UiCurrentOwner {
            owner: Some(UiOwnerId::new("screen.home")),
        });
        world.spawn(UiPanelRoot {
            id: UiPanelId::new("home.page"),
            kind: UiPanelKind::Page,
            owner: Some(UiOwnerId::new("screen.home")),
        });

        let first = collect_from_world(&mut world, false, None);
        assert_eq!(first.canonical_screen.as_deref(), Some("screen.home"));
        assert_eq!(
            first.screen_id.as_ref().map(StablePreviewId::as_str),
            Some("screen.home")
        );

        world.resource_mut::<UiCurrentOwner>().owner = Some(UiOwnerId::new("screen.lobby"));
        let second = collect_from_world(&mut world, false, None);
        assert_eq!(second.canonical_screen.as_deref(), Some("screen.lobby"));
        assert_ne!(first.screen_id, second.screen_id);
    }

    #[test]
    fn canonical_screen_resource_is_independent_from_owner() {
        let mut world = World::new();
        world.insert_resource(UiCurrentOwner {
            owner: Some(UiOwnerId::new("owner.audio_settings")),
        });
        let mut screen = UiCurrentScreen::default();
        screen.set("audio_settings");
        world.insert_resource(screen);

        let state = collect_from_world(&mut world, false, None);
        assert_eq!(state.owner.as_deref(), Some("owner.audio_settings"));
        assert_eq!(state.canonical_screen.as_deref(), Some("audio_settings"));
    }

    #[test]
    fn panel_summary_preserves_kind_visibility_and_z_order() {
        let mut world = World::new();
        world.spawn((
            UiPanelRoot {
                id: UiPanelId::new("page.main"),
                kind: UiPanelKind::Page,
                owner: None,
            },
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
            ZIndex(1),
        ));
        world.spawn((
            UiPanelRoot {
                id: UiPanelId::new("modal.hidden"),
                kind: UiPanelKind::Modal,
                owner: None,
            },
            Visibility::Hidden,
            InheritedVisibility::HIDDEN,
            ZIndex(20),
        ));
        world.spawn((
            UiPanelRoot {
                id: UiPanelId::new("overlay.blocking"),
                kind: UiPanelKind::BlockingOverlay,
                owner: None,
            },
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
            ZIndex(100),
        ));

        let state = collect_from_world(&mut world, false, None);
        assert_eq!(state.panels.len(), 3);
        let hidden = state
            .panels
            .iter()
            .find(|panel| panel.id.as_str() == "modal.hidden")
            .expect("hidden panel summary");
        assert_eq!(hidden.kind.as_deref(), Some("modal"));
        assert_eq!(hidden.active, Some(false));
        assert_eq!(hidden.z_index, Some(20));
        let blocking = state
            .panels
            .iter()
            .find(|panel| panel.id.as_str() == "overlay.blocking")
            .expect("blocking panel summary");
        assert_eq!(blocking.kind.as_deref(), Some("blocking_overlay"));
        assert_eq!(blocking.active, Some(true));
        assert_eq!(blocking.z_index, Some(100));
    }

    #[test]
    fn input_and_focus_state_expose_only_stable_ids() {
        let mut world = World::new();
        let panel_id = UiPanelId::new("modal.confirm");
        let node_id = UiNodeId::from_str("screen.confirm").unwrap();
        let focused_entity = world
            .spawn(UiDocumentNodeAuditMarker {
                instance_id: UiDocumentInstanceId(7),
                document_id: UiDocumentId::from_str("screen.home").unwrap(),
                schema_version: 3,
                node_id: node_id.clone(),
                document_path: "screen.home".to_owned(),
                source_path: "fixture:home".to_owned(),
            })
            .id();
        let mut input = UiInputState::default();
        input.pointer_blocked = true;
        input.focused_panel = Some(panel_id);
        input.top_blocking_panel = Some(panel_id);
        input.pointer_block_reason = "modal".to_owned();
        input.route_summary = "modal.confirm".to_owned();
        world.insert_resource(input);
        let mut focus = UiFocusState::default();
        focus.focused_entity = Some(focused_entity);
        world.insert_resource(focus);

        let state = collect_from_world(&mut world, false, None);
        assert_eq!(state.pointer_blocked, Some(true));
        assert_eq!(state.block_reason.as_deref(), Some("modal"));
        assert_eq!(state.blocking_reason.as_deref(), Some("modal.confirm"));
        assert_eq!(
            state.focus_panel_id.as_ref().map(StablePreviewId::as_str),
            Some("modal.confirm")
        );
        assert_eq!(
            state.focus_node_id.as_ref().map(StablePreviewId::as_str),
            Some("screen.confirm")
        );
    }

    #[test]
    fn document_ready_and_failure_are_explicit() {
        let mut world = World::new();
        world.spawn(UiDocumentRuntimeRoot {
            request_id: UiDocumentRequestId(1),
            instance_id: UiDocumentInstanceId(1),
            generation: 1,
            document_id: UiDocumentId::from_str("screen.home").unwrap(),
            schema_version: 3,
            owner: "screen.home".to_owned(),
            panel: UiDocumentPanel::Page,
            layer: UiDocumentLayer::Page,
            origin: UiDocumentSourceOrigin::Fixture {
                fixture_id: "home".to_owned(),
            },
        });
        let ready = collect_from_world(&mut world, false, None);
        assert_eq!(ready.document_status.as_deref(), Some("ready"));
        assert_eq!(ready.document_source.as_deref(), Some("fixture:home"));

        let mut failed_world = World::new();
        let failed = collect_from_world(&mut failed_world, false, Some("validation_failed"));
        assert_eq!(failed.document_status.as_deref(), Some("failed"));
        assert_eq!(failed.document_error.as_deref(), Some("validation_failed"));
    }

    #[test]
    fn panel_kind_labels_are_stable() {
        assert_eq!(panel_kind_label(UiPanelKind::Page), "page");
        assert_eq!(
            panel_kind_label(UiPanelKind::BlockingOverlay),
            "blocking_overlay"
        );
    }

    #[test]
    fn structure_signature_ignores_high_frequency_route_summary() {
        let mut previous = UiPreviewState {
            canonical_screen: Some("login".to_owned()),
            ..Default::default()
        };
        let mut next = previous.clone();
        previous.route_summary = Some("hovered button".to_owned());
        next.route_summary = Some("pressed button".to_owned());
        assert!(!ui_structure_changed(&previous, &next));
    }

    #[test]
    fn timeline_sequence_targets_the_next_dirty_snapshot_publish() {
        assert_eq!(next_publish_sequence(0), 1);
        assert_eq!(next_publish_sequence(41), 42);
        assert_eq!(next_publish_sequence(u64::MAX), u64::MAX);
    }
}
