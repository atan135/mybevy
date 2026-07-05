use bevy::{
    ecs::message::{MessageCursor, Messages},
    prelude::*,
};

use crate::framework::{
    audio::prelude::UiAudioCueOverride,
    fangyuan::{
        FANGYUAN_HOME_PREFAB_PALETTE_PATH, FANGYUAN_HOME_SCENE_LAYOUT_PATH,
        FangyuanChunkDebugSummary, FangyuanChunkRuntime,
    },
    scene::prelude::{SceneCommand, SceneEvent, SceneExitRequest},
    ui::{
        core::{UiLayer, UiLayerRoot, UiMetrics, UiPanelKind, UiViewport, UiWidthClass},
        i18n::UiI18n,
        style::{
            UiFontAssets, UiTheme,
            theme::{
                UiThemeBackgroundRole, UiThemeBorderRole, UiThemePanelNodeRole,
                UiThemeRootNodeRole, UiThemeTextColorRole, UiThemeTextStyleRole,
            },
        },
        widgets::{
            UiButtonEvent, UiButtonEventKind, screen_label, screen_label_key, screen_title_key,
            secondary_action_button_key,
        },
    },
};
use crate::game::{
    audio::UI_CONFIRM_CUE_ID,
    navigation::{AppUiMode, GameRouteCommand, game_panel_root},
    scenes::{FANGYUAN_HOME_SCENE_ID, FangyuanHomeBlueprintCommand, FangyuanHomeBlueprintStats},
    ui_ids::{OWNER_FANGYUAN_HOME, PANEL_FANGYUAN_HOME_HUD},
};

const FANGYUAN_HOME_PRIMITIVE_LIMIT: usize = 1000;

#[derive(Component)]
pub(super) struct FangyuanHomeReloadButton;

#[derive(Component)]
pub(super) struct FangyuanHomeClearButton;

#[derive(Component)]
pub(super) struct FangyuanHomeTrialRerunAuditButton;

#[derive(Component)]
pub(super) struct FangyuanHomeTrialBudgetButton;

#[derive(Component)]
pub(super) struct FangyuanHomeLobbyButton;

#[derive(Component)]
pub(super) struct FangyuanHomeHudStatusText;

pub(super) fn setup_fangyuan_home_hud(
    mut commands: Commands,
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    viewport: Res<UiViewport>,
    fonts: Res<UiFontAssets>,
    i18n: Res<UiI18n>,
) {
    let theme = theme.into_inner();
    let metrics = metrics.into_inner();
    let viewport = *viewport;
    let fonts = fonts.into_inner();
    let i18n = i18n.into_inner();

    commands.spawn((
        DespawnOnExit(AppUiMode::FangyuanHome),
        game_panel_root(
            PANEL_FANGYUAN_HOME_HUD,
            UiPanelKind::Hud,
            OWNER_FANGYUAN_HOME,
        ),
        UiLayerRoot {
            layer: UiLayer::Page,
        },
        fangyuan_home_hud_root_node(&viewport, metrics, theme),
        UiThemeRootNodeRole::Overlay,
        children![
            (
                UiThemePanelNodeRole::Content,
                fangyuan_home_status_panel_node(&viewport, theme),
                BackgroundColor(theme.colors.panel_background),
                BorderColor::all(theme.colors.panel_border),
                UiThemeBackgroundRole::Panel,
                UiThemeBorderRole::Panel,
                children![
                    screen_title_key(
                        theme,
                        fonts,
                        i18n,
                        "fangyuan_home.hud.title",
                        "方圆灵构家园",
                        UiThemeTextStyleRole::Title,
                    ),
                    screen_label_key(
                        theme,
                        fonts,
                        i18n,
                        "fangyuan_home.hud.scene",
                        "原型预览",
                        UiThemeTextStyleRole::Caption,
                        UiThemeTextColorRole::Muted,
                    ),
                    (
                        screen_label(
                            theme,
                            fonts,
                            fangyuan_home_hud_status_text(None, None),
                            UiThemeTextStyleRole::Caption,
                            UiThemeTextColorRole::Muted,
                        ),
                        FangyuanHomeHudStatusText,
                    ),
                ],
            ),
            (
                fangyuan_home_button_column_node(&viewport, theme),
                children![
                    (
                        secondary_action_button_key(
                            theme,
                            metrics,
                            fonts,
                            i18n,
                            "fangyuan_home.hud.reload",
                            "重新加载",
                        ),
                        FangyuanHomeReloadButton,
                    ),
                    (
                        secondary_action_button_key(
                            theme,
                            metrics,
                            fonts,
                            i18n,
                            "fangyuan_home.hud.clear",
                            "清空",
                        ),
                        FangyuanHomeClearButton,
                    ),
                    (
                        secondary_action_button_key(
                            theme,
                            metrics,
                            fonts,
                            i18n,
                            "fangyuan_home.hud.trial_rerun",
                            "重审",
                        ),
                        FangyuanHomeTrialRerunAuditButton,
                    ),
                    (
                        secondary_action_button_key(
                            theme,
                            metrics,
                            fonts,
                            i18n,
                            "fangyuan_home.hud.trial_budget",
                            "预算",
                        ),
                        FangyuanHomeTrialBudgetButton,
                    ),
                    (
                        secondary_action_button_key(
                            theme,
                            metrics,
                            fonts,
                            i18n,
                            "nav.lobby",
                            "大厅",
                        ),
                        fangyuan_home_lobby_button_audio_override(),
                        FangyuanHomeLobbyButton,
                    ),
                ],
            ),
        ],
    ));
}

fn fangyuan_home_hud_root_node(
    viewport: &UiViewport,
    metrics: &UiMetrics,
    theme: &UiTheme,
) -> Node {
    let compact = viewport.width_class == UiWidthClass::Compact;
    Node {
        width: percent(100),
        height: percent(100),
        padding: viewport.safe_area_padding(metrics.page_padding),
        align_items: AlignItems::FlexStart,
        justify_content: if compact {
            JustifyContent::FlexStart
        } else {
            JustifyContent::SpaceBetween
        },
        flex_direction: if compact {
            FlexDirection::Column
        } else {
            FlexDirection::Row
        },
        row_gap: px(theme.layout.row_gap),
        column_gap: px(theme.layout.header_gap),
        ..default()
    }
}

fn fangyuan_home_status_panel_node(viewport: &UiViewport, theme: &UiTheme) -> Node {
    let compact = viewport.width_class == UiWidthClass::Compact;
    Node {
        width: if compact { percent(100) } else { auto() },
        max_width: px(if compact { 360.0 } else { 420.0 }),
        flex_direction: FlexDirection::Column,
        overflow: Overflow::clip(),
        row_gap: px(theme.layout.row_gap),
        padding: UiRect::all(px(theme.layout.panel_gap)),
        border: UiRect::all(px(theme.panel.border)),
        border_radius: BorderRadius::all(px(theme.panel.radius)),
        ..default()
    }
}

fn fangyuan_home_button_column_node(viewport: &UiViewport, theme: &UiTheme) -> Node {
    let compact = viewport.width_class == UiWidthClass::Compact;
    Node {
        flex_direction: if compact {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        },
        flex_wrap: FlexWrap::Wrap,
        row_gap: px(theme.layout.row_gap),
        column_gap: px(theme.layout.row_column_gap),
        align_items: AlignItems::Stretch,
        align_self: if compact {
            AlignSelf::FlexStart
        } else {
            AlignSelf::Auto
        },
        ..default()
    }
}

fn fangyuan_home_lobby_button_audio_override() -> UiAudioCueOverride {
    UiAudioCueOverride::try_from(UI_CONFIRM_CUE_ID)
        .expect("fangyuan home lobby button UI audio cue id must be valid")
}

pub(super) fn update_fangyuan_home_hud_status(
    stats: Res<FangyuanHomeBlueprintStats>,
    chunk_runtime: Option<Res<FangyuanChunkRuntime>>,
    mut status_texts: Query<&mut Text, With<FangyuanHomeHudStatusText>>,
) {
    let chunk_summary = chunk_runtime
        .as_deref()
        .map(FangyuanChunkRuntime::debug_summary)
        .unwrap_or_default();
    let status = fangyuan_home_hud_status_text(Some(&stats), Some(&chunk_summary));
    for mut text in &mut status_texts {
        if text.0 != status {
            text.0 = status.clone();
        }
    }
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

pub(super) fn handle_fangyuan_home_hud_buttons(
    mut blueprint_commands: MessageWriter<FangyuanHomeBlueprintCommand>,
    mut scene_commands: MessageWriter<SceneCommand>,
    mut route_commands: MessageWriter<GameRouteCommand>,
    reload_buttons: Query<(), With<FangyuanHomeReloadButton>>,
    clear_buttons: Query<(), With<FangyuanHomeClearButton>>,
    trial_rerun_buttons: Query<(), With<FangyuanHomeTrialRerunAuditButton>>,
    trial_budget_buttons: Query<(), With<FangyuanHomeTrialBudgetButton>>,
    lobby_buttons: Query<(), With<FangyuanHomeLobbyButton>>,
    mut button_events: MessageReader<UiButtonEvent>,
) {
    for event in button_events.read() {
        if event.kind != UiButtonEventKind::Click {
            continue;
        }

        if reload_buttons.contains(event.entity) {
            blueprint_commands.write(FangyuanHomeBlueprintCommand::Reload);
        } else if clear_buttons.contains(event.entity) {
            blueprint_commands.write(FangyuanHomeBlueprintCommand::Clear);
        } else if trial_rerun_buttons.contains(event.entity) {
            blueprint_commands.write(FangyuanHomeBlueprintCommand::RerunTrialAudit);
        } else if trial_budget_buttons.contains(event.entity) {
            blueprint_commands.write(FangyuanHomeBlueprintCommand::SwitchTrialBudget);
        } else if lobby_buttons.contains(event.entity) {
            scene_commands.write(SceneCommand::Exit(SceneExitRequest::default()));
            route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::Lobby));
        }
    }
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
                FangyuanAuditSourceKind, FangyuanPrimitive, FangyuanPrimitiveKind,
                FangyuanPrimitiveRole, FangyuanPrimitiveSet, FangyuanSceneLayoutCompileReport,
            },
            scene::prelude::{SceneExited, SceneId, SceneSessionId},
            ui::widgets::UiButtonEvent,
        },
        game::scenes::FangyuanHomeBlueprintStats,
    };

    #[test]
    fn hud_buttons_write_reload_clear_and_lobby_exit_route() {
        let mut app = App::new();
        app.add_message::<FangyuanHomeBlueprintCommand>()
            .add_message::<SceneCommand>()
            .add_message::<GameRouteCommand>()
            .add_message::<UiButtonEvent>()
            .add_systems(Update, handle_fangyuan_home_hud_buttons);

        let reload_button = app.world_mut().spawn(FangyuanHomeReloadButton).id();
        let clear_button = app.world_mut().spawn(FangyuanHomeClearButton).id();
        let trial_rerun_button = app
            .world_mut()
            .spawn(FangyuanHomeTrialRerunAuditButton)
            .id();
        let trial_budget_button = app.world_mut().spawn(FangyuanHomeTrialBudgetButton).id();
        let lobby_button = app.world_mut().spawn(FangyuanHomeLobbyButton).id();
        let ignored_button = app.world_mut().spawn_empty().id();

        app.world_mut().write_message(UiButtonEvent {
            entity: ignored_button,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.world_mut().write_message(UiButtonEvent {
            entity: reload_button,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.world_mut().write_message(UiButtonEvent {
            entity: clear_button,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.world_mut().write_message(UiButtonEvent {
            entity: trial_rerun_button,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.world_mut().write_message(UiButtonEvent {
            entity: trial_budget_button,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.world_mut().write_message(UiButtonEvent {
            entity: lobby_button,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.update();

        assert_eq!(
            read_messages::<FangyuanHomeBlueprintCommand>(app.world()),
            vec![
                FangyuanHomeBlueprintCommand::Reload,
                FangyuanHomeBlueprintCommand::Clear,
                FangyuanHomeBlueprintCommand::RerunTrialAudit,
                FangyuanHomeBlueprintCommand::SwitchTrialBudget
            ]
        );
        assert_eq!(
            read_messages::<SceneCommand>(app.world()),
            vec![SceneCommand::Exit(SceneExitRequest::default())]
        );
        let route_commands = read_messages::<GameRouteCommand>(app.world());
        assert_eq!(route_commands.len(), 1);
        assert!(matches!(
            route_commands[0],
            GameRouteCommand::ChangeMode(AppUiMode::Lobby)
        ));
    }

    #[test]
    fn fangyuan_trial_hud_buttons_write_rerun_budget_and_lobby_return_commands() {
        let mut app = App::new();
        app.add_message::<FangyuanHomeBlueprintCommand>()
            .add_message::<SceneCommand>()
            .add_message::<GameRouteCommand>()
            .add_message::<UiButtonEvent>()
            .add_systems(Update, handle_fangyuan_home_hud_buttons);

        let rerun_button = app
            .world_mut()
            .spawn(FangyuanHomeTrialRerunAuditButton)
            .id();
        let budget_button = app.world_mut().spawn(FangyuanHomeTrialBudgetButton).id();
        let lobby_button = app.world_mut().spawn(FangyuanHomeLobbyButton).id();

        for entity in [rerun_button, budget_button, lobby_button] {
            app.world_mut().write_message(UiButtonEvent {
                entity,
                kind: UiButtonEventKind::Click,
                button: None,
            });
        }
        app.update();

        assert_eq!(
            read_messages::<FangyuanHomeBlueprintCommand>(app.world()),
            vec![
                FangyuanHomeBlueprintCommand::RerunTrialAudit,
                FangyuanHomeBlueprintCommand::SwitchTrialBudget
            ]
        );
        assert_eq!(
            read_messages::<SceneCommand>(app.world()),
            vec![SceneCommand::Exit(SceneExitRequest::default())]
        );
        assert!(matches!(
            read_messages::<GameRouteCommand>(app.world()).as_slice(),
            [GameRouteCommand::ChangeMode(AppUiMode::Lobby)]
        ));
    }

    #[test]
    fn hud_status_text_updates_from_blueprint_stats() {
        let mut app = App::new();
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
        app.insert_resource(stats)
            .add_systems(Update, update_fangyuan_home_hud_status);
        let status_text = app
            .world_mut()
            .spawn((Text::new("pending"), FangyuanHomeHudStatusText))
            .id();

        app.update();

        let text = app.world().get::<Text>(status_text).unwrap();
        assert_eq!(
            text.0,
            "layout loaded gen 3/1000 skip 2\naudit passed e0 w0 -\npal 2 pf 5 used 4 inst 8 mat 3\nmatprof 1 opaque 1 trans 2 emi 2.0 uniq 3\nrender standard ib 0 ii 0 bytes 0 fb -\nchunk 0 obj 0 state pending fail - ids -\nlod f0 r0 s0 m0 h0 aoi 0 pressure normal degrade -\npath std0 mg0 inst0 mk0 hid0\ntrial none sel - profile standard run 0 status pending e0 w0 s0\ntrial vfx 0 tpl - vis -\neq 0 npc 0 td 0 cost 0/96/128\ntrial before 0 objects cost 0 after keep 0 degrade 0 reject 0\nresult k0 d0 r0 fb0 ok reason ok suggest - find ok\nl fangyuan/home_scene.layout.ron\np ...an/home_prefabs.palette.ron"
        );
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

    #[test]
    fn lobby_button_uses_confirm_audio_override() {
        assert_eq!(
            fangyuan_home_lobby_button_audio_override().cue_id.as_str(),
            UI_CONFIRM_CUE_ID
        );
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
