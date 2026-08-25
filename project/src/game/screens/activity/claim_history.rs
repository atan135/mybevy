use std::{collections::BTreeMap, str::FromStr};

use bevy::prelude::*;

use crate::framework::ui::{
    core::binding::UiBindingValues,
    document::{
        UiActionDescriptor, UiActionDispatch, UiActionId, UiBindingDeclaration,
        UiBindingMissingBehavior, UiBindingPath, UiBindingScope, UiBindingType, UiBindingValue,
        UiBindingVisibility, UiDocumentId, UiNodeId, UiRegisteredActionKind,
    },
};
use crate::game::{
    myserver::{
        ActivityClaimHistoryRecord, ActivityClaimHistoryStatus, ActivityClientState, ActivityError,
        MAX_ACTIVITY_HISTORY_CLIENT_RECORDS, MyServerSession,
    },
    scenes::main_world_entry::MainWorldEntryState,
    ui_ids::OWNER_MAIN_WORLD_ACTIVITY_CLAIM_HISTORY,
};

use super::{
    ActivityScreenCommand, ActivityScreenState,
    host::{ACTIVITY_CLAIM_HISTORY_DOCUMENT_ID, ActivityClaimHistoryLoadState},
};

pub(super) const ACTION_ACTIVITY_CLAIM_HISTORY_REFRESH: &str =
    "main_world_activity_claim_history.refresh";
pub(super) const ACTION_ACTIVITY_CLAIM_HISTORY_RETRY: &str =
    "main_world_activity_claim_history.retry";
pub(super) const ACTION_ACTIVITY_CLAIM_HISTORY_LOAD_MORE: &str =
    "main_world_activity_claim_history.load_more";
pub(super) const ACTION_ACTIVITY_CLAIM_HISTORY_CLOSE: &str =
    "main_world_activity_claim_history.close";

const REFRESH_NODE: &str = "main_world_activity_claim_history.refresh";
const RETRY_NODE: &str = "main_world_activity_claim_history.retry";
const LOAD_MORE_NODE: &str = "main_world_activity_claim_history.load_more";
const CLOSE_NODE: &str = "main_world_activity_claim_history.close";

pub(super) fn activity_claim_history_binding_schema()
-> BTreeMap<UiBindingPath, UiBindingDeclaration> {
    let item = UiBindingType::Record {
        fields: BTreeMap::from([
            ("row_key".to_owned(), UiBindingType::String),
            ("activity".to_owned(), UiBindingType::String),
            ("version".to_owned(), UiBindingType::Number),
            ("action".to_owned(), UiBindingType::String),
            ("time".to_owned(), UiBindingType::String),
            ("status".to_owned(), UiBindingType::String),
            ("rewards".to_owned(), UiBindingType::String),
        ]),
    };
    let mut bindings = binding_schema(vec![
        (
            "activity_claim_history.collection_state",
            UiBindingType::Enum {
                values: ["loading", "ready", "error"].map(str::to_owned).to_vec(),
            },
        ),
        (
            "activity_claim_history.items",
            UiBindingType::List {
                item: Box::new(item),
                max_items: MAX_ACTIVITY_HISTORY_CLIENT_RECORDS as u16,
            },
        ),
        ("activity_claim_history.status", UiBindingType::String),
        ("activity_claim_history.error", UiBindingType::String),
        (
            "activity_claim_history.error_visibility",
            UiBindingType::Visibility,
        ),
        (
            "activity_claim_history.refresh_disabled",
            UiBindingType::Bool,
        ),
        ("activity_claim_history.retry_disabled", UiBindingType::Bool),
        ("activity_claim_history.retry_loading", UiBindingType::Bool),
        (
            "activity_claim_history.load_more_disabled",
            UiBindingType::Bool,
        ),
        (
            "activity_claim_history.load_more_visibility",
            UiBindingType::Visibility,
        ),
        ("activity_claim_history.loading_more", UiBindingType::Bool),
    ]);
    for (path, value_type) in [
        ("activity_claim_history.item.row_key", UiBindingType::String),
        (
            "activity_claim_history.item.activity",
            UiBindingType::String,
        ),
        ("activity_claim_history.item.version", UiBindingType::Number),
        ("activity_claim_history.item.action", UiBindingType::String),
        ("activity_claim_history.item.time", UiBindingType::String),
        ("activity_claim_history.item.status", UiBindingType::String),
        ("activity_claim_history.item.rewards", UiBindingType::String),
    ] {
        bindings.insert(
            UiBindingPath::from_str(path).expect("claim history item binding path is static"),
            UiBindingDeclaration {
                scope: UiBindingScope::Item,
                value_type,
                default: None,
                missing: UiBindingMissingBehavior::UseConsumerFallback,
            },
        );
    }
    bindings
}

pub(super) fn activity_claim_history_action_descriptors() -> Vec<UiActionDescriptor> {
    let document_id = || UiDocumentId::from_str(ACTIVITY_CLAIM_HISTORY_DOCUMENT_ID).unwrap();
    [
        (ACTION_ACTIVITY_CLAIM_HISTORY_REFRESH, REFRESH_NODE),
        (ACTION_ACTIVITY_CLAIM_HISTORY_RETRY, RETRY_NODE),
        (ACTION_ACTIVITY_CLAIM_HISTORY_LOAD_MORE, LOAD_MORE_NODE),
        (ACTION_ACTIVITY_CLAIM_HISTORY_CLOSE, CLOSE_NODE),
    ]
    .into_iter()
    .map(|(action, source)| {
        UiActionDescriptor::new(
            UiActionId::from_str(action).unwrap(),
            document_id(),
            OWNER_MAIN_WORLD_ACTIVITY_CLAIM_HISTORY.as_str(),
            business_command(action),
        )
        .with_source(UiNodeId::from_str(source).unwrap())
    })
    .collect()
}

pub(super) fn handle_activity_claim_history_document_actions(
    mut actions: MessageReader<UiActionDispatch>,
    entry: Option<Res<MainWorldEntryState>>,
    session: Option<Res<MyServerSession>>,
    activity: Res<ActivityClientState>,
    state: Res<ActivityScreenState>,
    mut commands: MessageWriter<ActivityScreenCommand>,
) {
    if !state.claim_history_open
        || !state.accepts_active_main_world(entry.as_deref(), session.as_deref())
    {
        return;
    }
    for action in actions.read() {
        match action.action.as_str() {
            ACTION_ACTIVITY_CLAIM_HISTORY_REFRESH
                if matches_zero_param_action(
                    action,
                    ACTION_ACTIVITY_CLAIM_HISTORY_REFRESH,
                    REFRESH_NODE,
                ) && !activity.claim_history.loading =>
            {
                commands.write(ActivityScreenCommand::RefreshClaimHistory);
            }
            ACTION_ACTIVITY_CLAIM_HISTORY_RETRY
                if matches_zero_param_action(
                    action,
                    ACTION_ACTIVITY_CLAIM_HISTORY_RETRY,
                    RETRY_NODE,
                ) && !activity.claim_history.loading
                    && state.claim_history_load_state == ActivityClaimHistoryLoadState::Failed =>
            {
                commands.write(ActivityScreenCommand::RetryClaimHistory);
            }
            ACTION_ACTIVITY_CLAIM_HISTORY_LOAD_MORE
                if matches_zero_param_action(
                    action,
                    ACTION_ACTIVITY_CLAIM_HISTORY_LOAD_MORE,
                    LOAD_MORE_NODE,
                ) && !activity.claim_history.loading
                    && activity.claim_history.has_more
                    && activity.claim_history.next_cursor.is_some()
                    && !history_display_limit_reached(&activity) =>
            {
                commands.write(ActivityScreenCommand::LoadMoreClaimHistory);
            }
            ACTION_ACTIVITY_CLAIM_HISTORY_CLOSE
                if matches_zero_param_action(
                    action,
                    ACTION_ACTIVITY_CLAIM_HISTORY_CLOSE,
                    CLOSE_NODE,
                ) =>
            {
                commands.write(ActivityScreenCommand::CloseClaimHistory);
            }
            _ => {}
        }
    }
}

pub(super) fn sync_activity_claim_history_bindings(
    entry: Option<Res<MainWorldEntryState>>,
    session: Option<Res<MyServerSession>>,
    activity: Res<ActivityClientState>,
    state: Res<ActivityScreenState>,
    contract: Res<super::host::ActivityScreenHostContract>,
    mut values: ResMut<UiBindingValues>,
) {
    if !state.claim_history_open
        || !state.accepts_active_main_world(entry.as_deref(), session.as_deref())
    {
        return;
    }

    let has_records = !activity.claim_history.records.is_empty();
    let is_loading = activity.claim_history.loading
        || matches!(
            state.claim_history_load_state,
            ActivityClaimHistoryLoadState::LoadingFirstPage
                | ActivityClaimHistoryLoadState::LoadingMore
        );
    let display_limit_reached = history_display_limit_reached(&activity);
    let has_more = activity.claim_history.has_more
        && activity.claim_history.next_cursor.is_some()
        && !display_limit_reached;
    let error = state
        .claim_history_error
        .as_ref()
        .map(activity_error_message)
        .unwrap_or_default();
    let items = activity
        .claim_history
        .records
        .iter()
        .take(MAX_ACTIVITY_HISTORY_CLIENT_RECORDS)
        .enumerate()
        .map(|(index, record)| claim_history_item(record, index))
        .collect();

    for (path, value) in [
        (
            "activity_claim_history.collection_state",
            UiBindingValue::Enum(
                state
                    .claim_history_load_state
                    .collection_binding_value(has_records)
                    .to_owned(),
            ),
        ),
        ("activity_claim_history.items", UiBindingValue::List(items)),
        (
            "activity_claim_history.status",
            UiBindingValue::String(activity_claim_history_status(
                state.claim_history_load_state,
                activity.claim_history.records.len(),
                display_limit_reached,
            )),
        ),
        (
            "activity_claim_history.error",
            UiBindingValue::String(error),
        ),
        (
            "activity_claim_history.error_visibility",
            UiBindingValue::Visibility(if state.claim_history_error.is_some() {
                UiBindingVisibility::Visible
            } else {
                UiBindingVisibility::Hidden
            }),
        ),
        (
            "activity_claim_history.refresh_disabled",
            UiBindingValue::Bool(is_loading),
        ),
        (
            "activity_claim_history.retry_disabled",
            UiBindingValue::Bool(
                is_loading
                    || state.claim_history_load_state != ActivityClaimHistoryLoadState::Failed,
            ),
        ),
        (
            "activity_claim_history.retry_loading",
            UiBindingValue::Bool(is_loading),
        ),
        (
            "activity_claim_history.load_more_disabled",
            UiBindingValue::Bool(is_loading || !has_more),
        ),
        (
            "activity_claim_history.load_more_visibility",
            UiBindingValue::Visibility(if has_more || display_limit_reached {
                UiBindingVisibility::Visible
            } else {
                UiBindingVisibility::Hidden
            }),
        ),
        (
            "activity_claim_history.loading_more",
            UiBindingValue::Bool(
                state.claim_history_load_state == ActivityClaimHistoryLoadState::LoadingMore,
            ),
        ),
    ] {
        set_binding(&contract, &mut values, path, value);
    }
}

fn history_display_limit_reached(activity: &ActivityClientState) -> bool {
    activity.claim_history.display_limit_reached
        || activity.claim_history.records.len() >= MAX_ACTIVITY_HISTORY_CLIENT_RECORDS
}

fn claim_history_item(record: &ActivityClaimHistoryRecord, index: usize) -> UiBindingValue {
    UiBindingValue::Record(BTreeMap::from([
        (
            "row_key".to_owned(),
            UiBindingValue::String(index.to_string()),
        ),
        (
            "activity".to_owned(),
            UiBindingValue::String(format!(
                "{} ({})",
                activity_type_label(&record.activity_type),
                record.activity_id
            )),
        ),
        (
            "version".to_owned(),
            UiBindingValue::Number(record.version as f64),
        ),
        (
            "action".to_owned(),
            UiBindingValue::String(activity_action_label(record)),
        ),
        (
            "time".to_owned(),
            UiBindingValue::String(activity_claim_time(record)),
        ),
        (
            "status".to_owned(),
            UiBindingValue::String(activity_claim_history_status_label(record.status).to_owned()),
        ),
        (
            "rewards".to_owned(),
            UiBindingValue::String(activity_reward_summary(record)),
        ),
    ]))
}

fn binding_schema(
    specs: Vec<(&str, UiBindingType)>,
) -> BTreeMap<UiBindingPath, UiBindingDeclaration> {
    specs
        .into_iter()
        .map(|(path, value_type)| {
            (
                UiBindingPath::from_str(path).expect("claim history binding path is static"),
                UiBindingDeclaration {
                    scope: UiBindingScope::Owner,
                    value_type,
                    default: None,
                    missing: UiBindingMissingBehavior::UseConsumerFallback,
                },
            )
        })
        .collect()
}

fn set_binding(
    contract: &super::host::ActivityScreenHostContract,
    values: &mut UiBindingValues,
    path: &str,
    value: UiBindingValue,
) {
    let path = UiBindingPath::from_str(path).unwrap();
    let declaration = contract
        .claim_history_bindings
        .get(&path)
        .expect("claim history binding schema contains synchronized value");
    values.set_scoped(
        ACTIVITY_CLAIM_HISTORY_DOCUMENT_ID,
        OWNER_MAIN_WORLD_ACTIVITY_CLAIM_HISTORY.as_str(),
        &path,
        declaration,
        value,
    );
}

fn matches_zero_param_action(
    action: &UiActionDispatch,
    expected_action: &str,
    source: &str,
) -> bool {
    action.params.is_empty()
        && business_target_matches_action(action)
        && action.document_id.as_str() == ACTIVITY_CLAIM_HISTORY_DOCUMENT_ID
        && action.owner == OWNER_MAIN_WORLD_ACTIVITY_CLAIM_HISTORY.as_str()
        && action.action.as_str() == expected_action
        && action.source_node.as_str() == source
}

fn business_target_matches_action(action: &UiActionDispatch) -> bool {
    matches!(
        &action.kind,
        UiRegisteredActionKind::BusinessCommand { target } if target == action.action.as_str()
    )
}

fn business_command(target: &str) -> UiRegisteredActionKind {
    UiRegisteredActionKind::BusinessCommand {
        target: target.to_owned(),
    }
}

fn activity_type_label(value: &str) -> &'static str {
    match value {
        "login_reward" => "Login Reward",
        "lottery" => "Lucky Draw",
        _ => "Activity",
    }
}

fn activity_action_label(record: &ActivityClaimHistoryRecord) -> String {
    match &record.stage_id {
        Some(stage_id) => format!("{}: {}", record.action_type, stage_id),
        None => record.action_type.clone(),
    }
}

fn activity_claim_time(record: &ActivityClaimHistoryRecord) -> String {
    match record.completed_at_ms {
        Some(completed_at_ms) => format!("Claimed: {}", display_time(completed_at_ms)),
        None => format!("Requested: {}", display_time(record.created_at_ms)),
    }
}

fn activity_claim_history_status_label(status: ActivityClaimHistoryStatus) -> &'static str {
    match status {
        ActivityClaimHistoryStatus::Granted => "Granted",
        ActivityClaimHistoryStatus::Processing => "Processing",
        ActivityClaimHistoryStatus::RetryableFailure => "Retry available",
        ActivityClaimHistoryStatus::PermanentFailure => "Failed",
        ActivityClaimHistoryStatus::ReconciliationPending => "Reconciling",
        ActivityClaimHistoryStatus::BlockedCapacity => "Capacity unavailable",
        ActivityClaimHistoryStatus::ManualReview => "Under review",
    }
}

fn activity_reward_summary(record: &ActivityClaimHistoryRecord) -> String {
    if record.status != ActivityClaimHistoryStatus::Granted {
        return "Reward summary unavailable".to_owned();
    }
    if record.rewards.is_empty() {
        return "No reward summary".to_owned();
    }
    record
        .rewards
        .iter()
        .map(|reward| {
            format!(
                "{} {} x{}",
                reward.reward_type, reward.asset_id, reward.quantity
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn activity_claim_history_status(
    state: ActivityClaimHistoryLoadState,
    record_count: usize,
    display_limit_reached: bool,
) -> String {
    match state {
        ActivityClaimHistoryLoadState::Idle => "Claim history unavailable".to_owned(),
        ActivityClaimHistoryLoadState::LoadingFirstPage => "Loading claim history".to_owned(),
        ActivityClaimHistoryLoadState::LoadingMore => "Loading more records".to_owned(),
        ActivityClaimHistoryLoadState::Ready if display_limit_reached => {
            format!("Showing latest {MAX_ACTIVITY_HISTORY_CLIENT_RECORDS} claim records")
        }
        ActivityClaimHistoryLoadState::Ready => format!("{} claim records", record_count),
        ActivityClaimHistoryLoadState::Empty => "No claim records".to_owned(),
        ActivityClaimHistoryLoadState::Failed => "Claim history could not be loaded".to_owned(),
    }
}

fn activity_error_message(error: &ActivityError) -> String {
    format!("Claim history request failed ({})", error.error_code)
}

fn display_time(value: i64) -> String {
    if value <= 0 {
        "--".to_owned()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        framework::network::ConnectionId,
        game::{
            myserver::{ActivityRewardSummary, GameConnectionState},
            scenes::main_world_entry::MainWorldEntryPhase,
        },
    };

    fn record(status: ActivityClaimHistoryStatus) -> ActivityClaimHistoryRecord {
        ActivityClaimHistoryRecord {
            activity_id: "login-reward-2026".to_owned(),
            version: 3,
            activity_type: "login_reward".to_owned(),
            action_type: "claim".to_owned(),
            stage_id: Some("day-7".to_owned()),
            created_at_ms: 100,
            completed_at_ms: Some(101),
            status,
            rewards: vec![ActivityRewardSummary {
                reward_type: "currency".to_owned(),
                asset_id: "gold".to_owned(),
                quantity: 50,
            }],
        }
    }

    #[test]
    fn history_item_exposes_only_public_record_summary_fields() {
        let UiBindingValue::Record(item) =
            claim_history_item(&record(ActivityClaimHistoryStatus::Granted), 0)
        else {
            panic!("claim history item must remain a record");
        };
        assert_eq!(item.len(), 7);
        assert_eq!(item["row_key"], UiBindingValue::String("0".to_owned()));
        assert_eq!(
            item["activity"],
            UiBindingValue::String("Login Reward (login-reward-2026)".to_owned())
        );
        assert_eq!(item["version"], UiBindingValue::Number(3.0));
        assert_eq!(
            item["action"],
            UiBindingValue::String("claim: day-7".to_owned())
        );
        assert_eq!(
            item["time"],
            UiBindingValue::String("Claimed: 101".to_owned())
        );
        assert_eq!(item["status"], UiBindingValue::String("Granted".to_owned()));
        assert_eq!(
            item["rewards"],
            UiBindingValue::String("currency gold x50".to_owned())
        );
        assert!(!item.contains_key("cursor"));
        assert!(!item.contains_key("ticket"));
    }

    #[test]
    fn non_granted_history_records_never_display_reward_snapshots() {
        let record = record(ActivityClaimHistoryStatus::ManualReview);
        assert_eq!(
            activity_reward_summary(&record),
            "Reward summary unavailable"
        );
    }

    #[test]
    fn failed_load_keeps_existing_records_renderable() {
        assert_eq!(
            ActivityClaimHistoryLoadState::Failed.collection_binding_value(true),
            "ready"
        );
        assert_eq!(
            ActivityClaimHistoryLoadState::Failed.collection_binding_value(false),
            "error"
        );
    }

    #[test]
    fn claim_history_binding_list_is_capped_even_for_an_oversized_client_cache() {
        let mut activity = ActivityClientState::default();
        activity.claim_history.records = (0..=MAX_ACTIVITY_HISTORY_CLIENT_RECORDS)
            .map(|index| {
                let mut item = record(ActivityClaimHistoryStatus::Granted);
                item.activity_id = format!("activity-{index}");
                item
            })
            .collect();
        activity.claim_history.has_more = true;
        activity.claim_history.next_cursor = Some("opaque-next-page".to_owned());

        let mut screen_state = ActivityScreenState::default();
        screen_state.main_world_generation = Some(4);
        screen_state.list_open = true;
        screen_state.claim_history_open = true;
        screen_state.claim_history_load_state = ActivityClaimHistoryLoadState::Ready;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<UiBindingValues>()
            .init_resource::<super::super::host::ActivityScreenHostContract>()
            .insert_resource(MainWorldEntryState {
                generation: 4,
                phase: MainWorldEntryPhase::Active,
                character_id: Some("character-1".to_owned()),
                ..Default::default()
            })
            .insert_resource(MyServerSession {
                character_id: Some("character-1".to_owned()),
                connected: true,
                authenticated: true,
                connection_id: Some(ConnectionId::from_raw(7)),
                game_connection_state: GameConnectionState::Authenticated,
                ..Default::default()
            })
            .insert_resource(activity)
            .insert_resource(screen_state)
            .add_systems(Update, sync_activity_claim_history_bindings);
        app.update();

        let path = UiBindingPath::from_str("activity_claim_history.items").unwrap();
        let contract = app
            .world()
            .resource::<super::super::host::ActivityScreenHostContract>();
        let declaration = contract.claim_history_bindings.get(&path).unwrap();
        let value = app
            .world()
            .resource::<UiBindingValues>()
            .scoped_value(
                ACTIVITY_CLAIM_HISTORY_DOCUMENT_ID,
                OWNER_MAIN_WORLD_ACTIVITY_CLAIM_HISTORY.as_str(),
                &path,
                declaration,
            )
            .unwrap();
        let UiBindingValue::List(items) = value else {
            panic!("claim history items binding must remain a list");
        };

        assert_eq!(items.len(), MAX_ACTIVITY_HISTORY_CLIENT_RECORDS);
        assert_eq!(
            items.first(),
            Some(&claim_history_item(
                &app.world()
                    .resource::<ActivityClientState>()
                    .claim_history
                    .records[0],
                0,
            ))
        );

        let disabled_path =
            UiBindingPath::from_str("activity_claim_history.load_more_disabled").unwrap();
        let disabled_declaration = contract.claim_history_bindings.get(&disabled_path).unwrap();
        assert_eq!(
            app.world().resource::<UiBindingValues>().scoped_value(
                ACTIVITY_CLAIM_HISTORY_DOCUMENT_ID,
                OWNER_MAIN_WORLD_ACTIVITY_CLAIM_HISTORY.as_str(),
                &disabled_path,
                disabled_declaration,
            ),
            Some(UiBindingValue::Bool(true))
        );
    }
}
