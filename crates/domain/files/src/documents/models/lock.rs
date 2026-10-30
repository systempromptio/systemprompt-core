//! Lock and presence rows with their requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_identifiers::{AgentName, DocumentId, DriveDeviceId, EditorId, UserId};
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

/// Why a stored `holder_kind` / `holder_id` pair is not a lock holder.
#[derive(Debug, thiserror::Error)]
pub enum LockHolderError {
    #[error(transparent)]
    Kind(#[from] UnknownDocumentValue),
    #[error("lock holder id is not valid for a {kind} holder")]
    Id {
        kind: LockHolderKind,
        #[source]
        source: IdValidationError,
    },
}

/// Who holds a document lock: the holder's kind fixes the type of its id.
///
/// Persisted as the `holder_kind` and `holder_id` columns.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LockHolder {
    Bridge(DriveDeviceId),
    Editor(EditorId),
    Agent(AgentName),
}

impl LockHolder {
    #[must_use]
    pub const fn kind(&self) -> LockHolderKind {
        match self {
            Self::Bridge(_) => LockHolderKind::Bridge,
            Self::Editor(_) => LockHolderKind::Editor,
            Self::Agent(_) => LockHolderKind::Agent,
        }
    }

    #[must_use]
    pub fn id_str(&self) -> &str {
        match self {
            Self::Bridge(id) => id.as_str(),
            Self::Editor(id) => id.as_str(),
            Self::Agent(id) => id.as_str(),
        }
    }

    pub fn from_columns(kind: &str, id: &str) -> Result<Self, LockHolderError> {
        let kind: LockHolderKind = kind.parse()?;
        let invalid = |source| LockHolderError::Id { kind, source };
        Ok(match kind {
            LockHolderKind::Bridge => Self::Bridge(DriveDeviceId::try_new(id).map_err(invalid)?),
            LockHolderKind::Editor => Self::Editor(EditorId::try_new(id).map_err(invalid)?),
            LockHolderKind::Agent => Self::Agent(AgentName::try_new(id).map_err(invalid)?),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentLock {
    pub document_id: DocumentId,
    pub holder_user_id: UserId,
    pub holder: LockHolder,
    pub acquired_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct LockRequest {
    pub document_id: DocumentId,
    pub holder_user_id: UserId,
    pub holder: LockHolder,
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
