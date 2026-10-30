//! [`FactChange`]: every way a stored fact may change, each one recorded
//! as a revision by `FactRepository::apply`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{BrainFactId, TopicId, UserId};
use systemprompt_models::brain::{FactSource, FactStatus, RevisionChange};

/// One change to a fact. Revisions carry a new statement, fields and
/// source; the lifecycle changes move status, confirmation, pin or review
/// state.
#[derive(Debug, Clone, PartialEq)]
pub enum FactChange {
    Revised {
        statement: String,
        // JSON: fact fields are validated against the kind before they reach the store.
        fields: serde_json::Map<String, serde_json::Value>,
        source: FactSource,
        topics: Vec<TopicId>,
        status: FactStatus,
        importance_base: f64,
        review_after: Option<DateTime<Utc>>,
        actor: Option<UserId>,
    },
    Merged {
        topics: Vec<TopicId>,
        actor: Option<UserId>,
    },
    FieldsEdited {
        statement: String,
        // JSON: fact fields are validated against the kind before they reach the store.
        fields: serde_json::Map<String, serde_json::Value>,
        actor: UserId,
    },
    Confirmed {
        by: UserId,
        review_after: Option<DateTime<Utc>>,
    },
    Disputed {
        actor: Option<UserId>,
    },
    DisputeResolved {
        status: FactStatus,
        actor: UserId,
    },
    Staled,
    Superseded {
        actor: Option<UserId>,
    },
    Retracted {
        actor: UserId,
    },
    Pinned {
        actor: UserId,
    },
    Unpinned {
        actor: UserId,
    },
    ReviewDue,
    Expired,
}

impl FactChange {
    #[must_use]
    pub const fn revision_change(&self) -> RevisionChange {
        match self {
            Self::Revised { .. } => RevisionChange::Revised,
            Self::Merged { .. } => RevisionChange::Merged,
            Self::FieldsEdited { .. } => RevisionChange::FieldsEdited,
            Self::Confirmed { .. } => RevisionChange::Confirmed,
            Self::Disputed { .. } => RevisionChange::Disputed,
            Self::DisputeResolved { .. } => RevisionChange::DisputeResolved,
            Self::Staled => RevisionChange::Staled,
            Self::Superseded { .. } => RevisionChange::Superseded,
            Self::Retracted { .. } => RevisionChange::Retracted,
            Self::Pinned { .. } => RevisionChange::Pinned,
            Self::Unpinned { .. } => RevisionChange::Unpinned,
            Self::ReviewDue => RevisionChange::ReviewDue,
            Self::Expired => RevisionChange::Expired,
        }
    }

    #[must_use]
    pub const fn new_status(&self) -> Option<FactStatus> {
        match self {
            Self::Revised { status, .. } | Self::DisputeResolved { status, .. } => Some(*status),
            Self::Confirmed { .. } => Some(FactStatus::Confirmed),
            Self::Disputed { .. } => Some(FactStatus::Disputed),
            Self::Staled => Some(FactStatus::Stale),
            Self::Superseded { .. } => Some(FactStatus::Superseded),
            Self::Retracted { .. } | Self::Expired => Some(FactStatus::Retired),
            Self::Merged { .. }
            | Self::FieldsEdited { .. }
            | Self::Pinned { .. }
            | Self::Unpinned { .. }
            | Self::ReviewDue => None,
        }
    }

    #[must_use]
    pub const fn new_pinned(&self) -> Option<bool> {
        match self {
            Self::Pinned { .. } => Some(true),
            Self::Unpinned { .. } => Some(false),
            _ => None,
        }
    }

    #[must_use]
    pub const fn actor(&self) -> Option<&UserId> {
        match self {
            Self::Revised { actor, .. }
            | Self::Merged { actor, .. }
            | Self::Disputed { actor }
            | Self::Superseded { actor } => actor.as_ref(),
            Self::FieldsEdited { actor, .. }
            | Self::DisputeResolved { actor, .. }
            | Self::Retracted { actor }
            | Self::Pinned { actor }
            | Self::Unpinned { actor } => Some(actor),
            Self::Confirmed { by, .. } => Some(by),
            Self::Staled | Self::ReviewDue | Self::Expired => None,
        }
    }
}

/// A revision row as `FactRevisionRepository::insert` writes it.
#[derive(Debug, Clone, PartialEq)]
pub struct NewRevision {
    pub fact_id: BrainFactId,
    pub revision: i32,
    pub change: RevisionChange,
    pub actor: Option<UserId>,
    pub status: FactStatus,
    pub statement: String,
    // JSON: the fact's JSONB fields as they stood at this revision.
    pub fields: serde_json::Map<String, serde_json::Value>,
}
