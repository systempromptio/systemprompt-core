//! The provider-issued device token, redacted wherever it could be printed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const MAX_PUSH_TOKEN_LEN: usize = 4096;

/// A provider-issued device token. Opaque: read only through
/// [`PushToken::expose`], and absent from `Debug` output.
#[derive(Clone, PartialEq, Eq)]
pub struct PushToken(String);

/// Why a string is not a [`PushToken`]. The rejected value is never carried.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PushTokenError {
    #[error("a push token must not be empty")]
    Empty,
    #[error("a push token is at most {max} bytes, got {len}")]
    TooLong { len: usize, max: usize },
    #[error("a push token holds only visible ASCII characters")]
    InvalidCharacter,
}

impl PushToken {
    pub fn try_new(value: impl Into<String>) -> Result<Self, PushTokenError> {
        let value = value.into();
        if value.is_empty() {
            return Err(PushTokenError::Empty);
        }
        if value.len() > MAX_PUSH_TOKEN_LEN {
            return Err(PushTokenError::TooLong {
                len: value.len(),
                max: MAX_PUSH_TOKEN_LEN,
            });
        }
        if !value.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(PushTokenError::InvalidCharacter);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn expose(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for PushToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PushToken(<redacted>)")
    }
}

impl Serialize for PushToken {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for PushToken {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = <String as Deserialize>::deserialize(deserializer)?;
        Self::try_new(raw).map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for PushToken {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "PushToken".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string", "writeOnly": true })
    }
}
