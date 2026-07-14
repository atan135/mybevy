use crate::lifecycle::{TaskFailure, TaskFailureKind};
use std::{env, ffi::OsString, fmt, sync::Arc};

const REDACTED: &str = "[REDACTED]";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialLocator {
    environment_variable: Option<String>,
    secure_store_entry: Option<String>,
}

impl CredentialLocator {
    pub fn new(
        environment_variable: Option<impl Into<String>>,
        secure_store_entry: Option<impl Into<String>>,
    ) -> Result<Self, TaskFailure> {
        let environment_variable = environment_variable.map(Into::into);
        let secure_store_entry = secure_store_entry.map(Into::into);
        if environment_variable.is_none() && secure_store_entry.is_none() {
            return Err(TaskFailure::invalid(
                "a credential locator must name an environment variable or secure-store entry",
            ));
        }
        if let Some(name) = environment_variable.as_deref() {
            validate_locator_name(name, "environment variable")?;
        }
        if let Some(name) = secure_store_entry.as_deref() {
            validate_locator_name(name, "secure-store entry")?;
        }
        Ok(Self {
            environment_variable,
            secure_store_entry,
        })
    }

    pub fn environment_variable(&self) -> Option<&str> {
        self.environment_variable.as_deref()
    }

    pub fn secure_store_entry(&self) -> Option<&str> {
        self.secure_store_entry.as_deref()
    }
}

fn validate_locator_name(value: &str, label: &str) -> Result<(), TaskFailure> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'/'))
    {
        return Err(TaskFailure::invalid(format!(
            "{label} contains unsupported characters"
        )));
    }
    Ok(())
}

pub struct SecretString(String);

impl SecretString {
    pub fn new(value: String) -> Result<Self, TaskFailure> {
        if value.is_empty() {
            return Err(credential_unavailable());
        }
        Ok(Self(value))
    }

    pub fn expose_to<T>(&self, consumer: impl FnOnce(&str) -> T) -> T {
        consumer(&self.0)
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(REDACTED)
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(REDACTED)
    }
}

pub trait SecureCredentialStore: Send + Sync {
    fn read(&self, entry: &str) -> Result<Option<SecretString>, TaskFailure>;
}

#[derive(Clone, Default)]
pub struct EnvironmentCredentialSource;

impl EnvironmentCredentialSource {
    fn read_os(&self, variable: &str) -> Option<OsString> {
        env::var_os(variable)
    }

    pub fn read(&self, variable: &str) -> Result<Option<SecretString>, TaskFailure> {
        validate_locator_name(variable, "environment variable")?;
        let Some(value) = self.read_os(variable) else {
            return Ok(None);
        };
        let value = value.into_string().map_err(|_| credential_unavailable())?;
        SecretString::new(value).map(Some)
    }
}

#[derive(Clone)]
pub struct CredentialResolver {
    environment: EnvironmentCredentialSource,
    secure_store: Option<Arc<dyn SecureCredentialStore>>,
}

impl CredentialResolver {
    pub fn environment_only() -> Self {
        Self {
            environment: EnvironmentCredentialSource,
            secure_store: None,
        }
    }

    pub fn with_secure_store(secure_store: Arc<dyn SecureCredentialStore>) -> Self {
        Self {
            environment: EnvironmentCredentialSource,
            secure_store: Some(secure_store),
        }
    }

    pub fn resolve(&self, locator: &CredentialLocator) -> Result<SecretString, TaskFailure> {
        if let Some(variable) = locator.environment_variable()
            && let Some(secret) = self.environment.read(variable)?
        {
            return Ok(secret);
        }
        if let (Some(store), Some(entry)) = (&self.secure_store, locator.secure_store_entry())
            && let Some(secret) = store.read(entry)?
        {
            return Ok(secret);
        }
        Err(credential_unavailable())
    }
}

fn credential_unavailable() -> TaskFailure {
    TaskFailure::new(
        TaskFailureKind::CredentialUnavailable,
        "the requested provider credential is unavailable",
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedSecureStore {
        secret: String,
    }

    impl SecureCredentialStore for FixedSecureStore {
        fn read(&self, _entry: &str) -> Result<Option<SecretString>, TaskFailure> {
            SecretString::new(self.secret.clone()).map(Some)
        }
    }

    #[test]
    fn secret_never_exposes_its_value_through_debug_or_display() {
        let secret = SecretString::new("fixture-secret-not-a-real-key".to_owned()).unwrap();
        assert_eq!(format!("{secret:?}"), REDACTED);
        assert_eq!(format!("{secret}"), REDACTED);
        assert_eq!(secret.expose_to(str::len), 29);
    }

    #[test]
    fn resolver_falls_back_to_a_secure_store_without_serializing_secrets() {
        let locator = CredentialLocator::new(
            Some("UI_GENERATION_TEST_VARIABLE_THAT_MUST_NOT_EXIST"),
            Some("ui-generation/test-provider"),
        )
        .unwrap();
        let resolver = CredentialResolver::with_secure_store(Arc::new(FixedSecureStore {
            secret: "fixture-secure-store-value".to_owned(),
        }));
        let secret = resolver.resolve(&locator).unwrap();
        assert_eq!(secret.expose_to(str::len), 26);
        assert!(!format!("{secret:?}").contains("fixture"));
    }

    #[test]
    fn missing_credentials_return_a_stable_redacted_failure() {
        let locator = CredentialLocator::new(
            Some("UI_GENERATION_TEST_VARIABLE_THAT_MUST_NOT_EXIST"),
            None::<String>,
        )
        .unwrap();
        let failure = CredentialResolver::environment_only()
            .resolve(&locator)
            .unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::CredentialUnavailable);
        assert!(!failure.message().contains("VARIABLE"));
    }
}
