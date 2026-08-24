use std::{collections::BTreeMap, str::FromStr};

use bevy::prelude::*;

use crate::framework::ui::{
    core::binding::UiBindingValues,
    document::{
        UiActionDescriptor, UiActionDispatch, UiActionId, UiActionParamSchema, UiActionParamType,
        UiBindingDeclaration, UiBindingMissingBehavior, UiBindingPath, UiBindingScope,
        UiBindingType, UiBindingValue, UiBindingVisibility, UiDocumentId, UiNodeId,
        UiRegisteredActionKind,
    },
};
use crate::game::{
    myserver::{ActivityClientState, ActivityError, ActivitySummary, MyServerSession},
    scenes::main_world_entry::MainWorldEntryState,
    ui_ids::OWNER_MAIN_WORLD_ACTIVITY_LIST,
};

use super::host::{ACTIVITY_LIST_DOCUMENT_ID, ActivityListLoadState, visible_activity};
use super::{ActivityScreenCommand, ActivityScreenState};

pub(super) const ACTION_ACTIVITY_LIST_RETRY: &str = "main_world_activity_list.retry";
pub(super) const ACTION_ACTIVITY_LIST_SELECT: &str = "main_world_activity_list.select";
pub(super) const ACTION_ACTIVITY_LIST_CLOSE: &str = "main_world_activity_list.close";

const RETRY_NODE: &str = "main_world_activity_list.retry";
const SELECT_NODE: &str = "main_world_activity_list.item.open";
const CLOSE_NODE: &str = "main_world_activity_list.close";

pub(super) fn activity_list_binding_schema() -> BTreeMap<UiBindingPath, UiBindingDeclaration> {
    let item = UiBindingType::Record {
        fields: BTreeMap::from([
            ("activity_id".to_owned(), UiBindingType::String),
            ("version".to_owned(), UiBindingType::Number),
            ("title".to_owned(), UiBindingType::String),
            ("status".to_owned(), UiBindingType::String),
            ("time".to_owned(), UiBindingType::String),
            ("claim_deadline".to_owned(), UiBindingType::String),
            ("disabled".to_owned(), UiBindingType::Bool),
        ]),
    };
    let mut bindings = binding_schema(vec![
        (
            "activity_list.collection_state",
            UiBindingType::Enum {
                values: ["loading", "ready", "error"].map(str::to_owned).to_vec(),
            },
        ),
        (
            "activity_list.items",
            UiBindingType::List {
                item: Box::new(item),
                max_items: 64,
            },
        ),
        ("activity_list.status", UiBindingType::String),
        ("activity_list.retry_disabled", UiBindingType::Bool),
        ("activity_list.retry_loading", UiBindingType::Bool),
        ("activity_list.error", UiBindingType::String),
        ("activity_list.error_visibility", UiBindingType::Visibility),
    ]);
    for (path, value_type) in [
        ("activity_list.item.activity_id", UiBindingType::String),
        ("activity_list.item.version", UiBindingType::Number),
        ("activity_list.item.title", UiBindingType::String),
        ("activity_list.item.status", UiBindingType::String),
        ("activity_list.item.time", UiBindingType::String),
        ("activity_list.item.claim_deadline", UiBindingType::String),
        ("activity_list.item.disabled", UiBindingType::Bool),
    ] {
        bindings.insert(
            UiBindingPath::from_str(path).expect("activity list item binding path is static"),
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

pub(super) fn activity_list_action_descriptors() -> Vec<UiActionDescriptor> {
    let document_id = || UiDocumentId::from_str(ACTIVITY_LIST_DOCUMENT_ID).unwrap();
    vec![
        UiActionDescriptor::new(
            UiActionId::from_str(ACTION_ACTIVITY_LIST_RETRY).unwrap(),
            document_id(),
            OWNER_MAIN_WORLD_ACTIVITY_LIST.as_str(),
            business_command(ACTION_ACTIVITY_LIST_RETRY),
        )
        .with_source(UiNodeId::from_str(RETRY_NODE).unwrap()),
        UiActionDescriptor::new(
            UiActionId::from_str(ACTION_ACTIVITY_LIST_SELECT).unwrap(),
            document_id(),
            OWNER_MAIN_WORLD_ACTIVITY_LIST.as_str(),
            business_command(ACTION_ACTIVITY_LIST_SELECT),
        )
        .with_source(UiNodeId::from_str(SELECT_NODE).unwrap())
        .with_param(
            "activity_id",
            UiActionParamSchema::required(UiActionParamType::String { max_bytes: 128 }),
        )
        .with_param(
            "version",
            UiActionParamSchema::required(UiActionParamType::Number {
                min: Some(1.0),
                max: Some(u32::MAX as f64),
            }),
        ),
        UiActionDescriptor::new(
            UiActionId::from_str(ACTION_ACTIVITY_LIST_CLOSE).unwrap(),
            document_id(),
            OWNER_MAIN_WORLD_ACTIVITY_LIST.as_str(),
            business_command(ACTION_ACTIVITY_LIST_CLOSE),
        )
        .with_source(UiNodeId::from_str(CLOSE_NODE).unwrap()),
    ]
}

pub(super) fn handle_activity_list_document_actions(
    mut actions: MessageReader<UiActionDispatch>,
    entry: Option<Res<MainWorldEntryState>>,
    session: Option<Res<MyServerSession>>,
    activity: Res<ActivityClientState>,
    state: Res<ActivityScreenState>,
    mut commands: MessageWriter<ActivityScreenCommand>,
) {
    if !state.list_open || !state.accepts_active_main_world(entry.as_deref(), session.as_deref()) {
        return;
    }
    for action in actions.read() {
        match action.action.as_str() {
            ACTION_ACTIVITY_LIST_RETRY
                if matches_zero_param_action(action, ACTION_ACTIVITY_LIST_RETRY, RETRY_NODE)
                    && state.list_load_state != ActivityListLoadState::Loading =>
            {
                commands.write(ActivityScreenCommand::RetryList);
            }
            ACTION_ACTIVITY_LIST_SELECT => {
                let Some((activity_id, version)) = activity_selection(action) else {
                    continue;
                };
                if state.list_load_state == ActivityListLoadState::Loading
                    || state.detail_open
                    || !activity.activities.iter().any(|summary| {
                        visible_activity(summary)
                            && summary.activity_id == activity_id
                            && summary.version == version
                    })
                {
                    continue;
                }
                commands.write(ActivityScreenCommand::OpenDetail {
                    activity_id,
                    version,
                });
            }
            ACTION_ACTIVITY_LIST_CLOSE
                if matches_zero_param_action(action, ACTION_ACTIVITY_LIST_CLOSE, CLOSE_NODE) =>
            {
                commands.write(ActivityScreenCommand::CloseList);
            }
            _ => {}
        }
    }
}

pub(super) fn sync_activity_list_bindings(
    entry: Option<Res<MainWorldEntryState>>,
    session: Option<Res<MyServerSession>>,
    activity: Res<ActivityClientState>,
    state: Res<ActivityScreenState>,
    contract: Res<super::host::ActivityScreenHostContract>,
    mut values: ResMut<UiBindingValues>,
) {
    if !state.list_open || !state.accepts_active_main_world(entry.as_deref(), session.as_deref()) {
        return;
    }
    let list_is_loading = state.list_load_state == ActivityListLoadState::Loading;
    let error = state
        .list_error
        .as_ref()
        .map(activity_error_message)
        .unwrap_or_default();
    let items = activity
        .activities
        .iter()
        .filter(|summary| visible_activity(summary))
        .map(|summary| activity_list_item(summary, list_is_loading))
        .collect();
    for (path, value) in [
        (
            "activity_list.collection_state",
            UiBindingValue::Enum(state.list_load_state.binding_value().to_owned()),
        ),
        ("activity_list.items", UiBindingValue::List(items)),
        (
            "activity_list.status",
            UiBindingValue::String(activity_list_status(state.list_load_state).to_owned()),
        ),
        (
            "activity_list.retry_disabled",
            UiBindingValue::Bool(list_is_loading),
        ),
        (
            "activity_list.retry_loading",
            UiBindingValue::Bool(list_is_loading),
        ),
        ("activity_list.error", UiBindingValue::String(error)),
        (
            "activity_list.error_visibility",
            UiBindingValue::Visibility(if state.list_error.is_some() {
                UiBindingVisibility::Visible
            } else {
                UiBindingVisibility::Hidden
            }),
        ),
    ] {
        set_binding(&contract, &mut values, path, value);
    }
}

fn activity_list_item(summary: &ActivitySummary, disabled: bool) -> UiBindingValue {
    UiBindingValue::Record(BTreeMap::from([
        (
            "activity_id".to_owned(),
            UiBindingValue::String(summary.activity_id.clone()),
        ),
        (
            "version".to_owned(),
            UiBindingValue::Number(summary.version as f64),
        ),
        (
            "title".to_owned(),
            UiBindingValue::String(activity_type_label(&summary.activity_type).to_owned()),
        ),
        (
            "status".to_owned(),
            UiBindingValue::String(activity_status_label(&summary.status).to_owned()),
        ),
        (
            "time".to_owned(),
            UiBindingValue::String(format!(
                "{} - {} ({})",
                display_time(summary.start_at_ms),
                display_time(summary.end_at_ms),
                summary.timezone
            )),
        ),
        (
            "claim_deadline".to_owned(),
            UiBindingValue::String(if summary.claim_deadline_ms > 0 {
                format!(
                    "Claim deadline: {}",
                    display_time(summary.claim_deadline_ms)
                )
            } else {
                String::new()
            }),
        ),
        ("disabled".to_owned(), UiBindingValue::Bool(disabled)),
    ]))
}

fn binding_schema(
    specs: Vec<(&str, UiBindingType)>,
) -> BTreeMap<UiBindingPath, UiBindingDeclaration> {
    specs
        .into_iter()
        .map(|(path, value_type)| {
            (
                UiBindingPath::from_str(path).expect("activity list binding path is static"),
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
        .list_bindings
        .get(&path)
        .expect("activity list binding schema contains synchronized value");
    values.set_scoped(
        ACTIVITY_LIST_DOCUMENT_ID,
        OWNER_MAIN_WORLD_ACTIVITY_LIST.as_str(),
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
        && action.document_id.as_str() == ACTIVITY_LIST_DOCUMENT_ID
        && action.owner == OWNER_MAIN_WORLD_ACTIVITY_LIST.as_str()
        && action.action.as_str() == expected_action
        && action.source_node.as_str() == source
}

fn activity_selection(action: &UiActionDispatch) -> Option<(String, u32)> {
    if !business_target_matches_action(action)
        || action.document_id.as_str() != ACTIVITY_LIST_DOCUMENT_ID
        || action.owner != OWNER_MAIN_WORLD_ACTIVITY_LIST.as_str()
        || action.action.as_str() != ACTION_ACTIVITY_LIST_SELECT
        || action.source_node.as_str() != SELECT_NODE
        || action.params.len() != 2
    {
        return None;
    }
    let crate::framework::ui::document::UiActionValue::String(activity_id) =
        action.params.get("activity_id")?
    else {
        return None;
    };
    let crate::framework::ui::document::UiActionValue::Number(version) =
        action.params.get("version")?
    else {
        return None;
    };
    let version = *version;
    (activity_id.trim() == activity_id
        && !activity_id.is_empty()
        && version.is_finite()
        && version.fract() == 0.0
        && (1.0..=u32::MAX as f64).contains(&version))
    .then(|| (activity_id.clone(), version as u32))
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

fn activity_status_label(value: &str) -> &'static str {
    match value {
        "published" => "Upcoming",
        "running" => "Running",
        "ended" => "Ended",
        _ => "Unavailable",
    }
}

fn activity_list_status(state: ActivityListLoadState) -> &'static str {
    match state {
        ActivityListLoadState::Idle => "Activities unavailable",
        ActivityListLoadState::Loading => "Loading activities",
        ActivityListLoadState::Ready => "Activities ready",
        ActivityListLoadState::Failed => "Activities could not be loaded",
    }
}

fn activity_error_message(error: &ActivityError) -> String {
    format!("Activity request failed ({})", error.error_code)
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

    fn summary(activity_type: &str, status: &str) -> ActivitySummary {
        ActivitySummary {
            activity_id: "activity-1".to_owned(),
            version: 1,
            activity_type: activity_type.to_owned(),
            status: status.to_owned(),
            start_at_ms: 1,
            end_at_ms: 2,
            claim_deadline_ms: 3,
            timezone: "UTC".to_owned(),
        }
    }

    #[test]
    fn visible_activity_filters_unreleased_offline_and_unknown_activity_types() {
        for (activity_type, status, expected) in [
            ("login_reward", "running", true),
            ("lottery", "published", true),
            ("lottery", "ended", true),
            ("lottery", "draft", false),
            ("lottery", "archived", false),
            ("lottery", "offline", false),
            ("future_type", "running", false),
        ] {
            assert_eq!(visible_activity(&summary(activity_type, status)), expected);
        }
    }

    #[test]
    fn activity_list_item_keeps_server_time_display_only() {
        let row = activity_list_item(&summary("lottery", "ended"), false);
        let UiBindingValue::Record(row) = row else {
            panic!("activity item must remain a record");
        };
        assert_eq!(row["status"], UiBindingValue::String("Ended".to_owned()));
        assert_eq!(row["disabled"], UiBindingValue::Bool(false));
        assert!(matches!(row["time"], UiBindingValue::String(_)));
    }
}
