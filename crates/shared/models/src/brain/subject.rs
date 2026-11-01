//! The business subject a fact is about, written `<kind>:<id>`, such as
//! `client:acme` or `service:billing`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A business subject: the subject kind a fact kind declares and the
/// subject's key within it.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct SubjectKey {
    pub kind: String,
    pub id: String,
}

/// A string that is not `<kind>:<id>` with both halves non-empty.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid subject key {0:?}: expected `<kind>:<id>`")]
pub struct SubjectKeyError(pub String);

impl SubjectKey {
    pub fn new(kind: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            id: id.into(),
        }
    }
}

impl fmt::Display for SubjectKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind, self.id)
    }
}

impl FromStr for SubjectKey {
    type Err = SubjectKeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_once(':') {
            Some((kind, id)) if !kind.is_empty() && !id.is_empty() => Ok(Self::new(kind, id)),
            _ => Err(SubjectKeyError(s.to_owned())),
        }
    }
}
