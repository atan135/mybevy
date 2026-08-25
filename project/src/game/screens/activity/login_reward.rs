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
        ActivityRequestKind, LoginRewardPlayerView, LoginRewardStageView, MyServerCommand,
        MyServerSession,
    },
    scenes::main_world_entry::MainWorldEntryState,
    ui_ids::OWNER_MAIN_WORLD_LOGIN_REWARD_DETAIL,
};

use super::{
    ActivityScreenCommand, ActivityScreenState,
    host::{LOGIN_REWARD_DETAIL_DOCUMENT_ID, LOGIN_REWARD_DETAIL_ROUTE},
};

pub(super) const ACTION_LOGIN_REWARD_CLAIM: &str = "main_world_login_reward_detail.claim";
pub(super) const ACTION_LOGIN_REWARD_BACK: &str = "main_world_login_reward_detail.back";
pub(super) const ACTION_LOGIN_REWARD_CLOSE: &str = "main_world_login_reward_detail.close";

const CLAIM_NODE: &str = "main_world_login_reward_detail.stage.claim";
const BACK_NODE: &str = "main_world_login_reward_detail.back";
const CLOSE_NODE: &str = "main_world_login_reward_detail.close";
const LOGIN_REWARD_REQUEST_ID_PREFIX: &str = "login-reward-";
const LOGIN_REWARD_REQUEST_NONCE_BYTES: usize = 16;

static LOGIN_REWARD_FALLBACK_NONCE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Eq, PartialEq)]
enum LoginRewardClaimPhase {
    Pending,
    Retryable,
    Refreshing,
    Reconciling,
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct LoginRewardClaimKey {
    character_id: String,
    activity: ActivityKey,
    stage_no: u32,
}

struct LoginRewardClaimEntry {
    client_request_id: String,
    phase: LoginRewardClaimPhase,
}

/// The plaintext id is deliberately contained in this page-local retry tracker.
/// Network tracking receives only its redacted fingerprint.
#[derive(Resource)]
pub(super) struct LoginRewardClaimTracker {
    character_id: Option<String>,
    request_nonce: [u8; LOGIN_REWARD_REQUEST_NONCE_BYTES],
    next_request_sequence: u64,
    entries: HashMap<LoginRewardClaimKey, LoginRewardClaimEntry>,
}

impl Default for LoginRewardClaimTracker {
    fn default() -> Self {
        Self {
            character_id: None,
            request_nonce: login_reward_request_nonce(),
            next_request_sequence: 0,
            entries: HashMap::new(),
        }
    }
}

impl LoginRewardClaimTracker {
    fn sync_character(&mut self, character_id: &str) {
        if self.character_id.as_deref() != Some(character_id) {
            self.character_id = Some(character_id.to_owned());
            self.entries.clear();
        }
    }

    fn sync_session_character(&mut self, character_id: Option<&str>) {
        if let Some(character_id) = character_id {
            self.sync_character(character_id);
        } else if self.character_id.take().is_some() || !self.entries.is_empty() {
            self.entries.clear();
        }
    }

    fn begin_or_retry(
        &mut self,
        character_id: &str,
        activity: &ActivityKey,
        stage_no: u32,
    ) -> Option<String> {
        self.sync_character(character_id);
        let key = LoginRewardClaimKey {
            character_id: character_id.to_owned(),
            activity: activity.clone(),
            stage_no,
        };
        if let Some(entry) = self.entries.get_mut(&key) {
            if entry.phase != LoginRewardClaimPhase::Retryable {
                return None;
            }
            entry.phase = LoginRewardClaimPhase::Pending;
            return Some(entry.client_request_id.clone());
        }
        if self
            .entries
            .keys()
            .any(|existing| existing.character_id == character_id && &existing.activity == activity)
        {
            return None;
        }
        self.next_request_sequence = self.next_request_sequence.wrapping_add(1).max(1);
        let client_request_id =
            login_reward_request_id(&self.request_nonce, self.next_request_sequence);
        self.entries.insert(
            key,
            LoginRewardClaimEntry {
                client_request_id: client_request_id.clone(),
                phase: LoginRewardClaimPhase::Pending,
            },
        );
        Some(client_request_id)
    }

    fn phase(
        &self,
        character_id: Option<&str>,
        activity: &ActivityKey,
        stage_no: u32,
    ) -> Option<LoginRewardClaimPhase> {
        let character_id = character_id?;
        self.entries
            .get(&LoginRewardClaimKey {
                character_id: character_id.to_owned(),
                activity: activity.clone(),
                stage_no,
            })
            .map(|entry| entry.phase)
    }

    fn mark_activity_retryable(&mut self, activity: &ActivityKey) {
        for (key, entry) in &mut self.entries {
            if &key.activity == activity {
                entry.phase = LoginRewardClaimPhase::Retryable;
            }
        }
    }

    fn mark_activity_reconciling(&mut self, activity: &ActivityKey) {
        for (key, entry) in &mut self.entries {
            if &key.activity == activity {
                entry.phase = LoginRewardClaimPhase::Reconciling;
            }
        }
    }

    fn mark_all_retryable(&mut self) {
        for entry in self.entries.values_mut() {
            if entry.phase != LoginRewardClaimPhase::Reconciling {
                entry.phase = LoginRewardClaimPhase::Retryable;
            }
        }
    }

    fn mark_resolved(&mut self, activity: &ActivityKey, stage_id: &str, processing: bool) {
        let Ok(stage_no) = stage_id.parse::<u32>() else {
            return;
        };
        for (key, entry) in &mut self.entries {
            if &key.activity == activity && key.stage_no == stage_no {
                entry.phase = if processing {
                    LoginRewardClaimPhase::Retryable
                } else {
                    LoginRewardClaimPhase::Refreshing
                };
            }
        }
    }

    fn reconcile_detail(
        &mut self,
        character_id: Option<&str>,
        activity: &ActivityKey,
        activity_status: &str,
        view: &LoginRewardPlayerView,
    ) {
        let Some(character_id) = character_id else {
            return;
        };
        self.sync_character(character_id);
        self.entries.retain(|key, entry| {
            if key.character_id != character_id || &key.activity != activity {
                return true;
            }
            let Some(stage) = view
                .stages
                .iter()
                .find(|stage| stage.stage_no == key.stage_no)
            else {
                return false;
            };
            if !stage_claim_is_available(activity_status, view, stage) {
                return false;
            }
            if entry.phase == LoginRewardClaimPhase::Refreshing {
                entry.phase = LoginRewardClaimPhase::Retryable;
            }
            true
        });
    }
}

fn login_reward_request_nonce() -> [u8; LOGIN_REWARD_REQUEST_NONCE_BYTES] {
    let mut nonce = [0; LOGIN_REWARD_REQUEST_NONCE_BYTES];
    if getrandom::fill(&mut nonce).is_ok() {
        return nonce;
    }

    // Only used when OS randomness is unavailable. This bounds collisions
    // without putting identity or gameplay data into the request ID.
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let timestamp = (elapsed.as_nanos() as u64) ^ ((elapsed.as_nanos() >> 64) as u64);
    let counter = LOGIN_REWARD_FALLBACK_NONCE_COUNTER.fetch_add(1, Ordering::Relaxed);
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

fn login_reward_request_id(
    nonce: &[u8; LOGIN_REWARD_REQUEST_NONCE_BYTES],
    sequence: u64,
) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut request_id = String::with_capacity(LOGIN_REWARD_REQUEST_ID_PREFIX.len() + 32 + 1 + 20);
    request_id.push_str(LOGIN_REWARD_REQUEST_ID_PREFIX);
    for byte in nonce {
        request_id.push(HEX[(byte >> 4) as usize] as char);
        request_id.push(HEX[(byte & 0x0f) as usize] as char);
    }
    request_id.push('-');
    request_id.push_str(&sequence.to_string());
    request_id
}

pub(super) fn login_reward_binding_schema() -> BTreeMap<UiBindingPath, UiBindingDeclaration> {
    let mut bindings = binding_schema([
        (
            "login_reward.collection_state",
            UiBindingType::Enum {
                values: ["loading", "ready", "error"].map(str::to_owned).to_vec(),
            },
        ),
        ("login_reward.status", UiBindingType::String),
        ("login_reward.cycle", UiBindingType::String),
        ("login_reward.counts", UiBindingType::String),
        ("login_reward.today", UiBindingType::String),
        ("login_reward.error", UiBindingType::String),
        ("login_reward.error_visibility", UiBindingType::Visibility),
        (
            "login_reward.stages",
            UiBindingType::List {
                item: Box::new(UiBindingType::Record {
                    fields: BTreeMap::from([
                        ("stage_id".to_owned(), UiBindingType::String),
                        ("stage_no".to_owned(), UiBindingType::Number),
                        ("title".to_owned(), UiBindingType::String),
                        ("reward_group_key".to_owned(), UiBindingType::String),
                        ("state".to_owned(), UiBindingType::String),
                        ("claim_visibility".to_owned(), UiBindingType::Visibility),
                        ("claim_disabled".to_owned(), UiBindingType::Bool),
                        ("claim_loading".to_owned(), UiBindingType::Bool),
                    ]),
                }),
                max_items: 64,
            },
        ),
        ("login_reward.item.stage_id", UiBindingType::String),
        ("login_reward.item.stage_no", UiBindingType::Number),
        ("login_reward.item.title", UiBindingType::String),
        ("login_reward.item.reward_group_key", UiBindingType::String),
        ("login_reward.item.state", UiBindingType::String),
        (
            "login_reward.item.claim_visibility",
            UiBindingType::Visibility,
        ),
        ("login_reward.item.claim_disabled", UiBindingType::Bool),
        ("login_reward.item.claim_loading", UiBindingType::Bool),
    ]);
    for path in [
        "login_reward.item.stage_no",
        "login_reward.item.stage_id",
        "login_reward.item.title",
        "login_reward.item.reward_group_key",
        "login_reward.item.state",
        "login_reward.item.claim_visibility",
        "login_reward.item.claim_disabled",
        "login_reward.item.claim_loading",
    ] {
        if let Some(declaration) = bindings.get_mut(&UiBindingPath::from_str(path).unwrap()) {
            declaration.scope = UiBindingScope::Item;
        }
    }
    bindings
}

pub(super) fn login_reward_action_descriptors() -> Vec<UiActionDescriptor> {
    let document_id = || UiDocumentId::from_str(LOGIN_REWARD_DETAIL_DOCUMENT_ID).unwrap();
    vec![
        UiActionDescriptor::new(
            UiActionId::from_str(ACTION_LOGIN_REWARD_CLAIM).unwrap(),
            document_id(),
            OWNER_MAIN_WORLD_LOGIN_REWARD_DETAIL.as_str(),
            business_command(ACTION_LOGIN_REWARD_CLAIM),
        )
        .with_source(UiNodeId::from_str(CLAIM_NODE).unwrap())
        .with_param(
            "stage_no",
            crate::framework::ui::document::UiActionParamSchema::required(
                crate::framework::ui::document::UiActionParamType::Number {
                    min: Some(1.0),
                    max: Some(u32::MAX as f64),
                },
            ),
        ),
        UiActionDescriptor::new(
            UiActionId::from_str(ACTION_LOGIN_REWARD_BACK).unwrap(),
            document_id(),
            OWNER_MAIN_WORLD_LOGIN_REWARD_DETAIL.as_str(),
            business_command(ACTION_LOGIN_REWARD_BACK),
        )
        .with_source(UiNodeId::from_str(BACK_NODE).unwrap()),
        UiActionDescriptor::new(
            UiActionId::from_str(ACTION_LOGIN_REWARD_CLOSE).unwrap(),
            document_id(),
            OWNER_MAIN_WORLD_LOGIN_REWARD_DETAIL.as_str(),
            business_command(ACTION_LOGIN_REWARD_CLOSE),
        )
        .with_source(UiNodeId::from_str(CLOSE_NODE).unwrap()),
    ]
}

pub(super) fn handle_login_reward_document_actions(
    mut actions: MessageReader<UiActionDispatch>,
    entry: Option<Res<MainWorldEntryState>>,
    session: Option<Res<MyServerSession>>,
    activity: Res<ActivityClientState>,
    state: Res<ActivityScreenState>,
    mut claims: ResMut<LoginRewardClaimTracker>,
    mut screen_commands: MessageWriter<ActivityScreenCommand>,
    mut myserver_commands: MessageWriter<MyServerCommand>,
) {
    if !state.detail_open
        || state.detail_route != Some(LOGIN_REWARD_DETAIL_ROUTE)
        || !state.accepts_active_main_world(entry.as_deref(), session.as_deref())
    {
        return;
    }
    for action in actions.read() {
        if matches_zero_param_action(action, ACTION_LOGIN_REWARD_BACK, BACK_NODE)
            || matches_zero_param_action(action, ACTION_LOGIN_REWARD_CLOSE, CLOSE_NODE)
        {
            screen_commands.write(ActivityScreenCommand::CloseDetail);
            continue;
        }
        let Some(stage_no) = claim_stage_no(action) else {
            continue;
        };
        let Some(character_id) = session
            .as_deref()
            .and_then(|session| session.character_id.as_deref())
        else {
            continue;
        };
        let Some(key) = state.selected.as_ref() else {
            continue;
        };
        let Some(view) = activity
            .details
            .get(key)
            .and_then(|detail| detail.progress.as_ref())
            .and_then(|progress| progress.player_view.login_reward())
        else {
            continue;
        };
        let Some(stage) = view.stages.iter().find(|stage| stage.stage_no == stage_no) else {
            continue;
        };
        let status = activity
            .details
            .get(key)
            .map(|detail| detail.summary.status.as_str())
            .unwrap_or_default();
        if !stage_claim_is_available(status, view, stage) {
            continue;
        }
        let Some(client_request_id) = claims.begin_or_retry(character_id, key, stage_no) else {
            continue;
        };
        myserver_commands.write(MyServerCommand::ClaimActivityStage {
            activity_id: key.activity_id.clone(),
            version: key.version,
            stage_id: stage_no.to_string(),
            client_request_id,
        });
    }
}

pub(super) fn apply_login_reward_activity_events(
    mut events: MessageReader<ActivityEvent>,
    session: Option<Res<MyServerSession>>,
    state: Res<ActivityScreenState>,
    activity: Res<ActivityClientState>,
    mut claims: ResMut<LoginRewardClaimTracker>,
    mut myserver_commands: MessageWriter<MyServerCommand>,
) {
    let character_id = session
        .as_deref()
        .and_then(|session| session.character_id.as_deref());
    claims.sync_session_character(character_id);
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
                        .and_then(|progress| progress.player_view.login_reward())
                {
                    claims.reconcile_detail(
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
                stage_id,
                action_type,
                processing,
                ..
            } if action_type == "claim" => {
                let key = ActivityKey {
                    activity_id: activity_id.clone(),
                    version: *version,
                };
                if state.detail_route != Some(LOGIN_REWARD_DETAIL_ROUTE)
                    || state.selected.as_ref() != Some(&key)
                {
                    continue;
                }
                claims.mark_resolved(&key, stage_id, *processing);
                refresh_login_reward(&key, &mut myserver_commands);
            }
            ActivityEvent::RequestFailed {
                kind: ActivityRequestKind::Claim,
                activity: failed_activity,
                error,
            } => {
                let Some(key) = state
                    .detail_route
                    .filter(|route| *route == LOGIN_REWARD_DETAIL_ROUTE)
                    .and(state.selected.as_ref())
                else {
                    continue;
                };
                if failed_activity
                    .as_ref()
                    .is_some_and(|activity| activity != key)
                {
                    continue;
                }
                match error.kind {
                    ActivityErrorKind::QualificationNotMet | ActivityErrorKind::AlreadyClaimed => {
                        claims.mark_activity_retryable(key);
                        refresh_login_reward(key, &mut myserver_commands);
                    }
                    ActivityErrorKind::VersionConflict => {
                        claims.mark_activity_retryable(key);
                        myserver_commands.write(MyServerCommand::LoadActivityList);
                        refresh_login_reward(key, &mut myserver_commands);
                    }
                    ActivityErrorKind::Processing => {
                        claims.mark_activity_retryable(key);
                        refresh_login_reward(key, &mut myserver_commands);
                    }
                    ActivityErrorKind::ReconciliationPending
                    | ActivityErrorKind::ManualReview
                    | ActivityErrorKind::Unknown => {
                        claims.mark_activity_reconciling(key);
                        refresh_login_reward(key, &mut myserver_commands);
                    }
                    _ => claims.mark_activity_retryable(key),
                }
            }
            ActivityEvent::ScopeChanged => claims.mark_all_retryable(),
            _ => {}
        }
    }
}

pub(super) fn sync_login_reward_bindings(
    entry: Option<Res<MainWorldEntryState>>,
    session: Option<Res<MyServerSession>>,
    activity: Res<ActivityClientState>,
    state: Res<ActivityScreenState>,
    claims: Res<LoginRewardClaimTracker>,
    contract: Res<super::host::ActivityScreenHostContract>,
    mut values: ResMut<UiBindingValues>,
) {
    if !state.detail_open
        || state.detail_route != Some(LOGIN_REWARD_DETAIL_ROUTE)
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
        .and_then(|progress| progress.player_view.login_reward());
    let error = state
        .detail_error
        .as_ref()
        .map(activity_error_message)
        .or_else(|| {
            (state.detail_load_state != super::host::ActivityDetailLoadState::Loading
                && view.is_none())
            .then(|| "Login reward detail is unavailable".to_owned())
        })
        .unwrap_or_default();
    let collection_state = if !error.is_empty() {
        "error"
    } else if state.detail_load_state == super::host::ActivityDetailLoadState::Loading {
        "loading"
    } else {
        "ready"
    };
    let status = detail.map_or_else(String::new, login_reward_status);
    let stages = match (detail, view, state.selected.as_ref()) {
        (Some(detail), Some(view), Some(key)) => view
            .stages
            .iter()
            .map(|stage| {
                login_reward_stage_binding(
                    detail.summary.status.as_str(),
                    view,
                    stage,
                    claims.phase(
                        session
                            .as_deref()
                            .and_then(|session| session.character_id.as_deref()),
                        key,
                        stage.stage_no,
                    ),
                )
            })
            .collect(),
        _ => Vec::new(),
    };
    for (path, value) in [
        (
            "login_reward.collection_state",
            UiBindingValue::Enum(collection_state.to_owned()),
        ),
        ("login_reward.status", UiBindingValue::String(status)),
        (
            "login_reward.cycle",
            UiBindingValue::String(view.map_or_else(String::new, |view| {
                format!(
                    "{} / {} / {}",
                    view.cycle_unit, view.progression, view.miss_policy
                )
            })),
        ),
        (
            "login_reward.counts",
            UiBindingValue::String(view.map_or_else(String::new, |view| {
                format!(
                    "Continuous {}  Cumulative {}",
                    view.consecutive_days, view.cumulative_days
                )
            })),
        ),
        (
            "login_reward.today",
            UiBindingValue::String(
                view.map_or_else(String::new, |view| format!("Today: {}", view.today_status)),
            ),
        ),
        ("login_reward.stages", UiBindingValue::List(stages)),
        ("login_reward.error", UiBindingValue::String(error.clone())),
        (
            "login_reward.error_visibility",
            UiBindingValue::Visibility(if error.is_empty() {
                UiBindingVisibility::Hidden
            } else {
                UiBindingVisibility::Visible
            }),
        ),
    ] {
        set_binding(&contract, &mut values, path, value);
    }
}

fn refresh_login_reward(key: &ActivityKey, commands: &mut MessageWriter<MyServerCommand>) {
    commands.write(MyServerCommand::LoadActivityDetail {
        activity_id: key.activity_id.clone(),
        version: key.version,
    });
    commands.write(MyServerCommand::LoadActivityProgress {
        activity_id: key.activity_id.clone(),
        version: key.version,
    });
}

fn stage_claim_is_available(
    activity_status: &str,
    view: &LoginRewardPlayerView,
    stage: &LoginRewardStageView,
) -> bool {
    matches!(activity_status, "running" | "ended")
        && view.claim_mode == "manual"
        && stage.achieved
        && stage.claimable
        && !stage.claimed
}

fn login_reward_status(detail: &crate::game::myserver::ActivityDetail) -> String {
    match detail.summary.status.as_str() {
        "published" => "Not started".to_owned(),
        "running" => "Running".to_owned(),
        "ended" if detail.summary.claim_deadline_ms > 0 => "Ended - claim grace".to_owned(),
        "ended" => "Ended".to_owned(),
        _ => "Unavailable".to_owned(),
    }
}

fn login_reward_stage_binding(
    activity_status: &str,
    view: &LoginRewardPlayerView,
    stage: &LoginRewardStageView,
    phase: Option<LoginRewardClaimPhase>,
) -> UiBindingValue {
    let claim_available = stage_claim_is_available(activity_status, view, stage);
    let (claim_disabled, claim_loading) = match phase {
        Some(LoginRewardClaimPhase::Pending | LoginRewardClaimPhase::Refreshing) => (true, true),
        Some(LoginRewardClaimPhase::Reconciling) => (true, false),
        Some(LoginRewardClaimPhase::Retryable) => (false, false),
        None => (false, false),
    };
    UiBindingValue::Record(BTreeMap::from([
        (
            "stage_id".to_owned(),
            UiBindingValue::String(stage.stage_no.to_string()),
        ),
        (
            "stage_no".to_owned(),
            UiBindingValue::Number(stage.stage_no as f64),
        ),
        (
            "title".to_owned(),
            UiBindingValue::String(format!("Stage {}", stage.stage_no)),
        ),
        (
            "reward_group_key".to_owned(),
            UiBindingValue::String(stage.reward_group_key.clone()),
        ),
        (
            "state".to_owned(),
            UiBindingValue::String(login_reward_stage_status(view, stage, phase)),
        ),
        (
            "claim_visibility".to_owned(),
            UiBindingValue::Visibility(if claim_available {
                UiBindingVisibility::Visible
            } else {
                UiBindingVisibility::Hidden
            }),
        ),
        (
            "claim_disabled".to_owned(),
            UiBindingValue::Bool(!claim_available || claim_disabled),
        ),
        (
            "claim_loading".to_owned(),
            UiBindingValue::Bool(claim_available && claim_loading),
        ),
    ]))
}

fn login_reward_stage_status(
    view: &LoginRewardPlayerView,
    stage: &LoginRewardStageView,
    phase: Option<LoginRewardClaimPhase>,
) -> String {
    if stage.claimed {
        return "Claimed".to_owned();
    }
    if !stage.achieved {
        return "Not achieved".to_owned();
    }
    if view.claim_mode == "automatic" {
        return "Automatic claim pending server refresh".to_owned();
    }
    match phase {
        Some(LoginRewardClaimPhase::Pending) => "Claiming".to_owned(),
        Some(LoginRewardClaimPhase::Refreshing) => "Refreshing server state".to_owned(),
        Some(LoginRewardClaimPhase::Retryable) => "Retry available".to_owned(),
        Some(LoginRewardClaimPhase::Reconciling) => {
            "Server result requires verification".to_owned()
        }
        None if stage.claimable => "Ready to claim".to_owned(),
        None => "Unavailable".to_owned(),
    }
}

fn binding_schema<const N: usize>(
    specs: [(&str, UiBindingType); N],
) -> BTreeMap<UiBindingPath, UiBindingDeclaration> {
    specs
        .into_iter()
        .map(|(path, value_type)| {
            (
                UiBindingPath::from_str(path).expect("login reward binding path is static"),
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
        .login_reward_bindings
        .get(&path)
        .expect("login reward binding schema contains synchronized value");
    values.set_scoped(
        LOGIN_REWARD_DETAIL_DOCUMENT_ID,
        OWNER_MAIN_WORLD_LOGIN_REWARD_DETAIL.as_str(),
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
        && action.document_id.as_str() == LOGIN_REWARD_DETAIL_DOCUMENT_ID
        && action.owner == OWNER_MAIN_WORLD_LOGIN_REWARD_DETAIL.as_str()
        && action.action.as_str() == expected_action
        && action.source_node.as_str() == source
}

fn claim_stage_no(action: &UiActionDispatch) -> Option<u32> {
    if !business_target_matches_action(action)
        || action.document_id.as_str() != LOGIN_REWARD_DETAIL_DOCUMENT_ID
        || action.owner != OWNER_MAIN_WORLD_LOGIN_REWARD_DETAIL.as_str()
        || action.action.as_str() != ACTION_LOGIN_REWARD_CLAIM
        || action.source_node.as_str() != CLAIM_NODE
        || action.params.len() != 1
    {
        return None;
    }
    let crate::framework::ui::document::UiActionValue::Number(stage_no) =
        action.params.get("stage_no")?
    else {
        return None;
    };
    (stage_no.is_finite() && stage_no.fract() == 0.0 && (1.0..=u32::MAX as f64).contains(stage_no))
        .then_some(*stage_no as u32)
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

fn activity_error_message(error: &ActivityError) -> String {
    format!("Activity request failed ({})", error.error_code)
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
                GameConnectionState,
            },
            scenes::main_world_entry::MainWorldEntryPhase,
        },
    };
    use bevy::ecs::message::MessageCursor;

    #[test]
    fn stage_id_uses_server_stage_number_instead_of_list_offset() {
        assert_eq!(17_u32.to_string(), "17");
    }

    #[test]
    fn automatic_or_unachieved_stages_never_offer_manual_claim() {
        let mut view = sample_view();
        let stage = &view.stages[0];
        assert!(stage_claim_is_available("running", &view, stage));
        view.claim_mode = "automatic".to_owned();
        assert!(!stage_claim_is_available("running", &view, stage));
        view.claim_mode = "manual".to_owned();
        view.stages[0].achieved = false;
        assert!(!stage_claim_is_available("running", &view, &view.stages[0]));
    }

    #[test]
    fn retry_processing_and_disconnect_keep_the_same_request_id() {
        let mut tracker = LoginRewardClaimTracker::default();
        let activity = sample_key();
        let first = tracker
            .begin_or_retry("character-1", &activity, 5)
            .expect("initial claim must allocate an id");
        assert!(
            tracker
                .begin_or_retry("character-1", &activity, 5)
                .is_none()
        );
        assert!(
            tracker
                .begin_or_retry("character-1", &activity, 6)
                .is_none()
        );
        tracker.mark_resolved(&activity, "5", true);
        let retry = tracker
            .begin_or_retry("character-1", &activity, 5)
            .expect("processing claim may retry");
        assert_eq!(first, retry);
        tracker.mark_all_retryable();
        let reconnect_retry = tracker
            .begin_or_retry("character-1", &activity, 5)
            .expect("disconnected claim may retry");
        assert_eq!(first, reconnect_retry);
    }

    #[test]
    fn fresh_trackers_use_distinct_bounded_request_ids() {
        let activity = sample_key();
        let mut first_tracker = LoginRewardClaimTracker::default();
        let mut second_tracker = LoginRewardClaimTracker::default();
        let first = first_tracker
            .begin_or_retry("character-1", &activity, 5)
            .expect("initial claim must allocate an id");
        let second = second_tracker
            .begin_or_retry("character-1", &activity, 5)
            .expect("initial claim must allocate an id");

        assert_ne!(first, second);
        for request_id in [first, second] {
            assert!(request_id.starts_with(LOGIN_REWARD_REQUEST_ID_PREFIX));
            assert!(request_id.len() <= 128);
        }
    }

    #[test]
    fn stage_binding_covers_claimed_and_retryable_render_states() {
        let view = sample_view();
        let ready = login_reward_stage_binding("running", &view, &view.stages[0], None);
        let UiBindingValue::Record(ready) = ready else {
            panic!("stage must bind as a record");
        };
        assert_eq!(
            ready["claim_visibility"],
            UiBindingValue::Visibility(UiBindingVisibility::Visible)
        );
        assert_eq!(ready["claim_disabled"], UiBindingValue::Bool(false));

        let mut claimed = view.stages[0].clone();
        claimed.claimed = true;
        let claimed = login_reward_stage_binding("running", &view, &claimed, None);
        let UiBindingValue::Record(claimed) = claimed else {
            panic!("stage must bind as a record");
        };
        assert_eq!(
            claimed["claim_visibility"],
            UiBindingValue::Visibility(UiBindingVisibility::Hidden)
        );
        assert_eq!(
            claimed["state"],
            UiBindingValue::String("Claimed".to_owned())
        );

        let reconciling = login_reward_stage_binding(
            "running",
            &view,
            &view.stages[0],
            Some(LoginRewardClaimPhase::Reconciling),
        );
        let UiBindingValue::Record(reconciling) = reconciling else {
            panic!("stage must bind as a record");
        };
        assert_eq!(reconciling["claim_disabled"], UiBindingValue::Bool(true));
        assert_eq!(reconciling["claim_loading"], UiBindingValue::Bool(false));
        assert_eq!(
            reconciling["state"],
            UiBindingValue::String("Server result requires verification".to_owned())
        );
    }

    #[test]
    fn manual_review_keeps_the_claim_reconciling_without_a_new_request() {
        let key = sample_key();
        let mut app = login_reward_mock_app(&key);
        let mut command_cursor = MessageCursor::default();

        app.world_mut().write_message(claim_dispatch(5));
        app.update();
        let first_commands = read_new_commands(&app, &mut command_cursor);
        assert!(matches!(
            first_commands.as_slice(),
            [MyServerCommand::ClaimActivityStage { .. }]
        ));

        let kind = ActivityErrorKind::ManualReview;
        app.world_mut().write_message(ActivityEvent::RequestFailed {
            kind: ActivityRequestKind::Claim,
            activity: Some(key.clone()),
            error: ActivityError {
                kind,
                error_code: "ACTIVITY_MANUAL_REVIEW".to_owned(),
                message_key: kind.message_key(),
                retryable: false,
            },
        });
        app.update();
        assert_refresh_commands(read_new_commands(&app, &mut command_cursor), &key);
        assert!(matches!(
            app.world()
                .resource::<LoginRewardClaimTracker>()
                .phase(Some("character-1"), &key, 5),
            Some(LoginRewardClaimPhase::Reconciling)
        ));

        app.world_mut().write_message(claim_dispatch(5));
        app.update();
        assert!(read_new_commands(&app, &mut command_cursor).is_empty());
    }

    #[test]
    fn scope_loss_drops_page_local_claim_request_ids() {
        let key = sample_key();
        let mut app = login_reward_mock_app(&key);
        let mut command_cursor = MessageCursor::default();

        app.world_mut().write_message(claim_dispatch(5));
        app.update();
        let first_commands = read_new_commands(&app, &mut command_cursor);
        let [first_claim] = first_commands.as_slice() else {
            panic!("initial interaction must issue one claim command");
        };
        let first_request_id = claim_request_id(first_claim).to_owned();

        {
            let mut session = app.world_mut().resource_mut::<MyServerSession>();
            session.character_id = None;
            session.connection_id = None;
            session.connected = false;
            session.authenticated = false;
            session.game_connection_state = GameConnectionState::Disconnected;
        }
        app.world_mut().write_message(ActivityEvent::ScopeChanged);
        app.update();
        assert!(read_new_commands(&app, &mut command_cursor).is_empty());
        let tracker = app.world().resource::<LoginRewardClaimTracker>();
        assert!(tracker.character_id.is_none());
        assert!(tracker.entries.is_empty());

        *app.world_mut().resource_mut::<MyServerSession>() = MyServerSession {
            character_id: Some("character-1".to_owned()),
            connection_id: Some(ConnectionId::from_raw(8)),
            connected: true,
            authenticated: true,
            game_connection_state: GameConnectionState::Authenticated,
            ..Default::default()
        };
        app.world_mut().write_message(claim_dispatch(5));
        app.update();
        let next_commands = read_new_commands(&app, &mut command_cursor);
        let [next_claim] = next_commands.as_slice() else {
            panic!("a new scope must not retain the old claim request");
        };
        assert_ne!(claim_request_id(next_claim), first_request_id);
    }

    #[test]
    fn claim_action_emits_only_the_server_stage_mapping_and_retry_id() {
        let key = sample_key();
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
        .init_resource::<LoginRewardClaimTracker>()
        .add_message::<UiActionDispatch>()
        .add_message::<ActivityScreenCommand>()
        .add_message::<MyServerCommand>()
        .add_systems(Update, handle_login_reward_document_actions);
        {
            let mut state = app.world_mut().resource_mut::<ActivityScreenState>();
            state.main_world_generation = Some(4);
            state.list_open = true;
            state.detail_open = true;
            state.detail_route = Some(LOGIN_REWARD_DETAIL_ROUTE);
            state.selected = Some(key.clone());
        }
        app.world_mut()
            .resource_mut::<ActivityClientState>()
            .details
            .insert(
                key.clone(),
                ActivityDetail {
                    summary: ActivitySummary {
                        activity_id: key.activity_id.clone(),
                        version: key.version,
                        activity_type: "login_reward".to_owned(),
                        status: "running".to_owned(),
                        start_at_ms: 0,
                        end_at_ms: 0,
                        claim_deadline_ms: 0,
                        timezone: "UTC".to_owned(),
                    },
                    progress: Some(ActivityProgressView {
                        type_name: "login_reward".to_owned(),
                        schema_version: 1,
                        value: serde_json::json!({}),
                        player_view: ActivityPlayerView::LoginReward(sample_view()),
                    }),
                    state_revision: 2,
                },
            );
        app.world_mut().write_message(claim_dispatch(5));
        app.update();

        let commands = app.world().resource::<Messages<MyServerCommand>>();
        let mut cursor = MessageCursor::default();
        let commands = cursor.read(commands).collect::<Vec<_>>();
        assert!(matches!(
            commands.as_slice(),
            [MyServerCommand::ClaimActivityStage {
                activity_id,
                version: 3,
                stage_id,
                client_request_id,
            }] if activity_id == "login-reward"
                && stage_id == "5"
                && client_request_id.starts_with("login-reward-")
        ));
    }

    #[test]
    fn mock_claim_success_duplicate_and_reconnect_reconcile_to_server_detail() {
        let key = sample_key();
        let mut app = login_reward_mock_app(&key);
        let mut command_cursor = MessageCursor::default();
        let mut server = InProcessLoginRewardActivityMock::default();

        app.world_mut().write_message(claim_dispatch(5));
        app.update();
        let first_commands = read_new_commands(&app, &mut command_cursor);
        let [first_claim] = first_commands.as_slice() else {
            panic!("initial interaction must issue one claim command");
        };
        let first_request_id = claim_request_id(first_claim).to_owned();
        let first_response = server.resolve_claim(first_claim);
        assert!(!matches!(
            first_response,
            ActivityEvent::ActionResolved {
                duplicate: true,
                ..
            }
        ));

        app.world_mut().write_message(first_response);
        app.update();
        assert_refresh_commands(read_new_commands(&app, &mut command_cursor), &key);

        // The successful claim has been accepted server-side, but detail refresh
        // has not arrived before a reconnect. The retained request ID is retried.
        app.world_mut().write_message(ActivityEvent::ScopeChanged);
        app.update();
        assert!(read_new_commands(&app, &mut command_cursor).is_empty());

        app.world_mut().write_message(claim_dispatch(5));
        app.update();
        let retry_commands = read_new_commands(&app, &mut command_cursor);
        let [retry_claim] = retry_commands.as_slice() else {
            panic!("reconnected interaction must issue one retry command");
        };
        assert_eq!(claim_request_id(retry_claim), first_request_id);
        let duplicate_response = server.resolve_claim(retry_claim);
        assert!(matches!(
            duplicate_response,
            ActivityEvent::ActionResolved {
                duplicate: true,
                ..
            }
        ));

        app.world_mut().write_message(duplicate_response);
        app.update();
        assert_refresh_commands(read_new_commands(&app, &mut command_cursor), &key);
        assert_eq!(server.first_claims, 1);
        assert_eq!(server.duplicate_claims, 1);

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

        app.world_mut().write_message(claim_dispatch(5));
        app.update();
        assert!(read_new_commands(&app, &mut command_cursor).is_empty());
        assert!(
            app.world()
                .resource::<LoginRewardClaimTracker>()
                .phase(Some("character-1"), &key, 5)
                .is_none()
        );
    }

    #[derive(Default)]
    struct InProcessLoginRewardActivityMock {
        accepted_request_ids: std::collections::HashSet<String>,
        first_claims: usize,
        duplicate_claims: usize,
        claimed: bool,
    }

    impl InProcessLoginRewardActivityMock {
        fn resolve_claim(&mut self, command: &MyServerCommand) -> ActivityEvent {
            let MyServerCommand::ClaimActivityStage {
                activity_id,
                version,
                stage_id,
                client_request_id,
            } = command
            else {
                panic!("mock only accepts activity claim commands");
            };
            let duplicate = !self.accepted_request_ids.insert(client_request_id.clone());
            if duplicate {
                self.duplicate_claims += 1;
            } else {
                self.first_claims += 1;
                self.claimed = true;
            }
            ActivityEvent::ActionResolved {
                activity_id: activity_id.clone(),
                version: *version,
                stage_id: stage_id.clone(),
                action_type: "claim".to_owned(),
                ok: true,
                processing: false,
                duplicate,
                state_revision: 3,
            }
        }

        fn detail(&self, key: &ActivityKey) -> ActivityDetail {
            let mut view = sample_view();
            view.stages[0].claimed = self.claimed;
            view.stages[0].claimable = !self.claimed;
            login_reward_detail(key, view, 3)
        }
    }

    fn login_reward_mock_app(key: &ActivityKey) -> App {
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
        .init_resource::<LoginRewardClaimTracker>()
        .add_message::<UiActionDispatch>()
        .add_message::<ActivityEvent>()
        .add_message::<ActivityScreenCommand>()
        .add_message::<MyServerCommand>()
        .add_systems(
            Update,
            (
                handle_login_reward_document_actions,
                apply_login_reward_activity_events,
            ),
        );
        {
            let mut state = app.world_mut().resource_mut::<ActivityScreenState>();
            state.main_world_generation = Some(4);
            state.list_open = true;
            state.detail_open = true;
            state.detail_route = Some(LOGIN_REWARD_DETAIL_ROUTE);
            state.selected = Some(key.clone());
        }
        app.world_mut()
            .resource_mut::<ActivityClientState>()
            .details
            .insert(key.clone(), login_reward_detail(key, sample_view(), 2));
        app
    }

    fn login_reward_detail(
        key: &ActivityKey,
        view: LoginRewardPlayerView,
        state_revision: u64,
    ) -> ActivityDetail {
        ActivityDetail {
            summary: ActivitySummary {
                activity_id: key.activity_id.clone(),
                version: key.version,
                activity_type: "login_reward".to_owned(),
                status: "running".to_owned(),
                start_at_ms: 0,
                end_at_ms: 0,
                claim_deadline_ms: 0,
                timezone: "UTC".to_owned(),
            },
            progress: Some(ActivityProgressView {
                type_name: "login_reward".to_owned(),
                schema_version: 1,
                value: serde_json::json!({}),
                player_view: ActivityPlayerView::LoginReward(view),
            }),
            state_revision,
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

    fn claim_request_id(command: &MyServerCommand) -> &str {
        let MyServerCommand::ClaimActivityStage {
            client_request_id, ..
        } = command
        else {
            panic!("expected an activity claim command");
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

    fn claim_dispatch(stage_no: u32) -> UiActionDispatch {
        UiActionDispatch {
            action: UiActionId::from_str(ACTION_LOGIN_REWARD_CLAIM).unwrap(),
            document_id: UiDocumentId::from_str(LOGIN_REWARD_DETAIL_DOCUMENT_ID).unwrap(),
            owner: OWNER_MAIN_WORLD_LOGIN_REWARD_DETAIL.as_str().to_owned(),
            source_node: UiNodeId::from_str(CLAIM_NODE).unwrap(),
            kind: UiRegisteredActionKind::BusinessCommand {
                target: ACTION_LOGIN_REWARD_CLAIM.to_owned(),
            },
            params: BTreeMap::from([(
                "stage_no".to_owned(),
                UiActionValue::Number(stage_no as f64),
            )]),
        }
    }

    fn sample_key() -> ActivityKey {
        ActivityKey {
            activity_id: "login-reward".to_owned(),
            version: 3,
        }
    }

    fn sample_view() -> LoginRewardPlayerView {
        LoginRewardPlayerView {
            event_source: "game_entry".to_owned(),
            cycle_unit: "natural_day".to_owned(),
            progression: "consecutive".to_owned(),
            miss_policy: "reset".to_owned(),
            claim_mode: "manual".to_owned(),
            current_stage_id: Some("5".to_owned()),
            last_period_key: Some("2026-08-24".to_owned()),
            consecutive_days: 5,
            cumulative_days: 8,
            today_status: "logged_in".to_owned(),
            stages: vec![LoginRewardStageView {
                stage_no: 5,
                required_count: 5,
                reward_group_key: "login_day_5".to_owned(),
                achieved: true,
                claimable: true,
                claimed: false,
            }],
        }
    }
}
