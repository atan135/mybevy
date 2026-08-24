use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

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
        ActivityClientState, ActivityError, ActivityErrorKind, ActivityEvent, ActivityKey,
        ActivityRequestKind, LotteryPlayerView, MyServerCommand, MyServerSession,
    },
    scenes::main_world_entry::MainWorldEntryState,
    ui_ids::OWNER_MAIN_WORLD_LOTTERY_DETAIL,
};

use super::{
    ActivityScreenCommand, ActivityScreenState,
    host::{LOTTERY_DETAIL_DOCUMENT_ID, LOTTERY_DETAIL_ROUTE},
};

pub(super) const ACTION_LOTTERY_DRAW: &str = "main_world_lottery_detail.draw";
pub(super) const ACTION_LOTTERY_BACK: &str = "main_world_lottery_detail.back";
pub(super) const ACTION_LOTTERY_CLOSE: &str = "main_world_lottery_detail.close";

const DRAW_NODE: &str = "main_world_lottery_detail.draw";
const BACK_NODE: &str = "main_world_lottery_detail.back";
const CLOSE_NODE: &str = "main_world_lottery_detail.close";
const LOTTERY_REQUEST_ID_PREFIX: &str = "lottery-draw-";
const LOTTERY_REQUEST_NONCE_BYTES: usize = 16;

static LOTTERY_FALLBACK_NONCE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Eq, PartialEq)]
enum LotteryDrawPhase {
    Pending,
    Retryable,
    Refreshing,
    Reconciling,
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct LotteryDrawKey {
    character_id: String,
    activity: ActivityKey,
}

struct LotteryDrawEntry {
    client_request_id: String,
    phase: LotteryDrawPhase,
}

/// Plaintext request IDs remain only in this page-local retry tracker.
/// The MyServer activity tracker stores an opaque fingerprint instead.
#[derive(Resource)]
pub(super) struct LotteryDrawTracker {
    character_id: Option<String>,
    request_nonce: [u8; LOTTERY_REQUEST_NONCE_BYTES],
    next_request_sequence: u64,
    entries: HashMap<LotteryDrawKey, LotteryDrawEntry>,
    last_error: Option<ActivityError>,
}

impl Default for LotteryDrawTracker {
    fn default() -> Self {
        Self {
            character_id: None,
            request_nonce: lottery_request_nonce(),
            next_request_sequence: 0,
            entries: HashMap::new(),
            last_error: None,
        }
    }
}

impl LotteryDrawTracker {
    fn sync_character(&mut self, character_id: &str) {
        if self.character_id.as_deref() != Some(character_id) {
            self.character_id = Some(character_id.to_owned());
            self.entries.clear();
            self.last_error = None;
        }
    }

    fn begin_or_retry(&mut self, character_id: &str, activity: &ActivityKey) -> Option<String> {
        self.sync_character(character_id);
        let key = LotteryDrawKey {
            character_id: character_id.to_owned(),
            activity: activity.clone(),
        };
        if let Some(entry) = self.entries.get_mut(&key) {
            if entry.phase != LotteryDrawPhase::Retryable {
                return None;
            }
            entry.phase = LotteryDrawPhase::Pending;
            self.last_error = None;
            return Some(entry.client_request_id.clone());
        }
        self.next_request_sequence = self.next_request_sequence.wrapping_add(1).max(1);
        let client_request_id = lottery_request_id(&self.request_nonce, self.next_request_sequence);
        self.entries.insert(
            key,
            LotteryDrawEntry {
                client_request_id: client_request_id.clone(),
                phase: LotteryDrawPhase::Pending,
            },
        );
        self.last_error = None;
        Some(client_request_id)
    }

    fn phase(
        &self,
        character_id: Option<&str>,
        activity: &ActivityKey,
    ) -> Option<LotteryDrawPhase> {
        let character_id = character_id?;
        self.entries
            .get(&LotteryDrawKey {
                character_id: character_id.to_owned(),
                activity: activity.clone(),
            })
            .map(|entry| entry.phase)
    }

    fn mark_resolved(&mut self, activity: &ActivityKey, processing: bool) {
        for (key, entry) in &mut self.entries {
            if &key.activity == activity {
                entry.phase = if processing {
                    LotteryDrawPhase::Retryable
                } else {
                    LotteryDrawPhase::Refreshing
                };
            }
        }
        self.last_error = None;
    }

    fn mark_activity_retryable(&mut self, activity: &ActivityKey, error: &ActivityError) {
        for (key, entry) in &mut self.entries {
            if &key.activity == activity {
                entry.phase = LotteryDrawPhase::Retryable;
            }
        }
        self.last_error = Some(error.clone());
    }

    fn mark_activity_refreshing(&mut self, activity: &ActivityKey, error: &ActivityError) {
        for (key, entry) in &mut self.entries {
            if &key.activity == activity {
                entry.phase = LotteryDrawPhase::Refreshing;
            }
        }
        self.last_error = Some(error.clone());
    }

    fn mark_activity_reconciling(&mut self, activity: &ActivityKey, error: &ActivityError) {
        for (key, entry) in &mut self.entries {
            if &key.activity == activity {
                entry.phase = LotteryDrawPhase::Reconciling;
            }
        }
        self.last_error = Some(error.clone());
    }

    fn mark_all_retryable(&mut self) {
        for entry in self.entries.values_mut() {
            if entry.phase != LotteryDrawPhase::Reconciling {
                entry.phase = LotteryDrawPhase::Retryable;
            }
        }
    }

    fn reconcile_detail(
        &mut self,
        character_id: Option<&str>,
        activity: &ActivityKey,
        activity_status: &str,
        view: &LotteryPlayerView,
    ) {
        let Some(character_id) = character_id else {
            return;
        };
        self.sync_character(character_id);
        let key = LotteryDrawKey {
            character_id: character_id.to_owned(),
            activity: activity.clone(),
        };
        let Some(entry) = self.entries.get_mut(&key) else {
            return;
        };
        if lottery_result_requires_reconciliation(view) {
            entry.phase = LotteryDrawPhase::Reconciling;
            return;
        }
        if matches!(
            entry.phase,
            LotteryDrawPhase::Refreshing | LotteryDrawPhase::Reconciling
        ) || !lottery_draw_is_available(activity_status, view)
        {
            self.entries.remove(&key);
        }
    }
}

pub(super) fn lottery_binding_schema() -> BTreeMap<UiBindingPath, UiBindingDeclaration> {
    binding_schema([
        ("lottery.status", UiBindingType::String),
        ("lottery.availability", UiBindingType::String),
        ("lottery.counts", UiBindingType::String),
        ("lottery.request", UiBindingType::String),
        ("lottery.result", UiBindingType::String),
        ("lottery.draw_state", UiBindingType::String),
        ("lottery.error", UiBindingType::String),
        ("lottery.error_visibility", UiBindingType::Visibility),
        ("lottery.draw_disabled", UiBindingType::Bool),
        ("lottery.draw_loading", UiBindingType::Bool),
    ])
}

pub(super) fn lottery_action_descriptors() -> Vec<UiActionDescriptor> {
    let document_id = || UiDocumentId::from_str(LOTTERY_DETAIL_DOCUMENT_ID).unwrap();
    vec![
        UiActionDescriptor::new(
            UiActionId::from_str(ACTION_LOTTERY_DRAW).unwrap(),
            document_id(),
            OWNER_MAIN_WORLD_LOTTERY_DETAIL.as_str(),
            business_command(ACTION_LOTTERY_DRAW),
        )
        .with_source(UiNodeId::from_str(DRAW_NODE).unwrap()),
        UiActionDescriptor::new(
            UiActionId::from_str(ACTION_LOTTERY_BACK).unwrap(),
            document_id(),
            OWNER_MAIN_WORLD_LOTTERY_DETAIL.as_str(),
            business_command(ACTION_LOTTERY_BACK),
        )
        .with_source(UiNodeId::from_str(BACK_NODE).unwrap()),
        UiActionDescriptor::new(
            UiActionId::from_str(ACTION_LOTTERY_CLOSE).unwrap(),
            document_id(),
            OWNER_MAIN_WORLD_LOTTERY_DETAIL.as_str(),
            business_command(ACTION_LOTTERY_CLOSE),
        )
        .with_source(UiNodeId::from_str(CLOSE_NODE).unwrap()),
    ]
}

pub(super) fn handle_lottery_document_actions(
    mut actions: MessageReader<UiActionDispatch>,
    entry: Option<Res<MainWorldEntryState>>,
    session: Option<Res<MyServerSession>>,
    activity: Res<ActivityClientState>,
    state: Res<ActivityScreenState>,
    mut draws: ResMut<LotteryDrawTracker>,
    mut screen_commands: MessageWriter<ActivityScreenCommand>,
    mut myserver_commands: MessageWriter<MyServerCommand>,
) {
    if !state.detail_open
        || state.detail_route != Some(LOTTERY_DETAIL_ROUTE)
        || !state.accepts_active_main_world(entry.as_deref(), session.as_deref())
    {
        return;
    }
    for action in actions.read() {
        if matches_zero_param_action(action, ACTION_LOTTERY_BACK, BACK_NODE)
            || matches_zero_param_action(action, ACTION_LOTTERY_CLOSE, CLOSE_NODE)
        {
            screen_commands.write(ActivityScreenCommand::CloseDetail);
            continue;
        }
        if !matches_zero_param_action(action, ACTION_LOTTERY_DRAW, DRAW_NODE) {
            continue;
        }
        let Some(character_id) = session
            .as_deref()
            .and_then(|session| session.character_id.as_deref())
        else {
            continue;
        };
        let Some(key) = state.selected.as_ref() else {
            continue;
        };
        let Some(detail) = activity.details.get(key) else {
            continue;
        };
        let Some(view) = detail
            .progress
            .as_ref()
            .and_then(|progress| progress.player_view.lottery())
        else {
            continue;
        };
        if !lottery_draw_is_available(detail.summary.status.as_str(), view) {
            continue;
        }
        let Some(client_request_id) = draws.begin_or_retry(character_id, key) else {
            continue;
        };
        myserver_commands.write(MyServerCommand::RunActivityAction {
            activity_id: key.activity_id.clone(),
            version: key.version,
            stage_id: String::new(),
            action_type: "draw".to_owned(),
            client_request_id,
        });
    }
}

pub(super) fn apply_lottery_activity_events(
    mut events: MessageReader<ActivityEvent>,
    session: Option<Res<MyServerSession>>,
    state: Res<ActivityScreenState>,
    activity: Res<ActivityClientState>,
    mut draws: ResMut<LotteryDrawTracker>,
    mut myserver_commands: MessageWriter<MyServerCommand>,
) {
    let character_id = session
        .as_deref()
        .and_then(|session| session.character_id.as_deref());
    if let Some(character_id) = character_id {
        draws.sync_character(character_id);
    }
    for event in events.read() {
        match event {
            ActivityEvent::DetailUpdated {
                activity_id,
                version,
            } => {
                let key = ActivityKey {
                    activity_id: activity_id.clone(),
                    version: *version,
                };
                if let Some(detail) = activity.details.get(&key)
                    && let Some(view) = detail
                        .progress
                        .as_ref()
                        .and_then(|progress| progress.player_view.lottery())
                {
                    draws.reconcile_detail(
                        character_id,
                        &key,
                        detail.summary.status.as_str(),
                        view,
                    );
                }
            }
            ActivityEvent::ActionResolved {
                activity_id,
                version,
                action_type,
                processing,
                ..
            } if action_type == "draw" => {
                let key = ActivityKey {
                    activity_id: activity_id.clone(),
                    version: *version,
                };
                if state.detail_route != Some(LOTTERY_DETAIL_ROUTE)
                    || state.selected.as_ref() != Some(&key)
                {
                    continue;
                }
                draws.mark_resolved(&key, *processing);
                refresh_lottery(&key, &mut myserver_commands);
            }
            ActivityEvent::RequestFailed {
                kind: ActivityRequestKind::Action,
                error,
            } => {
                let Some(key) = state
                    .detail_route
                    .filter(|route| *route == LOTTERY_DETAIL_ROUTE)
                    .and(state.selected.as_ref())
                else {
                    continue;
                };
                match error.kind {
                    ActivityErrorKind::QualificationNotMet | ActivityErrorKind::AlreadyClaimed => {
                        draws.mark_activity_refreshing(key, error);
                        refresh_lottery(key, &mut myserver_commands);
                    }
                    ActivityErrorKind::VersionConflict => {
                        draws.mark_activity_refreshing(key, error);
                        myserver_commands.write(MyServerCommand::LoadActivityList);
                        refresh_lottery(key, &mut myserver_commands);
                    }
                    ActivityErrorKind::Processing
                    | ActivityErrorKind::EngineUnavailable
                    | ActivityErrorKind::RetryableFailure
                    | ActivityErrorKind::StorageUnavailable
                    | ActivityErrorKind::Transport => {
                        draws.mark_activity_retryable(key, error);
                        refresh_lottery(key, &mut myserver_commands);
                    }
                    ActivityErrorKind::ReconciliationPending
                    | ActivityErrorKind::ManualReview
                    | ActivityErrorKind::Unknown => {
                        draws.mark_activity_reconciling(key, error);
                        refresh_lottery(key, &mut myserver_commands);
                    }
                    _ => draws.mark_activity_reconciling(key, error),
                }
            }
            ActivityEvent::ScopeChanged => draws.mark_all_retryable(),
            _ => {}
        }
    }
}

pub(super) fn sync_lottery_bindings(
    entry: Option<Res<MainWorldEntryState>>,
    session: Option<Res<MyServerSession>>,
    activity: Res<ActivityClientState>,
    state: Res<ActivityScreenState>,
    draws: Res<LotteryDrawTracker>,
    contract: Res<super::host::ActivityScreenHostContract>,
    mut values: ResMut<UiBindingValues>,
) {
    if !state.detail_open
        || state.detail_route != Some(LOTTERY_DETAIL_ROUTE)
        || !state.accepts_active_main_world(entry.as_deref(), session.as_deref())
    {
        return;
    }
    let detail = state
        .selected
        .as_ref()
        .and_then(|selected| activity.details.get(selected));
    let view = detail
        .and_then(|detail| detail.progress.as_ref())
        .and_then(|progress| progress.player_view.lottery());
    let error = state
        .detail_error
        .as_ref()
        .or(draws.last_error.as_ref())
        .map(lottery_error_message)
        .or_else(|| {
            (state.detail_load_state != super::host::ActivityDetailLoadState::Loading
                && view.is_none())
            .then(|| "Lottery detail is unavailable".to_owned())
        })
        .unwrap_or_default();
    let activity_status = detail
        .map(|detail| detail.summary.status.as_str())
        .unwrap_or_default();
    let phase = state.selected.as_ref().and_then(|key| {
        draws.phase(
            session
                .as_deref()
                .and_then(|session| session.character_id.as_deref()),
            key,
        )
    });
    let draw_available = view.is_some_and(|view| lottery_draw_is_available(activity_status, view));
    let (draw_disabled, draw_loading) = match phase {
        Some(LotteryDrawPhase::Pending | LotteryDrawPhase::Refreshing) => (true, true),
        Some(LotteryDrawPhase::Reconciling) => (true, false),
        Some(LotteryDrawPhase::Retryable) | None => (!draw_available, false),
    };
    for (path, value) in [
        (
            "lottery.status",
            UiBindingValue::String(lottery_status(detail)),
        ),
        (
            "lottery.availability",
            UiBindingValue::String(view.map_or_else(String::new, lottery_availability)),
        ),
        (
            "lottery.counts",
            UiBindingValue::String(view.map_or_else(String::new, lottery_counts)),
        ),
        (
            "lottery.request",
            UiBindingValue::String(view.map_or_else(String::new, lottery_request_reference)),
        ),
        (
            "lottery.result",
            UiBindingValue::String(view.map_or_else(String::new, lottery_result_reference)),
        ),
        (
            "lottery.draw_state",
            UiBindingValue::String(lottery_draw_state(view, phase)),
        ),
        ("lottery.error", UiBindingValue::String(error.clone())),
        (
            "lottery.error_visibility",
            UiBindingValue::Visibility(if error.is_empty() {
                UiBindingVisibility::Hidden
            } else {
                UiBindingVisibility::Visible
            }),
        ),
        ("lottery.draw_disabled", UiBindingValue::Bool(draw_disabled)),
        ("lottery.draw_loading", UiBindingValue::Bool(draw_loading)),
    ] {
        set_binding(&contract, &mut values, path, value);
    }
}

fn refresh_lottery(key: &ActivityKey, commands: &mut MessageWriter<MyServerCommand>) {
    commands.write(MyServerCommand::LoadActivityDetail {
        activity_id: key.activity_id.clone(),
        version: key.version,
    });
    commands.write(MyServerCommand::LoadActivityProgress {
        activity_id: key.activity_id.clone(),
        version: key.version,
    });
}

fn lottery_draw_is_available(activity_status: &str, view: &LotteryPlayerView) -> bool {
    matches!(activity_status, "running")
        && view.draw_source == "player_action"
        && !lottery_result_requires_reconciliation(view)
        && (view.state.free_draws_remaining > 0 || view.state.voucher_count > 0)
        && (view.daily_draw_limit == 0 || view.state.daily_draw_count < view.daily_draw_limit)
        && (view.total_draw_limit == 0 || view.state.total_draw_count < view.total_draw_limit)
}

fn lottery_result_requires_reconciliation(view: &LotteryPlayerView) -> bool {
    match view.state.result_state.as_deref() {
        Some("applied" | "not_applied") => false,
        Some(_) => true,
        None => view.state.draw_request_id.is_some(),
    }
}

fn lottery_status(detail: Option<&crate::game::myserver::ActivityDetail>) -> String {
    match detail.map(|detail| detail.summary.status.as_str()) {
        Some("published") => "Not started".to_owned(),
        Some("running") => "Ready for server-controlled draw".to_owned(),
        Some("ended") => "Ended".to_owned(),
        Some(_) => "Unavailable".to_owned(),
        None => String::new(),
    }
}

fn lottery_availability(view: &LotteryPlayerView) -> String {
    format!(
        "Free draws remaining: {}  Voucher balance: {}",
        view.state.free_draws_remaining, view.state.voucher_count
    )
}

fn lottery_counts(view: &LotteryPlayerView) -> String {
    format!(
        "Daily draws: {}/{}  Total draws: {}/{}",
        view.state.daily_draw_count,
        draw_limit_label(view.daily_draw_limit),
        view.state.total_draw_count,
        draw_limit_label(view.total_draw_limit)
    )
}

fn draw_limit_label(limit: u32) -> String {
    if limit == 0 {
        "Unlimited".to_owned()
    } else {
        limit.to_string()
    }
}

fn lottery_request_reference(view: &LotteryPlayerView) -> String {
    view.state
        .draw_request_id
        .as_ref()
        .map(|request_id| format!("Latest server request: {request_id}"))
        .unwrap_or_default()
}

fn lottery_result_reference(view: &LotteryPlayerView) -> String {
    match (&view.state.result_state, view.state.result_item_id) {
        (Some(state), Some(item_id)) => {
            format!("Server result ({state}): item:{item_id}; inventory verification pending")
        }
        (Some(state), None) => format!("Server result status: {state}"),
        (None, Some(item_id)) => {
            format!("Server item reference: item:{item_id}; inventory verification pending")
        }
        (None, None) => "No server-confirmed result".to_owned(),
    }
}

fn lottery_draw_state(view: Option<&LotteryPlayerView>, phase: Option<LotteryDrawPhase>) -> String {
    if let Some(view) = view
        && lottery_result_requires_reconciliation(view)
    {
        return "Server result requires verification".to_owned();
    }
    match phase {
        Some(LotteryDrawPhase::Pending) => "Submitting draw request".to_owned(),
        Some(LotteryDrawPhase::Refreshing) => "Refreshing server result".to_owned(),
        Some(LotteryDrawPhase::Retryable) => "Retry the same draw request".to_owned(),
        Some(LotteryDrawPhase::Reconciling) => "Server result requires verification".to_owned(),
        None => "Awaiting server-controlled draw".to_owned(),
    }
}

fn lottery_error_message(error: &ActivityError) -> String {
    format!(
        "Lottery request needs server verification ({})",
        error.error_code
    )
}

fn binding_schema<const N: usize>(
    specs: [(&str, UiBindingType); N],
) -> BTreeMap<UiBindingPath, UiBindingDeclaration> {
    specs
        .into_iter()
        .map(|(path, value_type)| {
            (
                UiBindingPath::from_str(path).expect("lottery binding path is static"),
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
        .lottery_bindings
        .get(&path)
        .expect("lottery binding schema contains synchronized value");
    values.set_scoped(
        LOTTERY_DETAIL_DOCUMENT_ID,
        OWNER_MAIN_WORLD_LOTTERY_DETAIL.as_str(),
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
        && action.document_id.as_str() == LOTTERY_DETAIL_DOCUMENT_ID
        && action.owner == OWNER_MAIN_WORLD_LOTTERY_DETAIL.as_str()
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

fn lottery_request_nonce() -> [u8; LOTTERY_REQUEST_NONCE_BYTES] {
    let mut nonce = [0; LOTTERY_REQUEST_NONCE_BYTES];
    if getrandom::fill(&mut nonce).is_ok() {
        return nonce;
    }

    // Only used when OS randomness is unavailable. The mixed values are not
    // exposed verbatim and contain no character or gameplay information.
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let timestamp = (elapsed.as_nanos() as u64) ^ ((elapsed.as_nanos() >> 64) as u64);
    let counter = LOTTERY_FALLBACK_NONCE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let seed =
        timestamp ^ counter.rotate_left(17) ^ (u64::from(std::process::id())).rotate_left(41);
    let first = splitmix64(seed);
    let second = splitmix64(first ^ counter.rotate_left(31));
    nonce[..8].copy_from_slice(&first.to_le_bytes());
    nonce[8..].copy_from_slice(&second.to_le_bytes());
    nonce
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn lottery_request_id(nonce: &[u8; LOTTERY_REQUEST_NONCE_BYTES], sequence: u64) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut request_id = String::with_capacity(LOTTERY_REQUEST_ID_PREFIX.len() + 32 + 1 + 20);
    request_id.push_str(LOTTERY_REQUEST_ID_PREFIX);
    for byte in nonce {
        request_id.push(HEX[(byte >> 4) as usize] as char);
        request_id.push(HEX[(byte & 0x0f) as usize] as char);
    }
    request_id.push('-');
    request_id.push_str(&sequence.to_string());
    request_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        framework::{
            network::ConnectionId,
            ui::document::{UiActionValue, UiDocumentId},
        },
        game::{
            myserver::{
                ActivityDetail, ActivityPlayerView, ActivityProgressView, ActivitySummary,
                GameConnectionState, LotteryStateView,
            },
            scenes::main_world_entry::MainWorldEntryPhase,
        },
    };
    use bevy::ecs::message::MessageCursor;

    #[test]
    fn lottery_availability_and_result_render_without_inventing_reward_content() {
        let mut view = sample_view();
        assert!(lottery_draw_is_available("running", &view));
        assert_eq!(
            lottery_availability(&view),
            "Free draws remaining: 1  Voucher balance: 4"
        );
        assert_eq!(
            lottery_counts(&view),
            "Daily draws: 2/10  Total draws: 7/100"
        );
        assert_eq!(
            lottery_result_reference(&view),
            "Server result (applied): item:1001; inventory verification pending"
        );

        view.state.result_state = Some("unknown".to_owned());
        assert!(!lottery_draw_is_available("running", &view));
        assert_eq!(
            lottery_draw_state(Some(&view), None),
            "Server result requires verification"
        );
    }

    #[test]
    fn processing_and_reconnect_retry_keep_the_same_request_id() {
        let mut tracker = LotteryDrawTracker::default();
        let activity = sample_key();
        let first = tracker
            .begin_or_retry("character-1", &activity)
            .expect("initial draw must allocate an id");
        assert!(tracker.begin_or_retry("character-1", &activity).is_none());
        tracker.mark_resolved(&activity, true);
        assert_eq!(
            tracker
                .begin_or_retry("character-1", &activity)
                .expect("processing draw may retry"),
            first
        );
        tracker.mark_all_retryable();
        assert_eq!(
            tracker
                .begin_or_retry("character-1", &activity)
                .expect("reconnected draw may retry"),
            first
        );
    }

    #[test]
    fn fresh_trackers_use_distinct_bounded_draw_request_ids() {
        let activity = sample_key();
        let mut first_tracker = LotteryDrawTracker::default();
        let mut second_tracker = LotteryDrawTracker::default();
        let first = first_tracker
            .begin_or_retry("character-1", &activity)
            .expect("initial draw must allocate an id");
        let second = second_tracker
            .begin_or_retry("character-1", &activity)
            .expect("initial draw must allocate an id");

        assert_ne!(first, second);
        for request_id in [first, second] {
            assert!(request_id.starts_with(LOTTERY_REQUEST_ID_PREFIX));
            assert!(request_id.len() <= 128);
        }
    }

    #[test]
    fn draw_action_emits_only_the_controlled_wire_fields() {
        let key = sample_key();
        let mut app = lottery_test_app(&key);
        let mut cursor = MessageCursor::default();

        app.world_mut().write_message(draw_dispatch());
        app.update();
        let commands = read_new_commands(&app, &mut cursor);
        assert!(matches!(
            commands.as_slice(),
            [MyServerCommand::RunActivityAction {
                activity_id,
                version: 3,
                stage_id,
                action_type,
                client_request_id,
            }] if activity_id == "lottery"
                && stage_id.is_empty()
                && action_type == "draw"
                && client_request_id.starts_with(LOTTERY_REQUEST_ID_PREFIX)
                && client_request_id.len() <= 128
        ));
    }

    #[test]
    fn mock_draw_duplicate_failure_and_reconnect_converge_to_server_detail() {
        let key = sample_key();
        let mut app = lottery_test_app(&key);
        let mut command_cursor = MessageCursor::default();
        let mut server = InProcessLotteryActivityMock::default();

        app.world_mut().write_message(draw_dispatch());
        app.update();
        let first_commands = read_new_commands(&app, &mut command_cursor);
        let [first_draw] = first_commands.as_slice() else {
            panic!("initial interaction must issue one draw command");
        };
        let first_request_id = draw_request_id(first_draw).to_owned();
        let first_response = server.resolve_draw(first_draw);
        assert!(matches!(
            &first_response,
            ActivityEvent::ActionResolved {
                duplicate: false,
                ..
            }
        ));
        app.world_mut().write_message(first_response);
        app.update();
        assert_refresh_commands(read_new_commands(&app, &mut command_cursor), &key);

        app.world_mut().write_message(ActivityEvent::ScopeChanged);
        app.update();
        assert!(read_new_commands(&app, &mut command_cursor).is_empty());

        app.world_mut().write_message(draw_dispatch());
        app.update();
        let retry_commands = read_new_commands(&app, &mut command_cursor);
        let [retry_draw] = retry_commands.as_slice() else {
            panic!("reconnect must retry one draw request");
        };
        assert_eq!(draw_request_id(retry_draw), first_request_id);
        let duplicate_response = server.resolve_draw(retry_draw);
        assert!(matches!(
            &duplicate_response,
            ActivityEvent::ActionResolved {
                duplicate: true,
                ..
            }
        ));
        app.world_mut().write_message(duplicate_response);
        app.update();
        assert_refresh_commands(read_new_commands(&app, &mut command_cursor), &key);
        assert_eq!(server.first_draws, 1);
        assert_eq!(server.duplicate_draws, 1);

        app.world_mut()
            .resource_mut::<ActivityClientState>()
            .details
            .insert(key.clone(), server.detail(&key));
        app.world_mut().write_message(ActivityEvent::DetailUpdated {
            activity_id: key.activity_id.clone(),
            version: key.version,
        });
        app.update();
        assert!(read_new_commands(&app, &mut command_cursor).is_empty());
        assert!(
            app.world()
                .resource::<LotteryDrawTracker>()
                .phase(Some("character-1"), &key)
                .is_none()
        );

        app.world_mut().write_message(draw_dispatch());
        app.update();
        let failure_commands = read_new_commands(&app, &mut command_cursor);
        let [failed_draw] = failure_commands.as_slice() else {
            panic!("settled server detail may start a new draw");
        };
        let failed_request_id = draw_request_id(failed_draw).to_owned();
        assert_ne!(failed_request_id, first_request_id);
        app.world_mut()
            .write_message(InProcessLotteryActivityMock::engine_unavailable());
        app.update();
        assert_refresh_commands(read_new_commands(&app, &mut command_cursor), &key);

        app.world_mut().write_message(draw_dispatch());
        app.update();
        let recovery_commands = read_new_commands(&app, &mut command_cursor);
        let [recovery_draw] = recovery_commands.as_slice() else {
            panic!("retryable failure must allow a controlled retry");
        };
        assert_eq!(draw_request_id(recovery_draw), failed_request_id);
    }

    #[test]
    fn confirmed_detail_clears_reconciling_draw_and_allows_a_new_draw() {
        let key = sample_key();
        let mut app = lottery_test_app(&key);
        let mut command_cursor = MessageCursor::default();

        app.world_mut().write_message(draw_dispatch());
        app.update();
        let first_commands = read_new_commands(&app, &mut command_cursor);
        let [first_draw] = first_commands.as_slice() else {
            panic!("initial interaction must issue one draw command");
        };
        let first_request_id = draw_request_id(first_draw).to_owned();

        let kind = ActivityErrorKind::ReconciliationPending;
        app.world_mut().write_message(ActivityEvent::RequestFailed {
            kind: ActivityRequestKind::Action,
            error: ActivityError {
                kind,
                error_code: "ACTIVITY_RECONCILIATION_PENDING".to_owned(),
                message_key: kind.message_key(),
                retryable: false,
            },
        });
        app.update();
        assert_refresh_commands(read_new_commands(&app, &mut command_cursor), &key);
        assert!(matches!(
            app.world()
                .resource::<LotteryDrawTracker>()
                .phase(Some("character-1"), &key),
            Some(LotteryDrawPhase::Reconciling)
        ));

        let mut unknown_view = sample_view();
        unknown_view.state.draw_request_id = Some("server-draw-pending".to_owned());
        unknown_view.state.result_state = Some("unknown".to_owned());
        app.world_mut()
            .resource_mut::<ActivityClientState>()
            .details
            .insert(key.clone(), lottery_detail(&key, unknown_view, 3));
        app.world_mut().write_message(ActivityEvent::DetailUpdated {
            activity_id: key.activity_id.clone(),
            version: key.version,
        });
        app.update();
        assert!(read_new_commands(&app, &mut command_cursor).is_empty());
        assert!(matches!(
            app.world()
                .resource::<LotteryDrawTracker>()
                .phase(Some("character-1"), &key),
            Some(LotteryDrawPhase::Reconciling)
        ));

        let mut confirmed_view = sample_view();
        confirmed_view.state.draw_request_id = Some("server-draw-confirmed".to_owned());
        confirmed_view.state.result_state = Some("applied".to_owned());
        confirmed_view.state.free_draws_remaining = 1;
        app.world_mut()
            .resource_mut::<ActivityClientState>()
            .details
            .insert(key.clone(), lottery_detail(&key, confirmed_view, 4));
        app.world_mut().write_message(ActivityEvent::DetailUpdated {
            activity_id: key.activity_id.clone(),
            version: key.version,
        });
        app.update();
        assert!(read_new_commands(&app, &mut command_cursor).is_empty());
        assert!(
            app.world()
                .resource::<LotteryDrawTracker>()
                .phase(Some("character-1"), &key)
                .is_none()
        );

        app.world_mut().write_message(draw_dispatch());
        app.update();
        let next_commands = read_new_commands(&app, &mut command_cursor);
        let [next_draw] = next_commands.as_slice() else {
            panic!("confirmed server detail with remaining draws must allow a new draw");
        };
        assert_ne!(draw_request_id(next_draw), first_request_id);
    }

    #[derive(Default)]
    struct InProcessLotteryActivityMock {
        accepted_request_ids: std::collections::HashSet<String>,
        first_draws: usize,
        duplicate_draws: usize,
        result_applied: bool,
    }

    impl InProcessLotteryActivityMock {
        fn resolve_draw(&mut self, command: &MyServerCommand) -> ActivityEvent {
            let MyServerCommand::RunActivityAction {
                activity_id,
                version,
                stage_id,
                action_type,
                client_request_id,
            } = command
            else {
                panic!("mock only accepts lottery draw commands");
            };
            assert!(stage_id.is_empty());
            assert_eq!(action_type, "draw");
            let duplicate = !self.accepted_request_ids.insert(client_request_id.clone());
            if duplicate {
                self.duplicate_draws += 1;
            } else {
                self.first_draws += 1;
                self.result_applied = true;
            }
            ActivityEvent::ActionResolved {
                activity_id: activity_id.clone(),
                version: *version,
                stage_id: String::new(),
                action_type: "draw".to_owned(),
                ok: true,
                processing: false,
                duplicate,
                state_revision: 3,
            }
        }

        fn engine_unavailable() -> ActivityEvent {
            let kind = ActivityErrorKind::EngineUnavailable;
            ActivityEvent::RequestFailed {
                kind: ActivityRequestKind::Action,
                error: ActivityError {
                    kind,
                    error_code: "ACTIVITY_ENGINE_UNAVAILABLE".to_owned(),
                    message_key: kind.message_key(),
                    retryable: true,
                },
            }
        }

        fn detail(&self, key: &ActivityKey) -> ActivityDetail {
            let mut view = sample_view();
            view.state.free_draws_remaining = 1;
            view.state.draw_request_id = Some("server-draw-1".to_owned());
            view.state.result_item_id = self.result_applied.then_some(1001);
            view.state.result_state = self.result_applied.then_some("applied".to_owned());
            lottery_detail(key, view, 3)
        }
    }

    fn lottery_test_app(key: &ActivityKey) -> App {
        let mut app = App::new();
        app.insert_resource(MainWorldEntryState {
            generation: 4,
            phase: MainWorldEntryPhase::Active,
            character_id: Some("character-1".to_owned()),
            ..Default::default()
        })
        .insert_resource(MyServerSession {
            character_id: Some("character-1".to_owned()),
            connection_id: Some(ConnectionId::from_raw(7)),
            connected: true,
            authenticated: true,
            game_connection_state: GameConnectionState::Authenticated,
            ..Default::default()
        })
        .init_resource::<ActivityClientState>()
        .init_resource::<ActivityScreenState>()
        .init_resource::<LotteryDrawTracker>()
        .add_message::<UiActionDispatch>()
        .add_message::<ActivityEvent>()
        .add_message::<ActivityScreenCommand>()
        .add_message::<MyServerCommand>()
        .add_systems(
            Update,
            (
                handle_lottery_document_actions,
                apply_lottery_activity_events,
            ),
        );
        {
            let mut state = app.world_mut().resource_mut::<ActivityScreenState>();
            state.main_world_generation = Some(4);
            state.list_open = true;
            state.detail_open = true;
            state.detail_route = Some(LOTTERY_DETAIL_ROUTE);
            state.selected = Some(key.clone());
        }
        app.world_mut()
            .resource_mut::<ActivityClientState>()
            .details
            .insert(key.clone(), lottery_detail(key, sample_view(), 2));
        app
    }

    fn lottery_detail(
        key: &ActivityKey,
        view: LotteryPlayerView,
        state_revision: u64,
    ) -> ActivityDetail {
        ActivityDetail {
            summary: ActivitySummary {
                activity_id: key.activity_id.clone(),
                version: key.version,
                activity_type: "lottery".to_owned(),
                status: "running".to_owned(),
                start_at_ms: 0,
                end_at_ms: 0,
                claim_deadline_ms: 0,
                timezone: "UTC".to_owned(),
            },
            progress: Some(ActivityProgressView {
                type_name: "lottery".to_owned(),
                schema_version: 1,
                value: serde_json::json!({}),
                player_view: ActivityPlayerView::Lottery(view),
            }),
            state_revision,
        }
    }

    fn draw_dispatch() -> UiActionDispatch {
        UiActionDispatch {
            action: UiActionId::from_str(ACTION_LOTTERY_DRAW).unwrap(),
            document_id: UiDocumentId::from_str(LOTTERY_DETAIL_DOCUMENT_ID).unwrap(),
            owner: OWNER_MAIN_WORLD_LOTTERY_DETAIL.as_str().to_owned(),
            source_node: UiNodeId::from_str(DRAW_NODE).unwrap(),
            kind: UiRegisteredActionKind::BusinessCommand {
                target: ACTION_LOTTERY_DRAW.to_owned(),
            },
            params: BTreeMap::<String, UiActionValue>::new(),
        }
    }

    fn read_new_commands(
        app: &App,
        cursor: &mut MessageCursor<MyServerCommand>,
    ) -> Vec<MyServerCommand> {
        cursor
            .read(app.world().resource::<Messages<MyServerCommand>>())
            .cloned()
            .collect()
    }

    fn draw_request_id(command: &MyServerCommand) -> &str {
        let MyServerCommand::RunActivityAction {
            client_request_id, ..
        } = command
        else {
            panic!("expected lottery draw command");
        };
        client_request_id
    }

    fn assert_refresh_commands(commands: Vec<MyServerCommand>, key: &ActivityKey) {
        assert!(matches!(
            commands.as_slice(),
            [
                MyServerCommand::LoadActivityDetail {
                    activity_id: detail_id,
                    version: detail_version,
                },
                MyServerCommand::LoadActivityProgress {
                    activity_id: progress_id,
                    version: progress_version,
                },
            ] if detail_id == &key.activity_id
                && *detail_version == key.version
                && progress_id == &key.activity_id
                && *progress_version == key.version
        ));
    }

    fn sample_key() -> ActivityKey {
        ActivityKey {
            activity_id: "lottery".to_owned(),
            version: 3,
        }
    }

    fn sample_view() -> LotteryPlayerView {
        LotteryPlayerView {
            draw_source: "player_action".to_owned(),
            pool_version: 3,
            free_draw_count: 2,
            daily_draw_limit: 10,
            total_draw_limit: 100,
            pool_total_weight: 10,
            state: LotteryStateView {
                free_draws_remaining: 1,
                voucher_count: 4,
                daily_draw_count: 2,
                total_draw_count: 7,
                last_draw_period_key: Some("2026-08-24".to_owned()),
                pool_version: Some(3),
                draw_request_id: None,
                result_item_id: Some(1001),
                result_state: Some("applied".to_owned()),
            },
        }
    }
}
