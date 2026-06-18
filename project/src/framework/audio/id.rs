use std::{
    error::Error,
    fmt,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_AUDIO_INSTANCE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioIdError {
    value: String,
}

impl AudioIdError {
    fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for AudioIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid audio id: {}", self.value)
    }
}

impl Error for AudioIdError {}

macro_rules! define_audio_string_id {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, AudioIdError> {
                let value = value.into();
                validate_audio_id(&value)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = AudioIdError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = AudioIdError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }
    };
}

define_audio_string_id!(AudioClipId);
define_audio_string_id!(AudioCueId);
define_audio_string_id!(AudioGroupId);
define_audio_string_id!(AudioScopeId);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AudioInstanceId(u64);

impl AudioInstanceId {
    pub fn new() -> Self {
        Self(NEXT_AUDIO_INSTANCE_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl Default for AudioInstanceId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AudioInstanceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

fn validate_audio_id(value: &str) -> Result<(), AudioIdError> {
    if value.is_empty()
        || value.starts_with(['.', '_', '-'])
        || value.ends_with(['.', '_', '-'])
        || value.contains("..")
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
    {
        return Err(AudioIdError::new(value));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_ids_accept_conservative_audio_names() {
        let clip = AudioClipId::try_from("ui.click_01").unwrap();
        let cue = AudioCueId::try_from("battle-hit.light").unwrap();
        let group = AudioGroupId::try_from("common_01").unwrap();
        let scope = AudioScopeId::try_from("scene.demo").unwrap();

        assert_eq!(clip.as_str(), "ui.click_01");
        assert_eq!(cue.to_string(), "battle-hit.light");
        assert_eq!(format!("{group}"), "common_01");
        assert_eq!(scope.to_string(), "scene.demo");
    }

    #[test]
    fn string_ids_display_their_inner_value() {
        assert_eq!(
            AudioClipId::try_from("ui.click_01").unwrap().to_string(),
            "ui.click_01"
        );
        assert_eq!(
            AudioCueId::try_from("battle-hit.light")
                .unwrap()
                .to_string(),
            "battle-hit.light"
        );
        assert_eq!(
            AudioGroupId::try_from("common_01").unwrap().to_string(),
            "common_01"
        );
        assert_eq!(
            AudioScopeId::try_from("scene.demo").unwrap().to_string(),
            "scene.demo"
        );
    }

    #[test]
    fn string_ids_reject_empty_or_unsafe_names() {
        for value in [
            "",
            ".ui.click",
            "ui.click.",
            "_ui.click",
            "ui.click_",
            "-ui.click",
            "ui.click-",
            "ui..click",
            "ui/click",
            "Ui.Click",
            "ui click",
            "ui:click",
            "按钮",
        ] {
            assert!(AudioClipId::try_from(value).is_err(), "{value} should fail");
        }
    }

    #[test]
    fn instance_id_exposes_raw_value_and_display() {
        let instance_id = AudioInstanceId::from_raw(42);

        assert_eq!(instance_id.raw(), 42);
        assert_eq!(instance_id.to_string(), "42");
    }
}
