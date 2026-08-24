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
        ActivitySummary, GameConnectionState, MyServerCommand, MyServerSession,
    },
    scenes::main_world_entry::{MainWorldEntryPhase, MainWorldEntryState},
    ui_ids::{OWNER_MAIN_WORLD_ACTIVITY_DETAIL, OWNER_MAIN_WORLD_ACTIVITY_LIST},
};

use super::{detail, list};

pub(in crate::game) const ACTIVITY_LIST_DOCUMENT_ID: &str = "game.main_world_activity_list";
pub(in crate::game) const ACTIVITY_DETAIL_DOCUMENT_ID: &str = "game.main_world_activity_detail";
pub(in crate::game) const ACTIVITY_LIST_ROUTE: &str = "main_world_activity_list";
pub(in crate::game) const ACTIVITY_DETAIL_ROUTE: &str = "main_world_activity_detail";

pub(super) const ACTIVITY_LIST_DOCUMENT_SOURCE: &str = include_str!(
    "../../../../assets/ui/documents/approved/activity/main_world_activity_list.v1.json"
);
pub(super) const ACTIVITY_DETAIL_DOCUMENT_SOURCE: &str = include_str!(
    "../../../../assets/ui/documents/approved/activity/main_world_activity_detail.v1.json"
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

#[derive(Clone, Debug, Default, Resource)]
pub(in crate::game) struct ActivityScreenState {
    pub(super) main_world_generation: Option<u64>,
    pub(super) list_open: bool,
    pub(super) detail_open: bool,
    pub(super) selected: Option<ActivityKey>,
    pub(super) list_load_state: ActivityListLoadState,
    pub(super) detail_load_state: ActivityDetailLoadState,
    pub(super) list_error: Option<ActivityError>,
    pub(super) detail_error: Option<ActivityError>,
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
        self.selected = None;
        self.list_load_state = ActivityListLoadState::Idle;
        self.detail_load_state = ActivityDetailLoadState::Idle;
        self.list_error = None;
        self.detail_error = None;
    }

    fn close_detail(&mut self) {
        self.detail_open = false;
        self.selected = None;
        self.detail_load_state = ActivityDetailLoadState::Idle;
        self.detail_error = None;
    }
}

#[derive(Resource)]
pub(super) struct ActivityScreenHostContract {
    pub(super) list_bindings: BTreeMap<UiBindingPath, UiBindingDeclaration>,
    pub(super) detail_bindings: BTreeMap<UiBindingPath, UiBindingDeclaration>,
}

impl Default for ActivityScreenHostContract {
    fn default() -> Self {
        Self {
            list_bindings: list::activity_list_binding_schema(),
            detail_bindings: detail::activity_detail_binding_schema(),
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
    {
        actions
            .register(descriptor)
            .expect("activity action registration must be valid and unique");
    }
    for host in [
        activity_list_declarative_screen_host(&contract),
        activity_detail_declarative_screen_host(&contract),
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
            list::ACTION_ACTIVITY_LIST_CLOSE,
        ],
        &contract.list_bindings,
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
        let was_open = state.list_open || state.detail_open;
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
                state.selected = None;
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
                if state.detail_open {
                    screen_commands.write(DeclarativeScreenHostCommand::CloseRoute {
                        route: ACTIVITY_DETAIL_ROUTE.to_owned(),
                    });
                }
                state.detail_open = true;
                state.selected = Some(key.clone());
                state.detail_load_state = ActivityDetailLoadState::Loading;
                state.detail_error = None;
                screen_commands.write(DeclarativeScreenHostCommand::OpenDetachedRoute {
                    route: ACTIVITY_DETAIL_ROUTE.to_owned(),
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
                state.close_detail();
                screen_commands.write(DeclarativeScreenHostCommand::CloseRoute {
                    route: ACTIVITY_DETAIL_ROUTE.to_owned(),
                });
                clear_focus(&mut focus);
            }
            ActivityScreenCommand::CloseList => {
                if !state.list_open && !state.detail_open {
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
                if state.list_open || state.detail_open {
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
                    state.close_detail();
                    screen_commands.write(DeclarativeScreenHostCommand::CloseRoute {
                        route: ACTIVITY_DETAIL_ROUTE.to_owned(),
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
            ActivityEvent::RequestFailed { kind, error } => match kind {
                ActivityRequestKind::List if state.list_open => {
                    state.list_load_state = ActivityListLoadState::Failed;
                    state.list_error = Some(error.clone());
                }
                ActivityRequestKind::Detail if state.detail_open => {
                    state.detail_load_state = ActivityDetailLoadState::Failed;
                    state.detail_error = Some(error.clone());
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
            DeclarativeScreenHostEvent::Closed { route, .. } if route == ACTIVITY_DETAIL_ROUTE => {
                state.close_detail();
            }
            DeclarativeScreenHostEvent::Closed { route, .. } if route == ACTIVITY_LIST_ROUTE => {
                state.reset_closed();
            }
            _ => {}
        }
    }
}

fn close_activity_routes(commands: &mut MessageWriter<DeclarativeScreenHostCommand>) {
    for route in [ACTIVITY_DETAIL_ROUTE, ACTIVITY_LIST_ROUTE] {
        commands.write(DeclarativeScreenHostCommand::CloseRoute {
            route: route.to_owned(),
        });
    }
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
            .add_message::<MyServerCommand>()
            .add_message::<DeclarativeScreenHostCommand>()
            .add_systems(Update, handle_activity_screen_commands);
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
    fn approved_activity_documents_parse_with_blocking_scroll_surfaces() {
        use crate::framework::ui::document::UiDocument;

        for source in [
            ACTIVITY_LIST_DOCUMENT_SOURCE,
            ACTIVITY_DETAIL_DOCUMENT_SOURCE,
        ] {
            UiDocument::parse_and_validate_json(source)
                .expect("activity surface must remain an approved valid UiDocument");
            assert!(source.contains("\"block_lower\": true"));
        }
    }
}
