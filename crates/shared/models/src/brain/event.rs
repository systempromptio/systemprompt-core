//! The event a producer appends to the outbox for the brain consumer, and
//! the kinds the brain reads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::UserId;

pub const BRAIN_CONSUMER: &str = "brain";
pub const BRAIN_EVENT_VERSION: u32 = 1;

/// A brain event kind: `.`-separated segments of `[a-z0-9_]`, such as
/// `document.revision.landed`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BrainEventKind(String);

/// A string that is not a valid [`BrainEventKind`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid brain event kind {value:?}: {reason}")]
pub struct BrainEventKindError {
    pub value: String,
    pub reason: &'static str,
}

impl BrainEventKind {
    pub const REVISION_LANDED: &'static str = "document.revision.landed";
    pub const COMMENT_RESOLVED: &'static str = "document.comment.resolved";
    pub const FACT_ENTERED: &'static str = "brain.fact.entered";

    pub fn new(value: impl Into<String>) -> Result<Self, BrainEventKindError> {
        let value = value.into();
        if let Some(reason) = kind_violation(&value) {
            return Err(BrainEventKindError { value, reason });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn revision_landed() -> Self {
        Self(Self::REVISION_LANDED.to_owned())
    }

    #[must_use]
    pub fn comment_resolved() -> Self {
        Self(Self::COMMENT_RESOLVED.to_owned())
    }

    #[must_use]
    pub fn fact_entered() -> Self {
        Self(Self::FACT_ENTERED.to_owned())
    }

    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

fn kind_violation(value: &str) -> Option<&'static str> {
    if value.is_empty() {
        return Some("must not be empty");
    }
    if value.split('.').any(str::is_empty) {
        return Some("segments must not be empty");
    }
    if !value
        .bytes()
        .all(|b| b == b'.' || b == b'_' || b.is_ascii_lowercase() || b.is_ascii_digit())
    {
        return Some("must contain only lowercase letters, digits, '_' and '.'");
    }
    None
}

impl fmt::Display for BrainEventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for BrainEventKind {
    type Err = BrainEventKindError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

crate::notifications::string_form::string_serde!(BrainEventKind, "BrainEventKind");

/// One thing that happened that the brain may learn from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BrainEvent {
    pub kind: BrainEventKind,
    pub actor: UserId,
    pub actor_is_agent: bool,
    pub via_agent: Option<String>,
    pub occurred_at: DateTime<Utc>,
    // JSON: the producer's payload is the event's free-form contract field.
    pub payload: serde_json::Value,
}

impl BrainEvent {
    #[must_use]
    pub const fn new(kind: BrainEventKind, actor: UserId, occurred_at: DateTime<Utc>) -> Self {
        Self {
            kind,
            actor,
            actor_is_agent: false,
            via_agent: None,
            occurred_at,
            payload: serde_json::Value::Null,
        }
    }

    #[must_use]
    pub const fn with_actor_is_agent(mut self, actor_is_agent: bool) -> Self {
        self.actor_is_agent = actor_is_agent;
        self
    }

    #[must_use]
    pub fn with_via_agent(mut self, via_agent: impl Into<String>) -> Self {
        self.via_agent = Some(via_agent.into());
        self
    }

    #[must_use]
    // JSON: the producer's payload is the event's free-form contract field.
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = payload;
        self
    }
}
