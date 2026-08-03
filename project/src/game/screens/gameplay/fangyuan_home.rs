use bevy::{
    ecs::message::{MessageCursor, Messages},
    prelude::*,
};

use crate::framework::{
    fangyuan::{
        FANGYUAN_HOME_PREFAB_PALETTE_PATH, FANGYUAN_HOME_SCENE_LAYOUT_PATH,
        FangyuanAoiDebugMetrics, FangyuanAuditDebugMetrics, FangyuanBakeDebugMetrics,
        FangyuanCacheDebugMetrics, FangyuanChunkDebugSummary, FangyuanChunkRuntime,
        FangyuanDebugMetricModule, FangyuanDebugMetricsSnapshot, FangyuanDebugModuleStatus,
        FangyuanDebugPanelState, FangyuanLodDebugMetrics, FangyuanPressureDebugMetrics,
        FangyuanPrimitiveDebugMetrics, FangyuanRenderDebugMetrics, fangyuan_debug_panel_snapshot,
    },
    scene::prelude::SceneEvent,
    ui::{
        core::{UiViewport, UiWidthClass, binding::UiBindingValues},
        document::{UiBindingValue, UiBindingVisibility},
    },
};
use crate::game::{
    navigation::{AppUiMode, GameRouteCommand},
    scenes::{FANGYUAN_HOME_SCENE_ID, FangyuanHomeBlueprintStats},
    ui_ids::OWNER_FANGYUAN_HOME,
};

use super::host::{FANGYUAN_HOME_DOCUMENT_ID, GameplayHudHostContract, set_binding};

const FANGYUAN_HOME_PRIMITIVE_LIMIT: usize = 1000;

fn fangyuan_home_debug_panel_is_compact(viewport: &UiViewport) -> bool {
    viewport.width_class == UiWidthClass::Compact || viewport.logical_height < 700.0
}

pub(super) fn update_fangyuan_home_hud_status(
    stats: Res<FangyuanHomeBlueprintStats>,
    chunk_runtime: Option<Res<FangyuanChunkRuntime>>,
    contract: Res<GameplayHudHostContract>,
    mut values: ResMut<UiBindingValues>,
) {
    let chunk_summary = chunk_runtime
        .as_deref()
        .map(FangyuanChunkRuntime::debug_summary)
        .unwrap_or_default();
    let status = fangyuan_home_hud_status_text(Some(&stats), Some(&chunk_summary));
    set_binding(
        &contract,
        &mut values,
        FANGYUAN_HOME_DOCUMENT_ID,
        OWNER_FANGYUAN_HOME.as_str(),
        "fangyuan_home.status",
        UiBindingValue::String(status),
    );
}

pub(super) fn update_fangyuan_home_debug_panel(
    stats: Res<FangyuanHomeBlueprintStats>,
    chunk_runtime: Option<Res<FangyuanChunkRuntime>>,
    viewport: Res<UiViewport>,
    mut debug_panel_state: ResMut<FangyuanDebugPanelState>,
    contract: Res<GameplayHudHostContract>,
    mut values: ResMut<UiBindingValues>,
) {
    let compact = fangyuan_home_debug_panel_is_compact(&viewport);
    debug_panel_state.set_compact(compact);
    set_binding(
        &contract,
        &mut values,
        FANGYUAN_HOME_DOCUMENT_ID,
        OWNER_FANGYUAN_HOME.as_str(),
        "fangyuan_home.debug.visibility",
        UiBindingValue::Visibility(if debug_panel_state.visible {
            UiBindingVisibility::Visible
        } else {
            UiBindingVisibility::Hidden
        }),
    );

    let chunk_summary = chunk_runtime
        .as_deref()
        .map(FangyuanChunkRuntime::debug_summary)
        .unwrap_or_default();
    let panel_text =
        fangyuan_home_debug_panel_text(Some(&stats), Some(&chunk_summary), &debug_panel_state);
    set_binding(
        &contract,
        &mut values,
        FANGYUAN_HOME_DOCUMENT_ID,
        OWNER_FANGYUAN_HOME.as_str(),
        "fangyuan_home.debug.text",
        UiBindingValue::String(panel_text),
    );
}

fn fangyuan_home_debug_panel_text(
    stats: Option<&FangyuanHomeBlueprintStats>,
    chunk_summary: Option<&FangyuanChunkDebugSummary>,
    state: &FangyuanDebugPanelState,
) -> String {
    let default_stats = FangyuanHomeBlueprintStats::default();
    let default_chunk_summary = FangyuanChunkDebugSummary::default();
    let stats = stats.unwrap_or(&default_stats);
    let chunk_summary = chunk_summary.unwrap_or(&default_chunk_summary);
    let snapshot = fangyuan_home_debug_metrics_snapshot(stats, chunk_summary);
    fangyuan_debug_panel_snapshot(&snapshot, state.toggles, state.compact).text()
}

fn fangyuan_home_debug_metrics_snapshot(
    stats: &FangyuanHomeBlueprintStats,
    chunk_summary: &FangyuanChunkDebugSummary,
) -> FangyuanDebugMetricsSnapshot {
    let mut snapshot = FangyuanDebugMetricsSnapshot {
        primitive: FangyuanPrimitiveDebugMetrics::from_stats(&stats.primitive_stats),
        render: FangyuanRenderDebugMetrics {
            render_mode: stats.render_mode.clone(),
            instance_count: stats.static_instance_count,
            batch_count: stats.static_instance_batch_count,
            mesh_count: fangyuan_home_debug_mesh_count(stats),
            buffer_bytes: stats.static_instance_buffer_bytes,
            buffer_update_bytes: stats.static_instance_buffer_bytes,
            draw_estimate: fangyuan_home_debug_draw_estimate(stats),
            material_profile_count: stats.material_profile_count,
            pressure_units: stats
                .static_instance_batch_count
                .max(stats.static_instance_buffer_bytes.div_ceil(1024)),
            limiting_path: compact_fangyuan_home_fallback_reason(
                &stats.static_instance_fallback_reason,
            ),
        },
        lod: fangyuan_home_debug_lod_metrics(stats),
        aoi: FangyuanAoiDebugMetrics {
            keep_chunks: chunk_summary.loaded_chunks,
            visible_objects: chunk_summary.visible_objects,
            radius: stats.lod_aoi_radius,
            ..Default::default()
        },
        pressure: FangyuanPressureDebugMetrics {
            active: stats.lod_pressure != "normal",
            severity: stats.lod_pressure.clone(),
            reason_count: usize::from(stats.lod_degrade_reason != "-"),
            pressure_units: usize::from(stats.lod_pressure != "normal"),
            degrade_reason: stats.lod_degrade_reason.clone(),
        },
        cache: FangyuanCacheDebugMetrics::default(),
        bake: FangyuanBakeDebugMetrics::default(),
        audit: FangyuanAuditDebugMetrics {
            status: stats.audit_status_label().to_string(),
            error_count: stats.audit_error_count,
            warning_count: stats.audit_warning_count,
            finding_count: stats.audit_error_count + stats.audit_warning_count,
        },
        trial: crate::framework::fangyuan::FangyuanTrialDebugMetrics {
            route_id: stats.trial_route_id.clone(),
            budget_profile: stats.trial_budget_profile.clone(),
            audit_status: stats.trial_audit_status.clone(),
            active_vfx_count: stats.active_vfx_count,
            budget_cost: stats.trial_budget_cost,
            budget_recommended: stats.trial_budget_recommended,
            budget_hard: stats.trial_budget_hard,
            kept_count: stats.trial_kept_count,
            degraded_count: stats.trial_degraded_count,
            rejected_count: stats.trial_rejected_count,
            fallback_missing_count: stats.trial_fallback_missing_count,
            reason_summary: stats.trial_plain_reason_summary.clone(),
        },
        ..Default::default()
    };

    for module in [
        FangyuanDebugMetricModule::Primitive,
        FangyuanDebugMetricModule::Render,
        FangyuanDebugMetricModule::Lod,
        FangyuanDebugMetricModule::Aoi,
        FangyuanDebugMetricModule::Pressure,
        FangyuanDebugMetricModule::Audit,
        FangyuanDebugMetricModule::Trial,
    ] {
        snapshot
            .module_status
            .insert(module.as_str(), FangyuanDebugModuleStatus::Present);
    }
    snapshot
}

fn fangyuan_home_debug_mesh_count(stats: &FangyuanHomeBlueprintStats) -> usize {
    if stats.static_instance_batch_count > 0 {
        stats.static_instance_batch_count
    } else {
        stats.generated_primitives
    }
}

fn fangyuan_home_debug_draw_estimate(stats: &FangyuanHomeBlueprintStats) -> usize {
    if stats.static_instance_batch_count > 0 {
        stats.static_instance_batch_count
    } else {
        stats.generated_primitives
    }
}

fn fangyuan_home_debug_lod_metrics(stats: &FangyuanHomeBlueprintStats) -> FangyuanLodDebugMetrics {
    let counts = parse_fangyuan_home_lod_distribution(&stats.lod_distribution);
    FangyuanLodDebugMetrics {
        near_count: counts[0],
        mid_count: counts[1],
        far_count: counts[2],
        marker_count: counts[3],
        hidden_count: counts[4],
        dominant_lod: dominant_fangyuan_home_lod_label(counts).to_string(),
    }
}

fn parse_fangyuan_home_lod_distribution(label: &str) -> [usize; 5] {
    let mut counts = [0; 5];
    for part in label.split_whitespace() {
        if part.len() < 2 {
            continue;
        }
        let (prefix, value) = part.split_at(1);
        let Ok(value) = value.parse::<usize>() else {
            continue;
        };
        match prefix {
            "f" => counts[0] = value,
            "r" => counts[1] = value,
            "s" => counts[2] = value,
            "m" => counts[3] = value,
            "h" => counts[4] = value,
            _ => {}
        }
    }
    counts
}

fn dominant_fangyuan_home_lod_label(counts: [usize; 5]) -> &'static str {
    [
        ("full", counts[0]),
        ("reduced", counts[1]),
        ("silhouette", counts[2]),
        ("marker", counts[3]),
        ("hidden_rule_only", counts[4]),
    ]
    .into_iter()
    .max_by_key(|(_, count)| *count)
    .filter(|(_, count)| *count > 0)
    .map(|(label, _)| label)
    .unwrap_or("-")
}

fn fangyuan_home_hud_status_text(
    stats: Option<&FangyuanHomeBlueprintStats>,
    chunk_summary: Option<&FangyuanChunkDebugSummary>,
) -> String {
    let default_stats = FangyuanHomeBlueprintStats::default();
    let default_chunk_summary = FangyuanChunkDebugSummary::default();
    let stats = stats.unwrap_or(&default_stats);
    let chunk_summary = chunk_summary.unwrap_or(&default_chunk_summary);
    let state = stats.state_label();
    let layout_path =
        compact_fangyuan_home_layout_path(stats.layout_path(), FANGYUAN_HOME_SCENE_LAYOUT_PATH);
    let palette_path =
        compact_fangyuan_home_layout_path(stats.palette_path(), FANGYUAN_HOME_PREFAB_PALETTE_PATH);

    format!(
        "layout {state} gen {}/{} skip {}\naudit {} e{} w{} {}\npal {} pf {} used {} inst {} mat {}\nmatprof {} opaque {} trans {} emi {:.1} uniq {}\nrender {} ib {} ii {} bytes {} fb {}\nchunk {} obj {} state {} fail {} ids {}\nlod {} aoi {:.0} pressure {} degrade {}\npath {}\ntrial {} sel {} profile {} run {} status {} e{} w{} s{}\ntrial vfx {} tpl {} vis {}\neq {} npc {} td {} cost {}/{}/{}\ntrial before {} after {}\nresult k{} d{} r{} fb{} {} reason {} suggest {} find {}\nl {layout_path}\np {palette_path}",
        stats.generated_primitives,
        FANGYUAN_HOME_PRIMITIVE_LIMIT,
        stats.skipped,
        stats.audit_status_label(),
        stats.audit_error_count,
        stats.audit_warning_count,
        stats.audit_primary_code(),
        stats.palette_count,
        stats.prefab_count,
        stats.used_prefab_count,
        stats.instance_count,
        stats.materials,
        stats.material_profile_count,
        stats.opaque_count,
        stats.transparent_count,
        stats.emissive_total,
        stats.unique_material_resource_count,
        stats.render_mode,
        stats.static_instance_batch_count,
        stats.static_instance_count,
        stats.static_instance_buffer_bytes,
        compact_fangyuan_home_fallback_reason(&stats.static_instance_fallback_reason),
        chunk_summary.loaded_chunks,
        chunk_summary.visible_objects,
        compact_fangyuan_home_chunk_state(&chunk_summary.load_state),
        chunk_summary.failure_label(26),
        chunk_summary.loaded_ids_label(32),
        compact_fangyuan_home_lod_label(&stats.lod_distribution),
        stats.lod_aoi_radius,
        compact_fangyuan_home_lod_label(&stats.lod_pressure),
        compact_fangyuan_home_lod_label(&stats.lod_degrade_reason),
        compact_fangyuan_home_lod_path(&stats.lod_render_paths),
        stats.trial_route_id,
        compact_fangyuan_home_trial_label(&stats.trial_selection_label),
        compact_fangyuan_home_trial_id(&stats.trial_budget_profile),
        stats.trial_audit_run,
        stats.trial_audit_status,
        stats.trial_audit_error_count,
        stats.trial_audit_warning_count,
        stats.trial_audit_suggestion_count,
        stats.active_vfx_count,
        compact_fangyuan_home_trial_id(&stats.trial_template_id),
        compact_fangyuan_home_trial_id(&stats.trial_visual_id),
        stats.trial_equipment_count,
        stats.trial_npc_count,
        stats.trial_tiandao_count,
        stats.trial_budget_cost,
        stats.trial_budget_recommended,
        stats.trial_budget_hard,
        compact_fangyuan_home_trial_label(&stats.trial_before_label),
        compact_fangyuan_home_trial_label(&stats.trial_after_label),
        stats.trial_kept_count,
        stats.trial_degraded_count,
        stats.trial_rejected_count,
        stats.trial_fallback_missing_count,
        compact_fangyuan_home_finding_summary(&stats.trial_fallback_summary),
        compact_fangyuan_home_trial_label(&stats.trial_plain_reason_summary),
        compact_fangyuan_home_trial_id(&stats.trial_primary_suggestion),
        compact_fangyuan_home_finding_summary(&stats.trial_finding_summary),
    )
}

fn compact_fangyuan_home_chunk_state(state: &str) -> String {
    compact_fangyuan_home_text(state, "pending", 18)
}

fn compact_fangyuan_home_trial_id(id: &str) -> String {
    const MAX_ID_CHARS: usize = 22;
    compact_fangyuan_home_text(id, "-", MAX_ID_CHARS)
}

fn compact_fangyuan_home_trial_label(label: &str) -> String {
    const MAX_LABEL_CHARS: usize = 46;
    compact_fangyuan_home_text(label, "-", MAX_LABEL_CHARS)
}

fn compact_fangyuan_home_fallback_reason(reason: &str) -> String {
    compact_fangyuan_home_text(reason, "-", 22)
}

fn compact_fangyuan_home_finding_summary(summary: &str) -> String {
    compact_fangyuan_home_text(summary, "ok", 32)
}

fn compact_fangyuan_home_lod_label(label: &str) -> String {
    compact_fangyuan_home_text(label, "-", 28)
}

fn compact_fangyuan_home_lod_path(label: &str) -> String {
    compact_fangyuan_home_text(label, "-", 38)
}

fn compact_fangyuan_home_text(value: &str, fallback: &str, max_chars: usize) -> String {
    let value = value.trim();
    if value.is_empty() {
        return fallback.to_string();
    }
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_string();
    }

    let tail = value
        .chars()
        .rev()
        .take(max_chars - 3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("...{tail}")
}

fn compact_fangyuan_home_layout_path(path: &str, fallback: &str) -> String {
    const MAX_PATH_CHARS: usize = 30;

    let path = if path.trim().is_empty() {
        fallback
    } else {
        path.trim()
    };
    let char_count = path.chars().count();
    if char_count <= MAX_PATH_CHARS {
        return path.to_string();
    }

    let tail = path
        .chars()
        .rev()
        .take(MAX_PATH_CHARS - 3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("...{tail}")
}

pub(super) fn route_to_lobby_on_fangyuan_home_exit(
    mut scene_events: MessageReader<SceneEvent>,
    current_mode: Res<State<AppUiMode>>,
    mut route_cursor: Local<MessageCursor<GameRouteCommand>>,
    mut route_messages: ResMut<Messages<GameRouteCommand>>,
) {
    let already_routing_to_lobby = route_cursor
        .read(&route_messages)
        .any(is_lobby_route_command);

    let mut fangyuan_home_exited = false;
    for event in scene_events.read() {
        let SceneEvent::Exited(exited) = event else {
            continue;
        };

        if exited.scene_id.as_str() != FANGYUAN_HOME_SCENE_ID {
            continue;
        }

        fangyuan_home_exited = true;
        break;
    }

    if should_route_fangyuan_home_exit_to_lobby(*current_mode.get(), already_routing_to_lobby)
        && fangyuan_home_exited
    {
        route_messages.write(GameRouteCommand::ChangeMode(AppUiMode::Lobby));
    }
}

fn should_route_fangyuan_home_exit_to_lobby(
    current_mode: AppUiMode,
    already_routing_to_lobby: bool,
) -> bool {
    current_mode == AppUiMode::FangyuanHome && !already_routing_to_lobby
}

fn is_lobby_route_command(command: &GameRouteCommand) -> bool {
    matches!(command, GameRouteCommand::ChangeMode(AppUiMode::Lobby))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        framework::{
            fangyuan::{
                FangyuanAuditFinding, FangyuanAuditReport, FangyuanAuditSeverity,
                FangyuanAuditSourceKind, FangyuanDebugPanelToggles, FangyuanPrimitive,
                FangyuanPrimitiveKind, FangyuanPrimitiveRole, FangyuanPrimitiveSet,
                FangyuanSceneLayoutCompileReport,
            },
            scene::prelude::{SceneExited, SceneId, SceneSessionId},
        },
        game::scenes::FangyuanHomeBlueprintStats,
    };

    #[test]
    fn hud_status_text_updates_from_blueprint_stats() {
        let session_id = SceneSessionId::from("fangyuan-session");
        let compile_report = hud_test_layout_compile_report();
        let mut stats = FangyuanHomeBlueprintStats::default();
        stats.record_layout_loaded(
            &session_id,
            "fangyuan/home_scene.layout.ron",
            "fangyuan/home_prefabs.palette.ron",
            &hud_test_audit_report(Vec::new()),
            &compile_report,
            Default::default(),
        );
        let text = fangyuan_home_hud_status_text(Some(&stats), None);
        assert_eq!(
            text,
            "layout loaded gen 3/1000 skip 2\naudit passed e0 w0 -\npal 2 pf 5 used 4 inst 8 mat 3\nmatprof 1 opaque 1 trans 2 emi 2.0 uniq 3\nrender standard ib 0 ii 0 bytes 0 fb -\nchunk 0 obj 0 state pending fail - ids -\nlod f0 r0 s0 m0 h0 aoi 0 pressure normal degrade -\npath std0 mg0 inst0 mk0 hid0\ntrial none sel - profile standard run 0 status pending e0 w0 s0\ntrial vfx 0 tpl - vis -\neq 0 npc 0 td 0 cost 0/96/128\ntrial before 0 objects cost 0 after keep 0 degrade 0 reject 0\nresult k0 d0 r0 fb0 ok reason ok suggest - find ok\nl fangyuan/home_scene.layout.ron\np ...an/home_prefabs.palette.ron"
        );
    }

    #[test]
    fn fangyuan_debug_panel_text_uses_stats_without_overloading_default_hud() {
        let session_id = SceneSessionId::from("fangyuan-session");
        let compile_report = hud_test_layout_compile_report();
        let mut stats = FangyuanHomeBlueprintStats::default();
        stats.record_layout_loaded(
            &session_id,
            "fangyuan/home_scene.layout.ron",
            "fangyuan/home_prefabs.palette.ron",
            &hud_test_audit_report(vec![hud_test_finding(
                FangyuanAuditSeverity::Warning,
                "invalid_primitive_color",
            )]),
            &compile_report,
            crate::game::scenes::FangyuanHomeBlueprintRenderSummary {
                mode: "static_instance".to_string(),
                static_instance_batch_count: 2,
                static_instance_count: 3,
                static_instance_buffer_bytes: 384,
                ..Default::default()
            },
        );
        stats.lod_distribution = "f1 r2 s3 m4 h5".to_string();
        stats.lod_aoi_radius = 28.0;
        stats.lod_pressure = "warm".to_string();
        stats.lod_degrade_reason = "transparent".to_string();
        stats.trial_route_id = "fangyuan.object_trial".to_string();
        stats.active_vfx_count = 4;

        let chunk_summary = FangyuanChunkDebugSummary {
            loaded_chunks: 2,
            visible_objects: 9,
            ..Default::default()
        };
        let panel_state = FangyuanDebugPanelState {
            visible: true,
            compact: false,
            toggles: FangyuanDebugPanelToggles::default(),
        };
        let debug_text =
            fangyuan_home_debug_panel_text(Some(&stats), Some(&chunk_summary), &panel_state);
        let hud_text = fangyuan_home_hud_status_text(Some(&stats), Some(&chunk_summary));

        assert!(debug_text.contains("fangyuan debug panel"));
        assert!(debug_text.contains("render mode static_instance mesh 2 instance_batch 2"));
        assert!(debug_text.contains(
            "render buffer_update 384 buffer_bytes 384 draw_estimate 2 material_profile 1"
        ));
        assert!(
            debug_text.contains("lod distribution full 1 reduced 2 silhouette 3 marker 4 hidden 5")
        );
        assert!(debug_text.contains(
            "lod aoi_radius 28 hotspot_pressure active true severity warm units 1 reasons 1 degrade transparent"
        ));
        assert!(debug_text.contains("cache missing hit/miss pending"));
        assert!(debug_text.contains("bake missing artifact none"));
        assert!(debug_text.contains("trial route fangyuan.object_trial"));
        assert!(!hud_text.contains("fangyuan debug panel"));
        assert!(!hud_text.contains("cache missing hit/miss pending"));
    }

    #[test]
    fn hud_status_text_reports_clear_reload_and_failure_states() {
        let session_id = SceneSessionId::from("fangyuan-session");
        let compile_report = hud_test_layout_compile_report();
        let mut stats = FangyuanHomeBlueprintStats::default();

        stats.record_layout_loaded(
            &session_id,
            "fangyuan/home_scene.layout.ron",
            "fangyuan/home_prefabs.palette.ron",
            &hud_test_audit_report(Vec::new()),
            &compile_report,
            Default::default(),
        );
        assert_eq!(
            fangyuan_home_hud_status_text(Some(&stats), None),
            "layout loaded gen 3/1000 skip 2\naudit passed e0 w0 -\npal 2 pf 5 used 4 inst 8 mat 3\nmatprof 1 opaque 1 trans 2 emi 2.0 uniq 3\nrender standard ib 0 ii 0 bytes 0 fb -\nchunk 0 obj 0 state pending fail - ids -\nlod f0 r0 s0 m0 h0 aoi 0 pressure normal degrade -\npath std0 mg0 inst0 mk0 hid0\ntrial none sel - profile standard run 0 status pending e0 w0 s0\ntrial vfx 0 tpl - vis -\neq 0 npc 0 td 0 cost 0/96/128\ntrial before 0 objects cost 0 after keep 0 degrade 0 reject 0\nresult k0 d0 r0 fb0 ok reason ok suggest - find ok\nl fangyuan/home_scene.layout.ron\np ...an/home_prefabs.palette.ron"
        );

        stats.record_cleared(&session_id);
        assert_eq!(
            fangyuan_home_hud_status_text(Some(&stats), None),
            "layout cleared gen 0/1000 skip 2\naudit passed e0 w0 -\npal 2 pf 5 used 4 inst 8 mat 3\nmatprof 1 opaque 1 trans 2 emi 2.0 uniq 3\nrender standard ib 0 ii 0 bytes 0 fb -\nchunk 0 obj 0 state pending fail - ids -\nlod f0 r0 s0 m0 h0 aoi 0 pressure normal degrade -\npath std0 mg0 inst0 mk0 hid0\ntrial none sel - profile standard run 0 status pending e0 w0 s0\ntrial vfx 0 tpl - vis -\neq 0 npc 0 td 0 cost 0/96/128\ntrial before 0 objects cost 0 after keep 0 degrade 0 reject 0\nresult k0 d0 r0 fb0 ok reason ok suggest - find ok\nl fangyuan/home_scene.layout.ron\np ...an/home_prefabs.palette.ron"
        );

        stats.record_layout_loaded(
            &session_id,
            "fangyuan/home_scene.layout.ron",
            "fangyuan/home_prefabs.palette.ron",
            &hud_test_audit_report(vec![hud_test_finding(
                FangyuanAuditSeverity::Warning,
                "invalid_primitive_color",
            )]),
            &compile_report,
            crate::game::scenes::FangyuanHomeBlueprintRenderSummary {
                mode: "static_instance->standard".to_string(),
                static_instance_batch_count: 0,
                static_instance_count: 0,
                static_instance_buffer_bytes: 0,
                static_instance_fallback_reason:
                    "fangyuan static instance render budget exceeded: buffer_bytes=5000/1"
                        .to_string(),
                ..Default::default()
            },
        );
        assert_eq!(
            fangyuan_home_hud_status_text(Some(&stats), None),
            "layout loaded gen 3/1000 skip 2\naudit warning e0 w1 invalid_primitive_color\npal 2 pf 5 used 4 inst 8 mat 3\nmatprof 1 opaque 1 trans 2 emi 2.0 uniq 3\nrender static_instance->standard ib 0 ii 0 bytes 0 fb ...buffer_bytes=5000/1\nchunk 0 obj 0 state pending fail - ids -\nlod f0 r0 s0 m0 h0 aoi 0 pressure normal degrade -\npath std0 mg0 inst0 mk0 hid0\ntrial none sel - profile standard run 0 status pending e0 w0 s0\ntrial vfx 0 tpl - vis -\neq 0 npc 0 td 0 cost 0/96/128\ntrial before 0 objects cost 0 after keep 0 degrade 0 reject 0\nresult k0 d0 r0 fb0 ok reason ok suggest - find ok\nl fangyuan/home_scene.layout.ron\np ...an/home_prefabs.palette.ron"
        );

        stats.record_layout_failed(
            &session_id,
            "fangyuan/very/deep/generated/debug/home_scene_failure_case.layout.ron",
            "fangyuan/very/deep/generated/debug/home_prefabs_failure_case.palette.ron",
            3,
            Some(&hud_test_audit_report(vec![hud_test_finding(
                FangyuanAuditSeverity::Error,
                "missing_prefab",
            )])),
        );
        assert_eq!(
            fangyuan_home_hud_status_text(Some(&stats), None),
            "layout failed gen 0/1000 skip 0\naudit failed e1 w0 missing_prefab\npal 0 pf 0 used 0 inst 0 mat 3\nmatprof 0 opaque 0 trans 0 emi 0.0 uniq 3\nrender standard ib 0 ii 0 bytes 0 fb -\nchunk 0 obj 0 state pending fail - ids -\nlod f0 r0 s0 m0 h0 aoi 0 pressure normal degrade -\npath std0 mg0 inst0 mk0 hid0\ntrial none sel - profile standard run 0 status pending e0 w0 s0\ntrial vfx 0 tpl - vis -\neq 0 npc 0 td 0 cost 0/96/128\ntrial before 0 objects cost 0 after keep 0 degrade 0 reject 0\nresult k0 d0 r0 fb0 ok reason ok suggest - find ok\nl ...ene_failure_case.layout.ron\np ...bs_failure_case.palette.ron"
        );
    }

    #[test]
    fn hud_status_text_defaults_to_non_successful_empty_state() {
        assert_eq!(
            fangyuan_home_hud_status_text(None, None),
            "layout pending gen 0/1000 skip 0\naudit pending e0 w0 -\npal 0 pf 0 used 0 inst 0 mat 0\nmatprof 0 opaque 0 trans 0 emi 0.0 uniq 0\nrender standard ib 0 ii 0 bytes 0 fb -\nchunk 0 obj 0 state pending fail - ids -\nlod f0 r0 s0 m0 h0 aoi 0 pressure normal degrade -\npath std0 mg0 inst0 mk0 hid0\ntrial none sel - profile standard run 0 status pending e0 w0 s0\ntrial vfx 0 tpl - vis -\neq 0 npc 0 td 0 cost 0/96/128\ntrial before 0 objects cost 0 after keep 0 degrade 0 reject 0\nresult k0 d0 r0 fb0 ok reason ok suggest - find ok\nl ...uan/layouts/home_layout.ron\np ...n/palettes/home_prefabs.ron"
        );
    }

    #[test]
    fn hud_status_text_reports_chunk_debug_summary() {
        let chunk_summary = FangyuanChunkDebugSummary {
            loaded_chunks: 2,
            loaded_chunk_ids: vec!["home_chunk_a".to_string(), "home_chunk_b".to_string()],
            visible_objects: 9,
            load_state: "fallback".to_string(),
            failure_reason: "home_chunk_b:missing_prefab_ref".to_string(),
        };

        assert_eq!(
            fangyuan_home_hud_status_text(None, Some(&chunk_summary)),
            "layout pending gen 0/1000 skip 0\naudit pending e0 w0 -\npal 0 pf 0 used 0 inst 0 mat 0\nmatprof 0 opaque 0 trans 0 emi 0.0 uniq 0\nrender standard ib 0 ii 0 bytes 0 fb -\nchunk 2 obj 9 state fallback fail ...nk_b:missing_prefab_ref ids home_chunk_a,home_chunk_b\nlod f0 r0 s0 m0 h0 aoi 0 pressure normal degrade -\npath std0 mg0 inst0 mk0 hid0\ntrial none sel - profile standard run 0 status pending e0 w0 s0\ntrial vfx 0 tpl - vis -\neq 0 npc 0 td 0 cost 0/96/128\ntrial before 0 objects cost 0 after keep 0 degrade 0 reject 0\nresult k0 d0 r0 fb0 ok reason ok suggest - find ok\nl ...uan/layouts/home_layout.ron\np ...n/palettes/home_prefabs.ron"
        );
    }

    #[test]
    fn fangyuan_home_exit_fallback_only_routes_while_hud_is_active() {
        assert!(should_route_fangyuan_home_exit_to_lobby(
            AppUiMode::FangyuanHome,
            false
        ));
        assert!(!should_route_fangyuan_home_exit_to_lobby(
            AppUiMode::FangyuanHome,
            true
        ));
        assert!(!should_route_fangyuan_home_exit_to_lobby(
            AppUiMode::Lobby,
            false
        ));
        assert!(is_lobby_route_command(&GameRouteCommand::ChangeMode(
            AppUiMode::Lobby
        )));
        assert!(!is_lobby_route_command(&GameRouteCommand::ChangeMode(
            AppUiMode::FangyuanHome
        )));
    }

    #[test]
    fn fangyuan_home_exit_fallback_ignores_other_scene_ids() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<AppUiMode>()
            .add_message::<SceneEvent>()
            .add_message::<GameRouteCommand>()
            .add_systems(Update, route_to_lobby_on_fangyuan_home_exit);
        app.world_mut()
            .resource_mut::<NextState<AppUiMode>>()
            .set(AppUiMode::FangyuanHome);
        app.update();

        app.world_mut()
            .write_message(SceneEvent::Exited(SceneExited {
                scene_id: SceneId::from("sample.dungeon_room"),
                session_id: SceneSessionId::from("sample-session"),
            }));
        app.update();
        assert!(read_messages::<GameRouteCommand>(app.world()).is_empty());

        app.world_mut()
            .write_message(SceneEvent::Exited(SceneExited {
                scene_id: SceneId::from(FANGYUAN_HOME_SCENE_ID),
                session_id: SceneSessionId::from("fangyuan-session"),
            }));
        app.update();
        let route_commands = read_messages::<GameRouteCommand>(app.world());
        assert!(matches!(
            route_commands.last(),
            Some(GameRouteCommand::ChangeMode(AppUiMode::Lobby))
        ));
    }

    fn read_messages<M>(world: &World) -> Vec<M>
    where
        M: Message + Clone,
    {
        let messages = world.resource::<Messages<M>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    fn hud_test_primitive_set() -> FangyuanPrimitiveSet {
        FangyuanPrimitiveSet::from_primitives(vec![
            FangyuanPrimitive::with_runtime_metadata(
                FangyuanPrimitiveKind::Cube,
                Vec3::ZERO,
                Vec3::ONE,
                Color::srgba(0.1, 0.2, 0.3, 1.0),
                FangyuanPrimitiveRole::Structure,
                1.0,
                0.0,
                None,
                Default::default(),
            ),
            FangyuanPrimitive::with_runtime_metadata(
                FangyuanPrimitiveKind::Sphere,
                Vec3::Y,
                Vec3::ONE,
                Color::srgba(0.4, 0.5, 0.6, 0.5),
                FangyuanPrimitiveRole::Core,
                0.5,
                0.0,
                None,
                Default::default(),
            ),
            FangyuanPrimitive::with_runtime_metadata(
                FangyuanPrimitiveKind::Sphere,
                Vec3::NEG_Y,
                Vec3::ONE,
                Color::srgba(0.7, 0.8, 0.9, 0.25),
                FangyuanPrimitiveRole::Decoration,
                0.25,
                2.0,
                Some("glow".to_string()),
                Default::default(),
            ),
        ])
    }

    fn hud_test_layout_compile_report() -> FangyuanSceneLayoutCompileReport {
        let primitive_set = hud_test_primitive_set();
        FangyuanSceneLayoutCompileReport {
            primitive_stats: primitive_set.stats(),
            primitive_set,
            palette_count: 2,
            prefab_count: 5,
            authored_prefab_primitives: 7,
            instance_count: 8,
            generated_primitives: 3,
            skipped_primitives: 2,
            used_prefab_count: 4,
            top_level_validated: true,
            layout_validated: true,
            palette_validated: true,
            warnings: Vec::new(),
        }
    }

    fn hud_test_audit_report(findings: Vec<FangyuanAuditFinding>) -> FangyuanAuditReport {
        let mut report = FangyuanAuditReport::new(FangyuanAuditSourceKind::SceneLayout, None);
        for finding in findings {
            report.add_finding(finding);
        }
        report
    }

    fn hud_test_finding(severity: FangyuanAuditSeverity, code: &str) -> FangyuanAuditFinding {
        let mut finding = FangyuanAuditFinding::new(
            severity,
            code,
            "hud test audit finding",
            FangyuanAuditSourceKind::SceneLayout,
        );
        finding.field_path = Some("instances[0].prefab".to_string());
        finding
    }
}
