//! The id a model issues for a tool call.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use crate::error::IdError;

const KIND: &str = "call";

/// A model-issued tool call id: opaque, never empty, echoed back to the model
/// verbatim with the result.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(try_from = "String", into = "String")]
pub struct CallId(String);

impl CallId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, IdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(IdError { kind: KIND, value });
        }
        Ok(Self(value))
    }

    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for CallId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for CallId {
    type Err = IdError;

    fn from_str(value: &str) -> Result<Self, IdError> {
        Self::try_new(value)
    }
}

impl TryFrom<String> for CallId {
    type Error = IdError;

    fn try_from(value: String) -> Result<Self, IdError> {
        Self::try_new(value)
    }
}

impl From<CallId> for String {
    fn from(id: CallId) -> Self {
        id.0
    }
}

impl AsRef<str> for CallId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
