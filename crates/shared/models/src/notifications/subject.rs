//! Resource references and audience subjects.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::UserId;

use super::attention::UnknownNotificationValue;

/// A resource a notification is about: `document`, `folder`, `comment` or an
/// extension-defined kind, with that resource's id.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SubjectRef {
    pub kind: String,
    pub id: String,
}

impl SubjectRef {
    pub fn new(kind: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            id: id.into(),
        }
    }
}

/// Who a stream, subscription or rule addresses, written
/// `user:<id> | role:<name> | <kind>:<value> | everyone`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AudienceSubject {
    User(UserId),
    Role(String),
    Dimension { kind: String, value: String },
    Everyone,
}

impl fmt::Display for AudienceSubject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User(id) => write!(f, "user:{id}"),
            Self::Role(name) => write!(f, "role:{name}"),
            Self::Dimension { kind, value } => write!(f, "{kind}:{value}"),
            Self::Everyone => f.write_str("everyone"),
        }
    }
}

impl FromStr for AudienceSubject {
    type Err = UnknownNotificationValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let unknown = || UnknownNotificationValue::new("audience_subject", s);
        if s == "everyone" {
            return Ok(Self::Everyone);
        }
        let (kind, value) = s.split_once(':').ok_or_else(unknown)?;
        if kind.is_empty() || value.is_empty() {
            return Err(unknown());
        }
        match kind {
            "user" => UserId::try_new(value)
                .map(Self::User)
                .map_err(|e| UnknownNotificationValue::invalid_id("audience_subject", s, e)),
            "role" => Ok(Self::Role(value.to_owned())),
            _ => Ok(Self::Dimension {
                kind: kind.to_owned(),
                value: value.to_owned(),
            }),
        }
    }
}

crate::notifications::string_form::string_serde!(AudienceSubject, "AudienceSubject");
