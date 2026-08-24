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
    myserver::{ActivityClientState, ActivityError, MyServerSession},
    scenes::main_world_entry::MainWorldEntryState,
    ui_ids::OWNER_MAIN_WORLD_ACTIVITY_DETAIL,
};

use super::host::{ACTIVITY_DETAIL_DOCUMENT_ID, ACTIVITY_DETAIL_ROUTE};
use super::{ActivityScreenCommand, ActivityScreenState};

pub(super) const ACTION_ACTIVITY_DETAIL_BACK: &str = "main_world_activity_detail.back";
pub(super) const ACTION_ACTIVITY_DETAIL_CLOSE: &str = "main_world_activity_detail.close";

const BACK_NODE: &str = "main_world_activity_detail.back";
const CLOSE_NODE: &str = "main_world_activity_detail.close";

pub(super) fn activity_detail_binding_schema() -> BTreeMap<UiBindingPath, UiBindingDeclaration> {
    binding_schema([
        (
            "activity_detail.collection_state",
            UiBindingType::Enum {
                values: ["loading", "ready", "error"].map(str::to_owned).to_vec(),
            },
        ),
        ("activity_detail.title", UiBindingType::String),
        ("activity_detail.status", UiBindingType::String),
        ("activity_detail.version", UiBindingType::String),
        ("activity_detail.time", UiBindingType::String),
        ("activity_detail.progress", UiBindingType::String),
        ("activity_detail.error", UiBindingType::String),
        (
            "activity_detail.error_visibility",
            UiBindingType::Visibility,
        ),
    ])
}

pub(super) fn activity_detail_action_descriptors() -> Vec<UiActionDescriptor> {
    let document_id = || UiDocumentId::from_str(ACTIVITY_DETAIL_DOCUMENT_ID).unwrap();
    vec![
        UiActionDescriptor::new(
            UiActionId::from_str(ACTION_ACTIVITY_DETAIL_BACK).unwrap(),
            document_id(),
            OWNER_MAIN_WORLD_ACTIVITY_DETAIL.as_str(),
            business_command(ACTION_ACTIVITY_DETAIL_BACK),
        )
        .with_source(UiNodeId::from_str(BACK_NODE).unwrap()),
        UiActionDescriptor::new(
            UiActionId::from_str(ACTION_ACTIVITY_DETAIL_CLOSE).unwrap(),
            document_id(),
            OWNER_MAIN_WORLD_ACTIVITY_DETAIL.as_str(),
            business_command(ACTION_ACTIVITY_DETAIL_CLOSE),
        )
        .with_source(UiNodeId::from_str(CLOSE_NODE).unwrap()),
    ]
}

pub(super) fn handle_activity_detail_document_actions(
    mut actions: MessageReader<UiActionDispatch>,
    entry: Option<Res<MainWorldEntryState>>,
    session: Option<Res<MyServerSession>>,
    state: Res<ActivityScreenState>,
    mut commands: MessageWriter<ActivityScreenCommand>,
) {
    if !state.detail_open
        || state.detail_route != Some(ACTIVITY_DETAIL_ROUTE)
        || !state.accepts_active_main_world(entry.as_deref(), session.as_deref())
    {
        return;
    }
    for action in actions.read() {
        if matches_zero_param_action(action, ACTION_ACTIVITY_DETAIL_BACK, BACK_NODE)
            || matches_zero_param_action(action, ACTION_ACTIVITY_DETAIL_CLOSE, CLOSE_NODE)
        {
            commands.write(ActivityScreenCommand::CloseDetail);
        }
    }
}

pub(super) fn sync_activity_detail_bindings(
    entry: Option<Res<MainWorldEntryState>>,
    session: Option<Res<MyServerSession>>,
    activity: Res<ActivityClientState>,
    state: Res<ActivityScreenState>,
    contract: Res<super::host::ActivityScreenHostContract>,
    mut values: ResMut<UiBindingValues>,
) {
    if !state.detail_open
        || state.detail_route != Some(ACTIVITY_DETAIL_ROUTE)
        || !state.accepts_active_main_world(entry.as_deref(), session.as_deref())
    {
        return;
    }
    let detail = state
        .selected
        .as_ref()
        .and_then(|selected| activity.details.get(selected));
    let error = state
        .detail_error
        .as_ref()
        .map(activity_error_message)
        .unwrap_or_default();
    for (path, value) in [
        (
            "activity_detail.collection_state",
            UiBindingValue::Enum(state.detail_load_state.binding_value().to_owned()),
        ),
        (
            "activity_detail.title",
            UiBindingValue::String(detail.map_or_else(
                || "Activity".to_owned(),
                |detail| activity_type_label(&detail.summary.activity_type).to_owned(),
            )),
        ),
        (
            "activity_detail.status",
            UiBindingValue::String(
                detail.map_or_else(String::new, |detail| detail.summary.status.clone()),
            ),
        ),
        (
            "activity_detail.version",
            UiBindingValue::String(detail.map_or_else(String::new, |detail| {
                format!("Version {}", detail.summary.version)
            })),
        ),
        (
            "activity_detail.time",
            UiBindingValue::String(detail.map_or_else(String::new, |detail| {
                format!(
                    "{} - {} ({})",
                    display_time(detail.summary.start_at_ms),
                    display_time(detail.summary.end_at_ms),
                    detail.summary.timezone
                )
            })),
        ),
        (
            "activity_detail.progress",
            UiBindingValue::String(detail.map_or_else(
                || "Waiting for server detail".to_owned(),
                |detail| match detail.progress.as_ref() {
                    Some(progress) => {
                        format!("{} schema {}", progress.type_name, progress.schema_version)
                    }
                    None => "Server progress is not available yet".to_owned(),
                },
            )),
        ),
        ("activity_detail.error", UiBindingValue::String(error)),
        (
            "activity_detail.error_visibility",
            UiBindingValue::Visibility(if state.detail_error.is_some() {
                UiBindingVisibility::Visible
            } else {
                UiBindingVisibility::Hidden
            }),
        ),
    ] {
        set_binding(&contract, &mut values, path, value);
    }
}

fn binding_schema<const N: usize>(
    specs: [(&str, UiBindingType); N],
) -> BTreeMap<UiBindingPath, UiBindingDeclaration> {
    specs
        .into_iter()
        .map(|(path, value_type)| {
            (
                UiBindingPath::from_str(path).expect("activity detail binding path is static"),
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
        .detail_bindings
        .get(&path)
        .expect("activity detail binding schema contains synchronized value");
    values.set_scoped(
        ACTIVITY_DETAIL_DOCUMENT_ID,
        OWNER_MAIN_WORLD_ACTIVITY_DETAIL.as_str(),
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
        && action.document_id.as_str() == ACTIVITY_DETAIL_DOCUMENT_ID
        && action.owner == OWNER_MAIN_WORLD_ACTIVITY_DETAIL.as_str()
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
