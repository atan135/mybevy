use serde::Deserialize;
use std::fmt;

pub const SCENE_ID_ALLOWED_CHARACTERS: &str =
    "lowercase ASCII letters, digits, dots, underscores, and hyphens";

macro_rules! scene_string_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_string(self) -> String {
                self.0
            }

            pub fn is_empty(&self) -> bool {
                self.0.is_empty()
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self::new(value)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

scene_string_id!(SceneId);
scene_string_id!(SceneSessionId);
scene_string_id!(SceneLayerId);
scene_string_id!(SceneAssetId);
scene_string_id!(SceneSpawnPointId);
scene_string_id!(SceneAnchorId);
scene_string_id!(SceneTriggerId);
scene_string_id!(SceneChunkId);

impl SceneId {
    pub fn validate(&self) -> Result<(), SceneIdError> {
        validate_scene_id(self.as_str())
    }

    pub fn is_valid_format(&self) -> bool {
        self.validate().is_ok()
    }
}

pub fn validate_scene_id(value: &str) -> Result<(), SceneIdError> {
    if value.is_empty() {
        return Err(SceneIdError::Empty);
    }

    if value.chars().all(is_valid_scene_id_char) {
        Ok(())
    } else {
        Err(SceneIdError::InvalidFormat(value.to_string()))
    }
}

fn is_valid_scene_id_char(value: char) -> bool {
    value.is_ascii_lowercase() || value.is_ascii_digit() || matches!(value, '.' | '_' | '-')
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneIdError {
    Empty,
    InvalidFormat(String),
}

impl fmt::Display for SceneIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("scene id must not be empty"),
            Self::InvalidFormat(value) => write!(
                formatter,
                "scene id has invalid format: {value}; allowed characters are {SCENE_ID_ALLOWED_CHARACTERS}"
            ),
        }
    }
}

impl std::error::Error for SceneIdError {}
