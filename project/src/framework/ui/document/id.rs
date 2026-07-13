use serde::{Deserialize, Deserializer, Serialize};
use std::{fmt, str::FromStr};

pub const UI_ID_MAX_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiIdKind {
    Document,
    Node,
    Asset,
    Style,
    Action,
    I18n,
    Binding,
}

impl UiIdKind {
    const fn requires_namespace(self) -> bool {
        matches!(
            self,
            Self::Document | Self::Node | Self::Action | Self::I18n | Self::Binding
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiIdError {
    kind: UiIdKind,
    value: String,
}

impl UiIdError {
    pub fn kind(&self) -> UiIdKind {
        self.kind
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for UiIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid {:?} id `{}`; expected 1..={UI_ID_MAX_BYTES} ASCII bytes, lowercase snake_case segments separated by dots{}",
            self.kind,
            self.value,
            if self.kind.requires_namespace() {
                ", with at least two segments"
            } else {
                ""
            }
        )
    }
}

impl std::error::Error for UiIdError {}

fn validate_id(kind: UiIdKind, value: &str) -> Result<(), UiIdError> {
    let valid_length = !value.is_empty() && value.len() <= UI_ID_MAX_BYTES;
    let mut segment_count = 0usize;
    let valid_segments = value.split('.').all(|segment| {
        segment_count += 1;
        let mut bytes = segment.bytes();
        bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
            && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    });
    let valid_namespace = !kind.requires_namespace() || segment_count >= 2;

    if valid_length && value.is_ascii() && valid_segments && valid_namespace {
        Ok(())
    } else {
        Err(UiIdError {
            kind,
            value: value.to_owned(),
        })
    }
}

macro_rules! define_ui_id {
    ($name:ident, $kind:expr, $pattern:literal) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[cfg_attr(test, derive(schemars::JsonSchema))]
        #[serde(transparent)]
        pub struct $name(
            #[cfg_attr(
                                test,
                                schemars(length(min = 1, max = 128), regex(pattern = $pattern))
                            )]
            String,
        );

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, UiIdError> {
                let value = value.into();
                validate_id($kind, &value)?;
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

        impl FromStr for $name {
            type Err = UiIdError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

define_ui_id!(
    UiDocumentId,
    UiIdKind::Document,
    "^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)+$"
);
define_ui_id!(
    UiNodeId,
    UiIdKind::Node,
    "^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)+$"
);
define_ui_id!(
    UiAssetId,
    UiIdKind::Asset,
    "^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)*$"
);
define_ui_id!(
    UiStyleId,
    UiIdKind::Style,
    "^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)*$"
);
define_ui_id!(
    UiActionId,
    UiIdKind::Action,
    "^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)+$"
);
define_ui_id!(
    UiI18nKey,
    UiIdKind::I18n,
    "^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)+$"
);
define_ui_id!(
    UiBindingPath,
    UiIdKind::Binding,
    "^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)+$"
);
