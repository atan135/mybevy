use crate::game::myserver::{
    AccountLoginState, CharacterSelectionState, CharacterSummary, ElementValues,
    GameConnectionState, MyServerDisplayError, MyServerErrorKind, MyServerErrorSource,
    MyServerOperation, MyServerSession, RegistrationState,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AuthStatusNotice {
    pub(super) kind: AuthNoticeKind,
    pub(super) title: String,
    pub(super) detail: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AuthNoticeKind {
    Maintenance,
    Banned,
    PendingReview,
    VersionIncompatible,
    Kicked,
    Network,
    GenericFailure,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct LoginUiSnapshot {
    pub(super) account_state: AccountLoginState,
    pub(super) character_state: CharacterSelectionState,
    pub(super) connection_state: GameConnectionState,
    pub(super) player_id: Option<String>,
    pub(super) login_name: Option<String>,
    pub(super) guest_id: Option<String>,
    pub(super) character_id: Option<String>,
    pub(super) pending_character_id: Option<String>,
    pub(super) characters: Vec<CharacterRowSnapshot>,
    pub(super) selected_character_name: Option<String>,
    pub(super) element_snapshot: Option<ElementSnapshot>,
    pub(super) last_error: Option<AuthErrorSnapshot>,
    pub(super) notice: Option<AuthStatusNotice>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CharacterRowSnapshot {
    pub(super) character_id: String,
    pub(super) name: String,
    pub(super) detail: String,
    pub(super) discriminator: String,
    pub(super) world_id: Option<i64>,
    pub(super) status: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ElementSnapshot {
    pub(super) affinity: ElementValues,
    pub(super) mastery: ElementValues,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AuthErrorSnapshot {
    pub(super) kind: MyServerErrorKind,
    pub(super) source: MyServerErrorSource,
    pub(super) operation: Option<MyServerOperation>,
    pub(super) message_key: &'static str,
    pub(super) error_code: Option<String>,
    pub(super) detail: Option<String>,
    pub(super) retryable: bool,
    pub(super) blocking: bool,
}

impl LoginUiSnapshot {
    pub(super) fn from_session(
        session: &MyServerSession,
        last_error: Option<&MyServerDisplayError>,
        notice: Option<&AuthStatusNotice>,
    ) -> Self {
        Self {
            account_state: session.account_login_state,
            character_state: session.character_selection_state,
            connection_state: session.game_connection_state,
            player_id: session.player_id.clone(),
            login_name: session.login_name.clone(),
            guest_id: session.guest_id.clone(),
            character_id: session.character_id.clone(),
            pending_character_id: session.pending_character_id.clone(),
            characters: session
                .characters
                .iter()
                .map(CharacterRowSnapshot::from_character)
                .collect(),
            selected_character_name: session
                .current_character
                .as_ref()
                .map(|character| character.name.clone()),
            element_snapshot: element_snapshot_for_session(session),
            last_error: last_error.map(AuthErrorSnapshot::from_display_error),
            notice: notice.cloned(),
        }
    }
}

impl CharacterRowSnapshot {
    fn from_character(character: &CharacterSummary) -> Self {
        Self {
            character_id: character.character_id.clone(),
            name: character.name.clone(),
            detail: character_display_detail(character),
            discriminator: character_discriminator(character),
            world_id: character.world_id,
            status: character
                .status
                .clone()
                .unwrap_or_else(|| "active".to_owned()),
        }
    }
}

impl AuthErrorSnapshot {
    fn from_display_error(error: &MyServerDisplayError) -> Self {
        Self {
            kind: error.kind,
            source: error.source,
            operation: error.operation,
            message_key: error.message_key,
            error_code: error.error_code.clone(),
            detail: error.detail.clone(),
            retryable: error.retryable,
            blocking: error.blocking,
        }
    }
}

pub(super) fn login_request_pending(session: &MyServerSession) -> bool {
    matches!(session.account_login_state, AccountLoginState::LoggingIn)
        || session.registration_state == RegistrationState::Registering
        || session.login_request.is_some()
}

pub(super) fn character_request_pending(session: &MyServerSession) -> bool {
    matches!(
        session.character_selection_state,
        CharacterSelectionState::Loading
            | CharacterSelectionState::Creating
            | CharacterSelectionState::LoadingProfile
            | CharacterSelectionState::Selecting
    )
}

pub(super) fn can_send_character_request(session: &MyServerSession) -> bool {
    session.account_login_state == AccountLoginState::LoggedIn
        && !character_request_pending(session)
}

pub(super) fn can_change_character(session: &MyServerSession) -> bool {
    session.account_login_state == AccountLoginState::LoggedIn
        && session.character_selection_state == CharacterSelectionState::Selected
        && !character_request_pending(session)
}

pub(super) fn login_status_text(snapshot: &LoginUiSnapshot) -> String {
    match snapshot.account_state {
        AccountLoginState::NotLoggedIn => "Not logged in".to_string(),
        AccountLoginState::LoggingIn => "Logging in...".to_string(),
        AccountLoginState::LoggedIn => {
            if let Some(login_name) = snapshot.login_name.as_deref() {
                format!("Logged in as {login_name}")
            } else if let Some(guest_id) = snapshot.guest_id.as_deref() {
                format!("Guest {guest_id}")
            } else if let Some(player_id) = snapshot.player_id.as_deref() {
                format!("Player {player_id}")
            } else {
                "Logged in".to_string()
            }
        }
        AccountLoginState::LoginFailed => "Login failed".to_string(),
        AccountLoginState::Blocked => "Account blocked".to_string(),
        AccountLoginState::Expired => "Session expired".to_string(),
        AccountLoginState::LoggedOut => "Logged out".to_string(),
    }
}

pub(super) fn element_snapshot_for_session(session: &MyServerSession) -> Option<ElementSnapshot> {
    let character_id = session.character_id.as_deref()?;
    if session.character_elements.character_id.as_deref() != Some(character_id) {
        return None;
    }

    session
        .character_elements
        .snapshot_refreshed_at
        .map(|_| ElementSnapshot {
            affinity: session.character_elements.affinity,
            mastery: session.character_elements.mastery,
        })
}

pub(super) fn auth_error_title(error: &AuthErrorSnapshot) -> String {
    match error.kind {
        MyServerErrorKind::Maintenance => "Server maintenance".to_string(),
        MyServerErrorKind::AccountBlocked
        | MyServerErrorKind::IpBlocked
        | MyServerErrorKind::PlayerBlocked => "Account blocked".to_string(),
        MyServerErrorKind::AccountBanned => "Account banned".to_string(),
        MyServerErrorKind::PendingReview => "Account under review".to_string(),
        MyServerErrorKind::VersionIncompatible => "Version incompatible".to_string(),
        MyServerErrorKind::CharacterUnavailable => "Character unavailable".to_string(),
        MyServerErrorKind::TicketExpired => "Ticket expired".to_string(),
        MyServerErrorKind::GameAuthRejected => "Game authentication failed".to_string(),
        MyServerErrorKind::SessionKicked => "Signed out elsewhere".to_string(),
        MyServerErrorKind::ConnectionTimeout | MyServerErrorKind::TransportFailed => {
            "Network unavailable".to_string()
        }
        MyServerErrorKind::Unauthorized => "Session expired".to_string(),
        _ => operation_failure_title(error.operation),
    }
}

pub(super) fn operation_failure_title(operation: Option<MyServerOperation>) -> String {
    match operation {
        Some(
            MyServerOperation::Login | MyServerOperation::GuestLogin | MyServerOperation::Register,
        ) => "Login failed".to_string(),
        Some(
            MyServerOperation::CharacterList
            | MyServerOperation::CharacterCreate
            | MyServerOperation::CharacterProfile
            | MyServerOperation::CharacterSelect,
        ) => "Character request failed".to_string(),
        Some(MyServerOperation::TicketRefresh) => "Ticket request failed".to_string(),
        Some(MyServerOperation::GameConnect | MyServerOperation::GameRequest) => {
            "Network unavailable".to_string()
        }
        _ => "Request failed".to_string(),
    }
}

pub(super) fn auth_error_detail(error: &AuthErrorSnapshot) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(detail) = error
        .detail
        .as_deref()
        .filter(|detail| !detail.trim().is_empty())
    {
        parts.push(detail.to_string());
    } else {
        parts.push(error_message_fallback(error.kind).to_string());
    }
    if let Some(code) = error
        .error_code
        .as_deref()
        .filter(|code| !code.trim().is_empty())
    {
        parts.push(format!("Code {code}."));
    }
    if error.retryable {
        parts.push("You can retry this operation.".to_string());
    }
    Some(parts.join(" "))
}

pub(super) fn error_message_fallback(kind: MyServerErrorKind) -> &'static str {
    match kind {
        MyServerErrorKind::Maintenance => "The server is temporarily closed for maintenance.",
        MyServerErrorKind::AccountBanned => "This account cannot enter the game.",
        MyServerErrorKind::PendingReview => "This account is waiting for review.",
        MyServerErrorKind::VersionIncompatible => "Update the client before entering.",
        MyServerErrorKind::SessionKicked => "This session was kicked by the server.",
        MyServerErrorKind::CharacterUnavailable => "Choose another character or contact support.",
        MyServerErrorKind::TicketExpired => "Issue a fresh character ticket before entering.",
        MyServerErrorKind::ConnectionTimeout | MyServerErrorKind::TransportFailed => {
            "Check the network connection and try again."
        }
        _ => "The request could not be completed.",
    }
}

pub(super) fn version_notice_detail(
    message: &str,
    required_version: Option<&str>,
    current_version: Option<&str>,
) -> String {
    let mut detail = message.to_string();
    if let Some(required) = required_version.filter(|value| !value.trim().is_empty()) {
        detail.push_str(&format!(" Required {required}."));
    }
    if let Some(current) = current_version.filter(|value| !value.trim().is_empty()) {
        detail.push_str(&format!(" Current {current}."));
    }
    detail
}

pub(super) fn account_status_notice(code: &str, message: &str) -> AuthStatusNotice {
    let normalized_code = normalize_status_code(code);
    let detail = Some(format!("{message} ({code})"));

    if is_pending_review_status(&normalized_code) {
        return AuthStatusNotice {
            kind: AuthNoticeKind::PendingReview,
            title: "Account requires review".to_string(),
            detail,
        };
    }

    if is_kicked_status(&normalized_code) {
        return AuthStatusNotice {
            kind: AuthNoticeKind::Kicked,
            title: "Signed out elsewhere".to_string(),
            detail,
        };
    }

    AuthStatusNotice {
        kind: AuthNoticeKind::GenericFailure,
        title: "Account blocked".to_string(),
        detail,
    }
}

pub(super) fn is_pending_review_status(normalized_code: &str) -> bool {
    normalized_code.starts_with("REGISTER_") || normalized_code.contains("PENDING_REVIEW")
}

pub(super) fn is_kicked_status(normalized_code: &str) -> bool {
    normalized_code.contains("KICK") || normalized_code.contains("CONCURRENT")
}

pub(super) fn normalize_status_code(value: &str) -> String {
    let mut code = String::new();
    let mut last_was_underscore = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            code.push(ch.to_ascii_uppercase());
            last_was_underscore = false;
        } else if !last_was_underscore && !code.is_empty() {
            code.push('_');
            last_was_underscore = true;
        }
    }
    while code.ends_with('_') {
        code.pop();
    }
    code
}

impl AuthStatusNotice {
    pub(super) fn generic_failure(title: impl Into<String>, detail: Option<String>) -> Self {
        Self {
            kind: AuthNoticeKind::GenericFailure,
            title: title.into(),
            detail,
        }
    }
}

pub(super) fn character_display_detail(character: &CharacterSummary) -> String {
    let discriminator = character_discriminator(character);

    let world = character
        .world_id
        .map(|world_id| format!("World {world_id}"))
        .unwrap_or_else(|| "World unknown".to_string());
    let status = character.status.as_deref().unwrap_or("active");
    format!("{discriminator} · {world} · {status}")
}

fn character_discriminator(character: &CharacterSummary) -> String {
    character
        .display_discriminator
        .as_deref()
        .or(character.character_id_short.as_deref())
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("#{value}"))
        .unwrap_or_else(|| short_character_id(&character.character_id))
}

pub(super) fn short_character_id(character_id: &str) -> String {
    let suffix: String = character_id
        .chars()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if suffix.is_empty() {
        "#unknown".to_string()
    } else {
        format!("#{suffix}")
    }
}
