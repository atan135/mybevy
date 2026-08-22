pub mod chat;
pub mod mail;
mod plugin;
pub mod protocol;
mod types;

pub(crate) use plugin::{MyServerPlugin, MyServerUpdateSet};
#[cfg(test)]
pub(crate) use types::CharacterElements;
#[cfg(test)]
pub(crate) use types::CharacterProfile;
#[cfg(test)]
pub(crate) use types::GameServiceEndpoint;
#[cfg(test)]
pub(crate) use types::LoginSession;
pub(crate) use types::{
    AccountLoginState, CharacterAttributes, CharacterElementsCache, CharacterSelectionState,
    CharacterSummary, ElementValues, GameConnectionState, MovementClientState,
    MyServerAutoClientConfig, MyServerCommand, MyServerConfig, MyServerDisplayError,
    MyServerEnvironment, MyServerErrorKind, MyServerErrorSource, MyServerEvent, MyServerOperation,
    MyServerProfiles, MyServerSession, ReconnectCause,
};
// Registration contracts are intentionally re-exported for the auth host,
// which owns sensitive form inputs without exposing the underlying types module.
#[allow(unused_imports)]
pub(crate) use types::{
    RegistrationServerError, RegistrationState, RegistrationValidationError,
    normalize_registration_login_name, validate_registration_request,
};
