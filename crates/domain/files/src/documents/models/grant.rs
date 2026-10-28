//! Grant rows, the grant request and the subject pairs a visibility
//! predicate matches against.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{DocumentFolderId, DocumentId, TraceId, UserId};
use systemprompt_models::documents::AccessLevel;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantTarget {
    Document(DocumentId),
    Folder(DocumentFolderId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentGrant {
    pub id: Uuid,
    pub target: GrantTarget,
    pub subject_kind: String,
    pub subject_value: String,
    pub access_level: AccessLevel,
    pub granted_by: UserId,
    pub valid_until: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub subject_display: Option<String>,
}

impl DocumentGrant {
    #[must_use]
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        self.valid_until.is_some_and(|until| until <= now)
    }
}

#[derive(Debug, Clone)]
pub struct NewGrant {
    pub target: GrantTarget,
    pub subject_kind: String,
    pub subject_value: String,
    pub subject_display: Option<String>,
    pub access_level: AccessLevel,
    pub granted_by: UserId,
    pub valid_until: Option<DateTime<Utc>>,
    pub trace_id: TraceId,
    /// Why the grant was made, written to its audit row.
    pub reason: Option<String>,
    /// The agent that made the grant for `granted_by`, on the audit row.
    pub via_agent: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubjectPairs {
    pub kinds: Vec<String>,
    pub values: Vec<String>,
}

impl SubjectPairs {
    pub fn push(&mut self, kind: impl Into<String>, value: impl Into<String>) {
        self.kinds.push(kind.into());
        self.values.push(value.into());
    }

    #[must_use]
    pub fn with(mut self, kind: impl Into<String>, value: impl Into<String>) -> Self {
        self.push(kind, value);
        self
    }
}
