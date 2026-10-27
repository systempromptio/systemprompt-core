//! Lock and presence rows with their requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{DocumentId, UserId};
use systemprompt_models::documents::{PresenceSource, PresenceState, UnknownDocumentValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LockHolderKind {
    Bridge,
    Editor,
    Agent,
}

impl LockHolderKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bridge => "bridge",
            Self::Editor => "editor",
            Self::Agent => "agent",
        }
    }
}

impl fmt::Display for LockHolderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LockHolderKind {
    type Err = UnknownDocumentValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "bridge" => Ok(Self::Bridge),
            "editor" => Ok(Self::Editor),
            "agent" => Ok(Self::Agent),
            other => Err(UnknownDocumentValue {
                kind: "lock holder kind",
                value: other.to_owned(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentLock {
    pub document_id: DocumentId,
    pub holder_user_id: UserId,
    pub holder_kind: LockHolderKind,
    pub holder_id: String,
    pub acquired_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct LockRequest {
    pub document_id: DocumentId,
    pub holder_user_id: UserId,
    pub holder_kind: LockHolderKind,
    pub holder_id: String,
    pub ttl: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockOutcome {
    Acquired(DocumentLock),
    Held { current: DocumentLock },
}

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentPresenceRow {
    pub document_id: DocumentId,
    pub user_id: UserId,
    pub source: PresenceSource,
    pub state: PresenceState,
    // JSON: editor cursor position; the shape belongs to the client's editor.
    pub cursor: Option<serde_json::Value>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PresenceUpsert {
    pub document_id: DocumentId,
    pub user_id: UserId,
    pub source: PresenceSource,
    pub state: PresenceState,
    // JSON: editor cursor position; the shape belongs to the client's editor.
    pub cursor: Option<serde_json::Value>,
}
