use std::{collections::BTreeMap, str::FromStr};

use bevy::prelude::*;

use crate::framework::ui::{
    core::{UiOwnerId, focus::UiFocusState},
    document::{
        UiActionId, UiActionRegistry, UiBindingDeclaration, UiBindingPath, UiDocumentId,
        UiDocumentLayer, UiDocumentPanel, UiHostBindingKey, UiPageState,
    },
};
use crate::game::{
    declarative_screen::{
        DeclarativeScreenFailurePolicy, DeclarativeScreenHost, DeclarativeScreenHostCommand,
        DeclarativeScreenHostEvent, DeclarativeScreenRegistry, DeclarativeScreenSource,
    },
    myserver::{
        ActivityClientState, ActivityError, ActivityEvent, ActivityKey, ActivityRequestKind,
        ActivitySummary, DEFAULT_ACTIVITY_HISTORY_PAGE_SIZE, GameConnectionState,
        MAX_ACTIVITY_HISTORY_CLIENT_RECORDS, MyServerCommand, MyServerSession,
    },
    scenes::main_world_entry::{MainWorldEntryPhase, MainWorldEntryState},
    ui_ids::{
        OWNER_MAIN_WORLD_ACTIVITY_CLAIM_HISTORY, OWNER_MAIN_WORLD_ACTIVITY_DETAIL,
        OWNER_MAIN_WORLD_ACTIVITY_LIST, OWNER_MAIN_WORLD_LOGIN_REWARD_DETAIL,
        OWNER_MAIN_WORLD_LOTTERY_DETAIL,
    },
};

use super::{claim_history, detail, list, login_reward, lottery};

pub(in crate::game) const ACTIVITY_LIST_DOCUMENT_ID: &str = "game.main_world_activity_list";
pub(in crate::game) const ACTIVITY_DETAIL_DOCUMENT_ID: &str = "game.main_world_activity_detail";
pub(in crate::game) const ACTIVITY_CLAIM_HISTORY_DOCUMENT_ID: &str =
    "game.main_world_activity_claim_history";
pub(in crate::game) const LOGIN_REWARD_DETAIL_DOCUMENT_ID: &str =
    "game.main_world_login_reward_detail";
pub(in crate::game) const LOTTERY_DETAIL_DOCUMENT_ID: &str = "game.main_world_lottery_detail";
pub(in crate::game) const ACTIVITY_LIST_ROUTE: &str = "main_world_activity_list";
pub(in crate::game) const ACTIVITY_DETAIL_ROUTE: &str = "main_world_activity_detail";
pub(in crate::game) const ACTIVITY_CLAIM_HISTORY_ROUTE: &str = "main_world_activity_claim_history";
pub(in crate::game) const LOGIN_REWARD_DETAIL_ROUTE: &str = "main_world_login_reward_detail";
pub(in crate::game) const LOTTERY_DETAIL_ROUTE: &str = "main_world_lottery_detail";

pub(super) const ACTIVITY_LIST_DOCUMENT_SOURCE: &str = include_str!(
    "../../../../assets/ui/documents/approved/activity/main_world_activity_list.v1.json"
);
pub(super) const ACTIVITY_DETAIL_DOCUMENT_SOURCE: &str = include_str!(
    "../../../../assets/ui/documents/approved/activity/main_world_activity_detail.v1.json"
);
pub(super) const ACTIVITY_CLAIM_HISTORY_DOCUMENT_SOURCE: &str = include_str!(
    "../../../../assets/ui/documents/approved/activity/main_world_activity_claim_history.v1.json"
);
pub(super) const LOGIN_REWARD_DETAIL_DOCUMENT_SOURCE: &str = include_str!(
    "../../../../assets/ui/documents/approved/activity/main_world_login_reward_detail.v1.json"
);
pub(super) const LOTTERY_DETAIL_DOCUMENT_SOURCE: &str = include_str!(
    "../../../../assets/ui/documents/approved/activity/main_world_lottery_detail.v1.json"
);

const DEFAULT_AUDIT_PROFILES: [&str; 4] = [
    "desktop",
    "phone-landscape",
    "phone-1080p-landscape",
    "tablet-landscape",
];

#[derive(Clone, Debug, Message, PartialEq, Eq)]
pub(in crate::game) enum ActivityScreenCommand {
    OpenList,
    RetryList,
    OpenDetail { activity_id: String, version: u32 },
    CloseDetail,
    OpenClaimHistory,
    RefreshClaimHistory,
    RetryClaimHistory,
    LoadMoreClaimHistory,
    CloseClaimHistory,
    CloseList,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum ActivityListLoadState {
    #[default]
    Idle,
    Loading,
    Ready,
    Failed,
}

impl ActivityListLoadState {
    pub(super) const fn binding_value(self) -> &'static str {
        match self {
            Self::Idle | Self::Ready => "ready",
            Self::Loading => "loading",
            Self::Failed => "error",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum ActivityDetailLoadState {
    #[default]
    Idle,
    Loading,
    Ready,
    Failed,
}

impl ActivityDetailLoadState {
    pub(super) const fn binding_value(self) -> &'static str {
        match self {
            Self::Idle | Self::Ready => "ready",
            Self::Loading => "loading",
            Self::Failed => "error",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum ActivityClaimHistoryLoadState {
    #[default]
    Idle,
    LoadingFirstPage,
    LoadingMore,
    Ready,
    Empty,
    Failed,
}

impl ActivityClaimHistoryLoadState {
    pub(super) const fn collection_binding_value(self, has_records: bool) -> &'static str {
        match self {
            Self::LoadingFirstPage => "loading",
            Self::Failed if !has_records => "error",
            Self::Idle | Self::LoadingMore | Self::Ready | Self::Empty | Self::Failed => "ready",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ActivityClaimHistoryRetryMode {
    FirstPage,
    LoadMore,
}

#[derive(Clone, Debug, Default, Resource)]
pub(in crate::game) struct ActivityScreenState {
    pub(super) main_world_generation: Option<u64>,
    pub(super) list_open: bool,
    pub(super) detail_open: bool,
    pub(super) detail_route: Option<&'static str>,
    pub(super) selected: Option<ActivityKey>,
    pub(super) claim_history_open: bool,
    pub(super) list_load_state: ActivityListLoadState,
    pub(super) detail_load_state: ActivityDetailLoadState,
    pub(super) list_error: Option<ActivityError>,
    pub(super) detail_error: Option<ActivityError>,
    pub(super) claim_history_load_state: ActivityClaimHistoryLoadState,
    pub(super) claim_history_retry_mode: Option<ActivityClaimHistoryRetryMode>,
    pub(super) claim_history_error: Option<ActivityError>,
}

impl ActivityScreenState {
    pub(super) fn accepts_active_main_world(
        &self,
        entry: Option<&MainWorldEntryState>,
        session: Option<&MyServerSession>,
    ) -> bool {
        self.main_world_generation
            .zip(entry.map(|entry| entry.generation))
            .is_some_and(|(expected, current)| expected == current)
            && activity_session_is_usable(entry, session)
    }

    fn reset_closed(&mut self) {
        self.main_world_generation = None;
        self.list_open = false;
        self.detail_open = false;
        self.detail_route = None;
        self.selected = None;
        self.claim_history_open = false;
        self.list_load_state = ActivityListLoadState::Idle;
        self.detail_load_state = ActivityDetailLoadState::Idle;
        self.list_error = None;
        self.detail_error = None;
        self.claim_history_load_state = ActivityClaimHistoryLoadState::Idle;
        self.claim_history_retry_mode = None;
        self.claim_history_error = None;
    }

    fn close_detail(&mut self) {
        self.detail_open = false;
        self.detail_route = None;
        self.selected = None;
        self.detail_load_state = ActivityDetailLoadState::Idle;
        self.detail_error = None;
    }

    fn close_claim_history(&mut self) {
        self.claim_history_open = false;
        self.claim_history_load_state = ActivityClaimHistoryLoadState::Idle;
        self.claim_history_retry_mode = None;
        self.claim_history_error = None;
    }
}

#[derive(Resource)]
pub(super) struct ActivityScreenHostContract {
    pub(super) list_bindings: BTreeMap<UiBindingPath, UiBindingDeclaration>,
    pub(super) detail_bindings: BTreeMap<UiBindingPath, UiBindingDeclaration>,
    pub(super) claim_history_bindings: BTreeMap<UiBindingPath, UiBindingDeclaration>,
    pub(super) login_reward_bindings: BTreeMap<UiBindingPath, UiBindingDeclaration>,
    pub(super) lottery_bindings: BTreeMap<UiBindingPath, UiBindingDeclaration>,
}

impl Default for ActivityScreenHostContract {
    fn default() -> Self {
        Self {
            list_bindings: list::activity_list_binding_schema(),
            detail_bindings: detail::activity_detail_binding_schema(),
            claim_history_bindings: claim_history::activity_claim_history_binding_schema(),
            login_reward_bindings: login_reward::login_reward_binding_schema(),
            lottery_bindings: lottery::lottery_binding_schema(),
        }
    }
}

pub(super) fn register_activity_screen_contract(
    contract: Res<ActivityScreenHostContract>,
    mut actions: ResMut<UiActionRegistry>,
    mut screens: ResMut<DeclarativeScreenRegistry>,
) {
    for descriptor in list::activity_list_action_descriptors()
        .into_iter()
        .chain(detail::activity_detail_action_descriptors())
        .chain(claim_history::activity_claim_history_action_descriptors())
        .chain(login_reward::login_reward_action_descriptors())
        .chain(lottery::lottery_action_descriptors())
    {
        actions
            .register(descriptor)
            .expect("activity action registration must be valid and unique");
    }
    for host in [
        activity_list_declarative_screen_host(&contract),
        activity_detail_declarative_screen_host(&contract),
        activity_claim_history_declarative_screen_host(&contract),
        login_reward_detail_declarative_screen_host(&contract),
        lottery_detail_declarative_screen_host(&contract),
    ] {
        screens
            .register(host)
            .expect("activity screen registration must be valid and unique");
    }
}

fn activity_list_declarative_screen_host(
    contract: &ActivityScreenHostContract,
) -> DeclarativeScreenHost {
    activity_host(
        ACTIVITY_LIST_DOCUMENT_ID,
        ACTIVITY_LIST_ROUTE,
        &[ACTIVITY_LIST_ROUTE, "main-world-activity-list", "activity"],
        OWNER_MAIN_WORLD_ACTIVITY_LIST,
        "activity/main_world_activity_list.v1.json",
        ACTIVITY_LIST_DOCUMENT_SOURCE,
        &[
            list::ACTION_ACTIVITY_LIST_RETRY,
            list::ACTION_ACTIVITY_LIST_SELECT,
            list::ACTION_ACTIVITY_LIST_CLAIM_HISTORY,
            list::ACTION_ACTIVITY_LIST_CLOSE,
        ],
        &contract.list_bindings,
    )
}

fn activity_claim_history_declarative_screen_host(
    contract: &ActivityScreenHostContract,
) -> DeclarativeScreenHost {
    activity_host(
        ACTIVITY_CLAIM_HISTORY_DOCUMENT_ID,
        ACTIVITY_CLAIM_HISTORY_ROUTE,
        &[
            ACTIVITY_CLAIM_HISTORY_ROUTE,
            "main-world-activity-claim-history",
        ],
        OWNER_MAIN_WORLD_ACTIVITY_CLAIM_HISTORY,
        "activity/main_world_activity_claim_history.v1.json",
        ACTIVITY_CLAIM_HISTORY_DOCUMENT_SOURCE,
        &[
            claim_history::ACTION_ACTIVITY_CLAIM_HISTORY_REFRESH,
            claim_history::ACTION_ACTIVITY_CLAIM_HISTORY_RETRY,
            claim_history::ACTION_ACTIVITY_CLAIM_HISTORY_LOAD_MORE,
            claim_history::ACTION_ACTIVITY_CLAIM_HISTORY_CLOSE,
        ],
        &contract.claim_history_bindings,
    )
}

fn activity_detail_declarative_screen_host(
    contract: &ActivityScreenHostContract,
) -> DeclarativeScreenHost {
    activity_host(
        ACTIVITY_DETAIL_DOCUMENT_ID,
        ACTIVITY_DETAIL_ROUTE,
        &[ACTIVITY_DETAIL_ROUTE, "main-world-activity-detail"],
        OWNER_MAIN_WORLD_ACTIVITY_DETAIL,
        "activity/main_world_activity_detail.v1.json",
        ACTIVITY_DETAIL_DOCUMENT_SOURCE,
        &[
            detail::ACTION_ACTIVITY_DETAIL_BACK,
            detail::ACTION_ACTIVITY_DETAIL_CLOSE,
        ],
        &contract.detail_bindings,
    )
}

fn login_reward_detail_declarative_screen_host(
    contract: &ActivityScreenHostContract,
) -> DeclarativeScreenHost {
    activity_host(
        LOGIN_REWARD_DETAIL_DOCUMENT_ID,
        LOGIN_REWARD_DETAIL_ROUTE,
        &[LOGIN_REWARD_DETAIL_ROUTE, "main-world-login-reward-detail"],
        OWNER_MAIN_WORLD_LOGIN_REWARD_DETAIL,
        "activity/main_world_login_reward_detail.v1.json",
        LOGIN_REWARD_DETAIL_DOCUMENT_SOURCE,
        &[
            login_reward::ACTION_LOGIN_REWARD_CLAIM,
            login_reward::ACTION_LOGIN_REWARD_BACK,
            login_reward::ACTION_LOGIN_REWARD_CLOSE,
        ],
        &contract.login_reward_bindings,
    )
}

fn lottery_detail_declarative_screen_host(
    contract: &ActivityScreenHostContract,
) -> DeclarativeScreenHost {
    activity_host(
        LOTTERY_DETAIL_DOCUMENT_ID,
        LOTTERY_DETAIL_ROUTE,
        &[LOTTERY_DETAIL_ROUTE, "main-world-lottery-detail"],
        OWNER_MAIN_WORLD_LOTTERY_DETAIL,
        "activity/main_world_lottery_detail.v1.json",
        LOTTERY_DETAIL_DOCUMENT_SOURCE,
        &[
            lottery::ACTION_LOTTERY_DRAW,
            lottery::ACTION_LOTTERY_BACK,
            lottery::ACTION_LOTTERY_CLOSE,
        ],
        &contract.lottery_bindings,
    )
}

#[allow(clippy::too_many_arguments)]
fn activity_host(
    document_id: &'static str,
    route: &'static str,
    route_aliases: &'static [&'static str],
    owner: UiOwnerId,
    source_path: &'static str,
    source_json: &'static str,
    action_ids: &[&str],
    bindings: &BTreeMap<UiBindingPath, UiBindingDeclaration>,
) -> DeclarativeScreenHost {
    let source = DeclarativeScreenSource::approved(source_path, source_json);
    DeclarativeScreenHost {
        document_id: UiDocumentId::from_str(document_id).expect("activity document ID is static"),
        route,
        route_aliases,
        mode: None,
        owner,
        panel: UiDocumentPanel::Floating,
        layer: UiDocumentLayer::Floating,
        initial_state: UiPageState::initial(),
        binding_schema: bindings
            .iter()
            .filter(|(_, declaration)| {
                matches!(
                    declaration.scope,
                    crate::framework::ui::document::UiBindingScope::Document
                        | crate::framework::ui::document::UiBindingScope::Owner
                )
            })
            .map(|(path, declaration)| {
                (
                    UiHostBindingKey::new(declaration.scope, path.clone()),
                    declaration.value_type.clone(),
                )
            })
            .collect(),
        action_allowlist: action_ids
            .iter()
            .map(|action| UiActionId::from_str(action).expect("activity action ID is static"))
            .collect(),
        audit_profiles: DEFAULT_AUDIT_PROFILES.map(str::to_owned).to_vec(),
        source: source.clone(),
        fallback_source: Some(source),
        failure_policy: DeclarativeScreenFailurePolicy::PackagedFallback,
    }
}

pub(super) fn activity_session_is_usable(
    entry: Option<&MainWorldEntryState>,
    session: Option<&MyServerSession>,
) -> bool {
    matches!(
        (entry, session),
        (Some(entry), Some(session))
            if entry.phase == MainWorldEntryPhase::Active
                && entry.character_id.as_deref() == session.character_id.as_deref()
                && session.authenticated
                && session.connected
                && session.connection_id.is_some()
                && session.game_connection_state == GameConnectionState::Authenticated
    )
}

pub(super) fn visible_activity(summary: &ActivitySummary) -> bool {
    matches!(summary.activity_type.as_str(), "login_reward" | "lottery")
        && matches!(summary.status.as_str(), "published" | "running" | "ended")
}

pub(super) fn handle_activity_screen_commands(
    mut commands: MessageReader<ActivityScreenCommand>,
    entry: Option<Res<MainWorldEntryState>>,
    session: Option<Res<MyServerSession>>,
    activity: Res<ActivityClientState>,
    mut state: ResMut<ActivityScreenState>,
    mut myserver_commands: MessageWriter<MyServerCommand>,
    mut screen_commands: MessageWriter<DeclarativeScreenHostCommand>,
    mut focus: Option<ResMut<UiFocusState>>,
) {
    if !activity_session_is_usable(entry.as_deref(), session.as_deref()) {
        let was_open = state.list_open || state.detail_open || state.claim_history_open;
        state.reset_closed();
        if was_open {
            close_activity_routes(&mut screen_commands);
            clear_focus(&mut focus);
        }
        return;
    }
    let generation = entry.as_deref().map(|entry| entry.generation);
    for command in commands.read().cloned() {
        match command {
            ActivityScreenCommand::OpenList => {
                if state.list_open && state.main_world_generation == generation {
                    continue;
                }
                state.main_world_generation = generation;
                state.list_open = true;
                state.detail_open = false;
                state.detail_route = None;
                state.selected = None;
                state.close_claim_history();
                state.list_load_state = ActivityListLoadState::Loading;
                state.list_error = None;
                state.detail_load_state = ActivityDetailLoadState::Idle;
                state.detail_error = None;
                screen_commands.write(DeclarativeScreenHostCommand::OpenDetachedRoute {
                    route: ACTIVITY_LIST_ROUTE.to_owned(),
                });
                myserver_commands.write(MyServerCommand::LoadActivityList);
            }
            ActivityScreenCommand::RetryList => {
                if !state.list_open
                    || !state.accepts_active_main_world(entry.as_deref(), session.as_deref())
                    || state.claim_history_open
                    || state.list_load_state == ActivityListLoadState::Loading
                {
                    continue;
                }
                state.list_load_state = ActivityListLoadState::Loading;
                state.list_error = None;
                myserver_commands.write(MyServerCommand::LoadActivityList);
            }
            ActivityScreenCommand::OpenDetail {
                activity_id,
                version,
            } => {
                if !state.list_open
                    || !state.accepts_active_main_world(entry.as_deref(), session.as_deref())
                    || state.claim_history_open
                {
                    continue;
                }
                let key = ActivityKey {
                    activity_id,
                    version,
                };
                if !activity.activities.iter().any(|summary| {
                    visible_activity(summary)
                        && summary.activity_id == key.activity_id
                        && summary.version == key.version
                }) || (state.detail_open && state.selected.as_ref() == Some(&key))
                {
                    continue;
                }
                let detail_route = detail_route_for_activity(&key, &activity)
                    .expect("visible activity has a fixed detail route");
                if state.detail_open {
                    screen_commands.write(DeclarativeScreenHostCommand::CloseRoute {
                        route: state
                            .detail_route
                            .unwrap_or(ACTIVITY_DETAIL_ROUTE)
                            .to_owned(),
                    });
                }
                state.detail_open = true;
                state.detail_route = Some(detail_route);
                state.selected = Some(key.clone());
                state.detail_load_state = ActivityDetailLoadState::Loading;
                state.detail_error = None;
                screen_commands.write(DeclarativeScreenHostCommand::OpenDetachedRoute {
                    route: detail_route.to_owned(),
                });
                myserver_commands.write(MyServerCommand::LoadActivityDetail {
                    activity_id: key.activity_id,
                    version: key.version,
                });
            }
            ActivityScreenCommand::CloseDetail => {
                if !state.detail_open {
                    continue;
                }
                let detail_route = state.detail_route.unwrap_or(ACTIVITY_DETAIL_ROUTE);
                state.close_detail();
                screen_commands.write(DeclarativeScreenHostCommand::CloseRoute {
                    route: detail_route.to_owned(),
                });
                clear_focus(&mut focus);
            }
            ActivityScreenCommand::OpenClaimHistory => {
                if !state.list_open
                    || state.detail_open
                    || state.claim_history_open
                    || !state.accepts_active_main_world(entry.as_deref(), session.as_deref())
                {
                    continue;
                }
                state.claim_history_open = true;
                request_first_claim_history_page(&mut state, &mut myserver_commands);
                screen_commands.write(DeclarativeScreenHostCommand::OpenDetachedRoute {
                    route: ACTIVITY_CLAIM_HISTORY_ROUTE.to_owned(),
                });
            }
            ActivityScreenCommand::RefreshClaimHistory => {
                if !state.claim_history_open
                    || activity.claim_history.loading
                    || matches!(
                        state.claim_history_load_state,
                        ActivityClaimHistoryLoadState::LoadingFirstPage
                            | ActivityClaimHistoryLoadState::LoadingMore
                    )
                    || !state.accepts_active_main_world(entry.as_deref(), session.as_deref())
                {
                    continue;
                }
                request_first_claim_history_page(&mut state, &mut myserver_commands);
            }
            ActivityScreenCommand::RetryClaimHistory => {
                if !state.claim_history_open
                    || state.claim_history_load_state != ActivityClaimHistoryLoadState::Failed
                    || activity.claim_history.loading
                    || !state.accepts_active_main_world(entry.as_deref(), session.as_deref())
                {
                    continue;
                }
                match state.claim_history_retry_mode {
                    Some(ActivityClaimHistoryRetryMode::FirstPage) => {
                        request_first_claim_history_page(&mut state, &mut myserver_commands);
                    }
                    Some(ActivityClaimHistoryRetryMode::LoadMore) => {
                        request_next_claim_history_page(
                            &mut state,
                            &activity,
                            &mut myserver_commands,
                        );
                    }
                    None => {}
                }
            }
            ActivityScreenCommand::LoadMoreClaimHistory => {
                if !state.claim_history_open
                    || activity.claim_history.loading
                    || !activity.claim_history.has_more
                    || activity.claim_history.display_limit_reached
                    || activity.claim_history.records.len() >= MAX_ACTIVITY_HISTORY_CLIENT_RECORDS
                    || matches!(
                        state.claim_history_load_state,
                        ActivityClaimHistoryLoadState::LoadingFirstPage
                            | ActivityClaimHistoryLoadState::LoadingMore
                    )
                    || !state.accepts_active_main_world(entry.as_deref(), session.as_deref())
                {
                    continue;
                }
                request_next_claim_history_page(&mut state, &activity, &mut myserver_commands);
            }
            ActivityScreenCommand::CloseClaimHistory => {
                if !state.claim_history_open {
                    continue;
                }
                state.close_claim_history();
                screen_commands.write(DeclarativeScreenHostCommand::CloseRoute {
                    route: ACTIVITY_CLAIM_HISTORY_ROUTE.to_owned(),
                });
                clear_focus(&mut focus);
            }
            ActivityScreenCommand::CloseList => {
                if !state.list_open && !state.detail_open && !state.claim_history_open {
                    continue;
                }
                state.reset_closed();
                close_activity_routes(&mut screen_commands);
                clear_focus(&mut focus);
            }
        }
    }
}

pub(super) fn apply_activity_screen_events(
    mut events: MessageReader<ActivityEvent>,
    activity: Res<ActivityClientState>,
    mut state: ResMut<ActivityScreenState>,
    mut screen_commands: MessageWriter<DeclarativeScreenHostCommand>,
    mut focus: Option<ResMut<UiFocusState>>,
) {
    for event in events.read() {
        match event {
            ActivityEvent::ScopeChanged => {
                if state.list_open || state.detail_open || state.claim_history_open {
                    state.reset_closed();
                    close_activity_routes(&mut screen_commands);
                    clear_focus(&mut focus);
                }
            }
            ActivityEvent::ListUpdated if state.list_open => {
                state.list_load_state = ActivityListLoadState::Ready;
                state.list_error = None;
                if state.detail_open
                    && state.selected.as_ref().is_some_and(|selected| {
                        !activity.activities.iter().any(|summary| {
                            visible_activity(summary)
                                && summary.activity_id == selected.activity_id
                                && summary.version == selected.version
                        })
                    })
                {
                    let detail_route = state.detail_route.unwrap_or(ACTIVITY_DETAIL_ROUTE);
                    state.close_detail();
                    screen_commands.write(DeclarativeScreenHostCommand::CloseRoute {
                        route: detail_route.to_owned(),
                    });
                    clear_focus(&mut focus);
                }
            }
            ActivityEvent::DetailUpdated {
                activity_id,
                version,
            } if state.detail_open
                && state.selected.as_ref().is_some_and(|selected| {
                    selected.activity_id == *activity_id && selected.version == *version
                }) =>
            {
                state.detail_load_state = ActivityDetailLoadState::Ready;
                state.detail_error = None;
            }
            ActivityEvent::ClaimHistoryUpdated { .. } if state.claim_history_open => {
                state.claim_history_load_state = if activity.claim_history.records.is_empty() {
                    ActivityClaimHistoryLoadState::Empty
                } else {
                    ActivityClaimHistoryLoadState::Ready
                };
                state.claim_history_retry_mode = None;
                state.claim_history_error = None;
            }
            ActivityEvent::RequestFailed {
                kind,
                activity: failed_activity,
                error,
            } => match kind {
                ActivityRequestKind::List if state.list_open => {
                    state.list_load_state = ActivityListLoadState::Failed;
                    state.list_error = Some(error.clone());
                }
                ActivityRequestKind::Detail
                    if state.detail_open
                        && failed_activity
                            .as_ref()
                            .is_none_or(|activity| state.selected.as_ref() == Some(activity)) =>
                {
                    state.detail_load_state = ActivityDetailLoadState::Failed;
                    state.detail_error = Some(error.clone());
                }
                ActivityRequestKind::History if state.claim_history_open => {
                    state.claim_history_load_state = ActivityClaimHistoryLoadState::Failed;
                    state.claim_history_error = Some(error.clone());
                }
                _ => {}
            },
            _ => {}
        }
    }
}

pub(super) fn track_activity_screen_host_events(
    mut events: MessageReader<DeclarativeScreenHostEvent>,
    mut state: ResMut<ActivityScreenState>,
) {
    for event in events.read() {
        match event {
            DeclarativeScreenHostEvent::Closed { route, .. }
                if matches!(
                    route.as_str(),
                    ACTIVITY_DETAIL_ROUTE | LOGIN_REWARD_DETAIL_ROUTE | LOTTERY_DETAIL_ROUTE
                ) =>
            {
                state.close_detail();
            }
            DeclarativeScreenHostEvent::Closed { route, .. }
                if route == ACTIVITY_CLAIM_HISTORY_ROUTE =>
            {
                state.close_claim_history();
            }
            DeclarativeScreenHostEvent::Closed { route, .. } if route == ACTIVITY_LIST_ROUTE => {
                state.reset_closed();
            }
            _ => {}
        }
    }
}

fn close_activity_routes(commands: &mut MessageWriter<DeclarativeScreenHostCommand>) {
    for route in [
        LOTTERY_DETAIL_ROUTE,
        LOGIN_REWARD_DETAIL_ROUTE,
        ACTIVITY_DETAIL_ROUTE,
        ACTIVITY_CLAIM_HISTORY_ROUTE,
        ACTIVITY_LIST_ROUTE,
    ] {
        commands.write(DeclarativeScreenHostCommand::CloseRoute {
            route: route.to_owned(),
        });
    }
}

fn request_first_claim_history_page(
    state: &mut ActivityScreenState,
    commands: &mut MessageWriter<MyServerCommand>,
) {
    state.claim_history_load_state = ActivityClaimHistoryLoadState::LoadingFirstPage;
    state.claim_history_retry_mode = Some(ActivityClaimHistoryRetryMode::FirstPage);
    state.claim_history_error = None;
    commands.write(MyServerCommand::LoadActivityClaimHistory {
        cursor: None,
        limit: DEFAULT_ACTIVITY_HISTORY_PAGE_SIZE,
    });
}

fn request_next_claim_history_page(
    state: &mut ActivityScreenState,
    activity: &ActivityClientState,
    commands: &mut MessageWriter<MyServerCommand>,
) {
    if activity.claim_history.display_limit_reached
        || activity.claim_history.records.len() >= MAX_ACTIVITY_HISTORY_CLIENT_RECORDS
    {
        return;
    }
    let Some(cursor) = activity.claim_history.next_cursor.clone() else {
        return;
    };
    state.claim_history_load_state = ActivityClaimHistoryLoadState::LoadingMore;
    state.claim_history_retry_mode = Some(ActivityClaimHistoryRetryMode::LoadMore);
    state.claim_history_error = None;
    commands.write(MyServerCommand::LoadActivityClaimHistory {
        cursor: Some(cursor),
        limit: DEFAULT_ACTIVITY_HISTORY_PAGE_SIZE,
    });
}

fn detail_route_for_activity(
    key: &ActivityKey,
    activity: &ActivityClientState,
) -> Option<&'static str> {
    activity
        .activities
        .iter()
        .find(|summary| summary.activity_id == key.activity_id && summary.version == key.version)
        .map(|summary| match summary.activity_type.as_str() {
            "login_reward" => LOGIN_REWARD_DETAIL_ROUTE,
            "lottery" => LOTTERY_DETAIL_ROUTE,
            _ => ACTIVITY_DETAIL_ROUTE,
        })
}

fn clear_focus(focus: &mut Option<ResMut<UiFocusState>>) {
    if let Some(focus) = focus.as_deref_mut() {
        focus.focused_entity = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::network::ConnectionId;
    use bevy::ecs::message::MessageCursor;

    fn active_entry(generation: u64) -> MainWorldEntryState {
        MainWorldEntryState {
            generation,
            phase: MainWorldEntryPhase::Active,
            character_id: Some("character-1".to_owned()),
            ..Default::default()
        }
    }

    fn active_session() -> MyServerSession {
        MyServerSession {
            character_id: Some("character-1".to_owned()),
            connection_id: Some(ConnectionId::from_raw(7)),
            connected: true,
            authenticated: true,
            game_connection_state: GameConnectionState::Authenticated,
            ..Default::default()
        }
    }

    fn test_app(entry: MainWorldEntryState, session: MyServerSession) -> App {
        let mut app = App::new();
        app.insert_resource(entry)
            .insert_resource(session)
            .init_resource::<ActivityClientState>()
            .init_resource::<ActivityScreenState>()
            .add_message::<ActivityScreenCommand>()
            .add_message::<ActivityEvent>()
            .add_message::<MyServerCommand>()
            .add_message::<DeclarativeScreenHostCommand>()
            .add_systems(
                Update,
                (
                    handle_activity_screen_commands,
                    apply_activity_screen_events,
                ),
            );
        app
    }

    fn read_messages<M>(app: &App) -> Vec<M>
    where
        M: Message + Clone,
    {
        let messages = app.world().resource::<Messages<M>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    #[test]
    fn opening_list_requests_only_the_activity_summary() {
        let mut app = test_app(active_entry(4), active_session());
        app.world_mut()
            .write_message(ActivityScreenCommand::OpenList);
        app.update();

        assert!(matches!(
            read_messages::<MyServerCommand>(&app).as_slice(),
            [MyServerCommand::LoadActivityList]
        ));
        assert!(matches!(
            read_messages::<DeclarativeScreenHostCommand>(&app).as_slice(),
            [DeclarativeScreenHostCommand::OpenDetachedRoute { route }]
                if route == ACTIVITY_LIST_ROUTE
        ));
        let state = app.world().resource::<ActivityScreenState>();
        assert!(state.list_open);
        assert!(!state.detail_open);
        assert_eq!(state.main_world_generation, Some(4));
    }

    #[test]
    fn stale_main_world_generation_cannot_open_an_activity_detail() {
        let mut app = test_app(active_entry(9), active_session());
        {
            let mut state = app.world_mut().resource_mut::<ActivityScreenState>();
            state.main_world_generation = Some(8);
            state.list_open = true;
        }
        app.world_mut()
            .resource_mut::<ActivityClientState>()
            .activities
            .push(ActivitySummary {
                activity_id: "activity-1".to_owned(),
                version: 1,
                activity_type: "lottery".to_owned(),
                status: "running".to_owned(),
                start_at_ms: 1,
                end_at_ms: 2,
                claim_deadline_ms: 3,
                timezone: "UTC".to_owned(),
            });
        app.world_mut()
            .write_message(ActivityScreenCommand::OpenDetail {
                activity_id: "activity-1".to_owned(),
                version: 1,
            });
        app.update();

        assert!(read_messages::<MyServerCommand>(&app).is_empty());
        assert!(read_messages::<DeclarativeScreenHostCommand>(&app).is_empty());
    }

    #[test]
    fn old_detail_failure_does_not_replace_the_current_detail_state() {
        let mut app = test_app(active_entry(9), active_session());
        let current = ActivityKey {
            activity_id: "lottery".to_owned(),
            version: 2,
        };
        {
            let mut state = app.world_mut().resource_mut::<ActivityScreenState>();
            state.main_world_generation = Some(9);
            state.list_open = true;
            state.detail_open = true;
            state.detail_route = Some(LOTTERY_DETAIL_ROUTE);
            state.selected = Some(current);
            state.detail_load_state = ActivityDetailLoadState::Ready;
        }
        app.world_mut().write_message(ActivityEvent::RequestFailed {
            kind: ActivityRequestKind::Detail,
            activity: Some(ActivityKey {
                activity_id: "login-reward".to_owned(),
                version: 1,
            }),
            error: ActivityError {
                kind: crate::game::myserver::ActivityErrorKind::Transport,
                error_code: "ACTIVITY_TRANSPORT_FAILURE".to_owned(),
                message_key: "activity.error.transport",
                retryable: true,
            },
        });
        app.update();

        let state = app.world().resource::<ActivityScreenState>();
        assert_eq!(state.detail_load_state, ActivityDetailLoadState::Ready);
        assert!(state.detail_error.is_none());
    }

    #[test]
    fn login_reward_uses_its_dedicated_fixed_detail_route() {
        let mut app = test_app(active_entry(9), active_session());
        {
            let mut state = app.world_mut().resource_mut::<ActivityScreenState>();
            state.main_world_generation = Some(9);
            state.list_open = true;
        }
        app.world_mut()
            .resource_mut::<ActivityClientState>()
            .activities
            .push(ActivitySummary {
                activity_id: "login-reward".to_owned(),
                version: 2,
                activity_type: "login_reward".to_owned(),
                status: "running".to_owned(),
                start_at_ms: 1,
                end_at_ms: 2,
                claim_deadline_ms: 3,
                timezone: "UTC".to_owned(),
            });
        app.world_mut()
            .write_message(ActivityScreenCommand::OpenDetail {
                activity_id: "login-reward".to_owned(),
                version: 2,
            });
        app.update();

        assert!(matches!(
            read_messages::<DeclarativeScreenHostCommand>(&app).as_slice(),
            [DeclarativeScreenHostCommand::OpenDetachedRoute { route }]
                if route == LOGIN_REWARD_DETAIL_ROUTE
        ));
        assert_eq!(
            app.world().resource::<ActivityScreenState>().detail_route,
            Some(LOGIN_REWARD_DETAIL_ROUTE)
        );
    }

    #[test]
    fn lottery_uses_its_dedicated_fixed_detail_route() {
        let mut app = test_app(active_entry(9), active_session());
        {
            let mut state = app.world_mut().resource_mut::<ActivityScreenState>();
            state.main_world_generation = Some(9);
            state.list_open = true;
        }
        app.world_mut()
            .resource_mut::<ActivityClientState>()
            .activities
            .push(ActivitySummary {
                activity_id: "lottery".to_owned(),
                version: 2,
                activity_type: "lottery".to_owned(),
                status: "running".to_owned(),
                start_at_ms: 1,
                end_at_ms: 2,
                claim_deadline_ms: 3,
                timezone: "UTC".to_owned(),
            });
        app.world_mut()
            .write_message(ActivityScreenCommand::OpenDetail {
                activity_id: "lottery".to_owned(),
                version: 2,
            });
        app.update();

        assert!(matches!(
            read_messages::<DeclarativeScreenHostCommand>(&app).as_slice(),
            [DeclarativeScreenHostCommand::OpenDetachedRoute { route }]
                if route == LOTTERY_DETAIL_ROUTE
        ));
        assert_eq!(
            app.world().resource::<ActivityScreenState>().detail_route,
            Some(LOTTERY_DETAIL_ROUTE)
        );
    }

    #[test]
    fn retrying_a_failed_list_reuses_the_list_route_without_requesting_detail() {
        let mut app = test_app(active_entry(3), active_session());
        {
            let mut state = app.world_mut().resource_mut::<ActivityScreenState>();
            state.main_world_generation = Some(3);
            state.list_open = true;
            state.list_load_state = ActivityListLoadState::Failed;
        }
        app.world_mut()
            .write_message(ActivityScreenCommand::RetryList);
        app.update();

        assert!(matches!(
            read_messages::<MyServerCommand>(&app).as_slice(),
            [MyServerCommand::LoadActivityList]
        ));
        assert!(read_messages::<DeclarativeScreenHostCommand>(&app).is_empty());
        assert_eq!(
            app.world()
                .resource::<ActivityScreenState>()
                .list_load_state,
            ActivityListLoadState::Loading
        );
    }

    #[test]
    fn opening_claim_history_requests_only_the_first_history_page() {
        let mut app = test_app(active_entry(6), active_session());
        {
            let mut state = app.world_mut().resource_mut::<ActivityScreenState>();
            state.main_world_generation = Some(6);
            state.list_open = true;
            state.list_load_state = ActivityListLoadState::Ready;
        }
        app.world_mut()
            .write_message(ActivityScreenCommand::OpenClaimHistory);
        app.update();

        assert!(matches!(
            read_messages::<MyServerCommand>(&app).as_slice(),
            [MyServerCommand::LoadActivityClaimHistory { cursor: None, limit }]
                if *limit == DEFAULT_ACTIVITY_HISTORY_PAGE_SIZE
        ));
        assert!(matches!(
            read_messages::<DeclarativeScreenHostCommand>(&app).as_slice(),
            [DeclarativeScreenHostCommand::OpenDetachedRoute { route }]
                if route == ACTIVITY_CLAIM_HISTORY_ROUTE
        ));
        let state = app.world().resource::<ActivityScreenState>();
        assert!(state.list_open);
        assert!(state.claim_history_open);
        assert_eq!(
            state.claim_history_load_state,
            ActivityClaimHistoryLoadState::LoadingFirstPage
        );
    }

    #[test]
    fn claim_history_load_more_is_deduplicated_and_keeps_the_cursor_out_of_screen_state() {
        let mut app = test_app(active_entry(6), active_session());
        {
            let mut activity = app.world_mut().resource_mut::<ActivityClientState>();
            activity.claim_history.has_more = true;
            activity.claim_history.next_cursor = Some("opaque-next-page".to_owned());
        }
        {
            let mut state = app.world_mut().resource_mut::<ActivityScreenState>();
            state.main_world_generation = Some(6);
            state.list_open = true;
            state.claim_history_open = true;
            state.claim_history_load_state = ActivityClaimHistoryLoadState::Ready;
        }
        app.world_mut()
            .write_message(ActivityScreenCommand::LoadMoreClaimHistory);
        app.world_mut()
            .write_message(ActivityScreenCommand::LoadMoreClaimHistory);
        app.update();

        assert_eq!(read_messages::<MyServerCommand>(&app).len(), 1);
        assert!(matches!(
            read_messages::<MyServerCommand>(&app).as_slice(),
            [MyServerCommand::LoadActivityClaimHistory {
                cursor: Some(_),
                limit,
            }] if *limit == DEFAULT_ACTIVITY_HISTORY_PAGE_SIZE
        ));
        assert_eq!(
            app.world()
                .resource::<ActivityScreenState>()
                .claim_history_load_state,
            ActivityClaimHistoryLoadState::LoadingMore
        );
    }

    #[test]
    fn claim_history_load_more_at_the_display_cap_sends_no_command() {
        let mut app = test_app(active_entry(6), active_session());
        {
            let mut activity = app.world_mut().resource_mut::<ActivityClientState>();
            activity.claim_history.records = (0..MAX_ACTIVITY_HISTORY_CLIENT_RECORDS)
                .map(|index| crate::game::myserver::ActivityClaimHistoryRecord {
                    activity_id: format!("activity-{index}"),
                    version: 1,
                    activity_type: "lottery".to_owned(),
                    action_type: "draw".to_owned(),
                    stage_id: None,
                    created_at_ms: index as i64 + 1,
                    completed_at_ms: None,
                    status: crate::game::myserver::ActivityClaimHistoryStatus::Granted,
                    rewards: Vec::new(),
                })
                .collect();
            activity.claim_history.has_more = true;
            activity.claim_history.next_cursor = Some("opaque-next-page".to_owned());
        }
        {
            let mut state = app.world_mut().resource_mut::<ActivityScreenState>();
            state.main_world_generation = Some(6);
            state.list_open = true;
            state.claim_history_open = true;
            state.claim_history_load_state = ActivityClaimHistoryLoadState::Ready;
        }
        app.world_mut()
            .write_message(ActivityScreenCommand::LoadMoreClaimHistory);
        app.update();

        assert!(read_messages::<MyServerCommand>(&app).is_empty());
        assert_eq!(
            app.world()
                .resource::<ActivityScreenState>()
                .claim_history_load_state,
            ActivityClaimHistoryLoadState::Ready
        );
    }

    #[test]
    fn closing_claim_history_returns_to_the_existing_list_without_reloading_it() {
        let mut app = test_app(active_entry(6), active_session());
        {
            let mut state = app.world_mut().resource_mut::<ActivityScreenState>();
            state.main_world_generation = Some(6);
            state.list_open = true;
            state.list_load_state = ActivityListLoadState::Ready;
            state.claim_history_open = true;
            state.claim_history_load_state = ActivityClaimHistoryLoadState::Ready;
        }
        app.world_mut()
            .write_message(ActivityScreenCommand::CloseClaimHistory);
        app.update();

        assert!(read_messages::<MyServerCommand>(&app).is_empty());
        assert!(matches!(
            read_messages::<DeclarativeScreenHostCommand>(&app).as_slice(),
            [DeclarativeScreenHostCommand::CloseRoute { route }]
                if route == ACTIVITY_CLAIM_HISTORY_ROUTE
        ));
        let state = app.world().resource::<ActivityScreenState>();
        assert!(state.list_open);
        assert!(!state.claim_history_open);
        assert_eq!(state.list_load_state, ActivityListLoadState::Ready);
    }

    #[test]
    fn empty_claim_history_response_uses_the_repeat_empty_rendering_path() {
        let mut app = test_app(active_entry(6), active_session());
        {
            let mut state = app.world_mut().resource_mut::<ActivityScreenState>();
            state.main_world_generation = Some(6);
            state.list_open = true;
            state.claim_history_open = true;
            state.claim_history_load_state = ActivityClaimHistoryLoadState::LoadingFirstPage;
        }
        app.world_mut()
            .write_message(ActivityEvent::ClaimHistoryUpdated { appended: false });
        app.update();

        let state = app.world().resource::<ActivityScreenState>();
        assert_eq!(
            state.claim_history_load_state,
            ActivityClaimHistoryLoadState::Empty
        );
        assert_eq!(
            state
                .claim_history_load_state
                .collection_binding_value(false),
            "ready"
        );
    }

    #[test]
    fn claim_history_load_more_failure_retries_the_existing_page_without_dropping_records() {
        let mut app = test_app(active_entry(6), active_session());
        {
            let mut activity = app.world_mut().resource_mut::<ActivityClientState>();
            activity.claim_history.records.push(
                crate::game::myserver::ActivityClaimHistoryRecord {
                    activity_id: "activity-1".to_owned(),
                    version: 1,
                    activity_type: "lottery".to_owned(),
                    action_type: "draw".to_owned(),
                    stage_id: None,
                    created_at_ms: 1,
                    completed_at_ms: Some(2),
                    status: crate::game::myserver::ActivityClaimHistoryStatus::Granted,
                    rewards: Vec::new(),
                },
            );
            activity.claim_history.has_more = true;
            activity.claim_history.next_cursor = Some("opaque-next-page".to_owned());
        }
        {
            let mut state = app.world_mut().resource_mut::<ActivityScreenState>();
            state.main_world_generation = Some(6);
            state.list_open = true;
            state.claim_history_open = true;
            state.claim_history_load_state = ActivityClaimHistoryLoadState::Ready;
        }
        app.world_mut()
            .write_message(ActivityScreenCommand::LoadMoreClaimHistory);
        app.update();
        app.world_mut().write_message(ActivityEvent::RequestFailed {
            kind: ActivityRequestKind::History,
            activity: None,
            error: ActivityError {
                kind: crate::game::myserver::ActivityErrorKind::Transport,
                error_code: "ACTIVITY_TRANSPORT_FAILURE".to_owned(),
                message_key: "activity.error.transport",
                retryable: true,
            },
        });
        app.update();

        assert_eq!(
            app.world()
                .resource::<ActivityScreenState>()
                .claim_history_load_state,
            ActivityClaimHistoryLoadState::Failed
        );
        assert_eq!(
            app.world()
                .resource::<ActivityScreenState>()
                .claim_history_retry_mode,
            Some(ActivityClaimHistoryRetryMode::LoadMore)
        );
        app.world_mut()
            .write_message(ActivityScreenCommand::RetryClaimHistory);
        app.update();

        let state = app.world().resource::<ActivityScreenState>();
        assert_eq!(
            state.claim_history_load_state,
            ActivityClaimHistoryLoadState::LoadingMore
        );
        assert!(state.claim_history_error.is_none());
        assert_eq!(
            app.world()
                .resource::<ActivityClientState>()
                .claim_history
                .records
                .len(),
            1
        );
    }

    #[test]
    fn claim_history_promotion_registration_matches_fixed_host_contract() {
        const REGISTRATION_SOURCE: &str = include_str!(
            "../../../../assets/ui/documents/approved/activity/main_world_activity_claim_history.promotion.v1.json"
        );
        let contract = ActivityScreenHostContract::default();
        let host = activity_claim_history_declarative_screen_host(&contract);
        let registration = crate::framework::ui::document::parse_approved_document_registration(
            REGISTRATION_SOURCE,
        )
        .unwrap();
        let audit = registration
            .audit_report(ACTIVITY_CLAIM_HISTORY_DOCUMENT_SOURCE)
            .unwrap();

        assert_eq!(
            host.document_id.as_str(),
            ACTIVITY_CLAIM_HISTORY_DOCUMENT_ID
        );
        assert_eq!(host.owner, OWNER_MAIN_WORLD_ACTIVITY_CLAIM_HISTORY);
        assert_eq!(host.route, ACTIVITY_CLAIM_HISTORY_ROUTE);
        assert_eq!(host.action_allowlist.len(), 4);
        assert_eq!(
            registration.owner(),
            OWNER_MAIN_WORLD_ACTIVITY_CLAIM_HISTORY.as_str()
        );
        assert_eq!(registration.route(), ACTIVITY_CLAIM_HISTORY_ROUTE);
        assert_eq!(audit.actions.len(), 4);
        let owner_binding_count = contract
            .claim_history_bindings
            .values()
            .filter(|binding| {
                binding.scope == crate::framework::ui::document::UiBindingScope::Owner
            })
            .count();
        assert_eq!(host.binding_schema.len(), owner_binding_count);
        assert_eq!(audit.bindings.len(), owner_binding_count);
    }

    #[test]
    fn activity_list_promotion_registration_keeps_the_claim_history_entry() {
        const REGISTRATION_SOURCE: &str = include_str!(
            "../../../../assets/ui/documents/approved/activity/main_world_activity_list.promotion.v1.json"
        );
        let contract = ActivityScreenHostContract::default();
        let host = activity_list_declarative_screen_host(&contract);
        let registration = crate::framework::ui::document::parse_approved_document_registration(
            REGISTRATION_SOURCE,
        )
        .unwrap();
        let audit = registration
            .audit_report(ACTIVITY_LIST_DOCUMENT_SOURCE)
            .unwrap();

        assert_eq!(host.action_allowlist.len(), 4);
        assert_eq!(audit.actions.len(), 4);
        let owner_binding_count = contract
            .list_bindings
            .values()
            .filter(|binding| {
                binding.scope == crate::framework::ui::document::UiBindingScope::Owner
            })
            .count();
        assert_eq!(host.binding_schema.len(), owner_binding_count);
        assert_eq!(audit.bindings.len(), owner_binding_count);
    }

    #[test]
    fn approved_activity_documents_parse_with_blocking_scroll_surfaces() {
        use crate::framework::ui::document::UiDocument;

        for source in [
            ACTIVITY_LIST_DOCUMENT_SOURCE,
            ACTIVITY_DETAIL_DOCUMENT_SOURCE,
            ACTIVITY_CLAIM_HISTORY_DOCUMENT_SOURCE,
            LOGIN_REWARD_DETAIL_DOCUMENT_SOURCE,
            LOTTERY_DETAIL_DOCUMENT_SOURCE,
        ] {
            UiDocument::parse_and_validate_json(source)
                .expect("activity surface must remain an approved valid UiDocument");
            assert!(source.contains("\"block_lower\": true"));
        }
    }

    #[test]
    fn activity_list_and_claim_history_documents_resolve_in_packaged_i18n_catalogs() {
        use std::path::Path;

        use crate::framework::ui::{document::UiDocument, i18n::load_ui_i18n_catalog_for_test};

        for locale in ["en_us", "zh_cn"] {
            let catalog_path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets/ui/i18n")
                .join(format!("{locale}.ron"));
            let catalog = load_ui_i18n_catalog_for_test(&catalog_path)
                .unwrap_or_else(|error| panic!("{locale} catalog should load: {error}"));

            for (document_id, source) in [
                (ACTIVITY_LIST_DOCUMENT_ID, ACTIVITY_LIST_DOCUMENT_SOURCE),
                (
                    ACTIVITY_CLAIM_HISTORY_DOCUMENT_ID,
                    ACTIVITY_CLAIM_HISTORY_DOCUMENT_SOURCE,
                ),
            ] {
                let document = UiDocument::parse_and_validate_json(source)
                    .expect("activity surface must remain an approved valid UiDocument");
                let errors = document.document().validate_content_with_catalog(&catalog);
                assert!(
                    errors.is_empty(),
                    "{document_id} must resolve every i18n key in the packaged {locale} catalog: {errors:?}"
                );
            }
        }
    }

    #[test]
    fn activity_hosts_match_the_runtime_binding_contract() {
        use crate::game::declarative_screen::validate_host_source_for_test;

        let contract = ActivityScreenHostContract::default();
        let mut actions = UiActionRegistry::default();
        for descriptor in list::activity_list_action_descriptors()
            .into_iter()
            .chain(detail::activity_detail_action_descriptors())
            .chain(claim_history::activity_claim_history_action_descriptors())
            .chain(login_reward::login_reward_action_descriptors())
            .chain(lottery::lottery_action_descriptors())
        {
            actions
                .register(descriptor)
                .expect("activity action registration must be valid and unique");
        }

        for host in [
            activity_list_declarative_screen_host(&contract),
            activity_detail_declarative_screen_host(&contract),
            activity_claim_history_declarative_screen_host(&contract),
            login_reward_detail_declarative_screen_host(&contract),
            lottery_detail_declarative_screen_host(&contract),
        ] {
            validate_host_source_for_test(&host, &host.source.source_json, &actions)
                .unwrap_or_else(|cause| {
                    panic!(
                        "{} must match the declarative runtime binding contract: {cause}",
                        host.document_id
                    )
                });
        }
    }
}
