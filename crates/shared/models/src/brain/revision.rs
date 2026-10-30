//! One append-only revision of a fact and the change that produced it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{BrainFactId, BrainFactRevisionId, UserId};

use super::status::FactStatus;

/// What a revision changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RevisionChange {
    Created,
    Revised,
    Merged,
    FieldsEdited,
    Confirmed,
    Disputed,
    DisputeResolved,
    Staled,
    Superseded,
    Retracted,
    Pinned,
    Unpinned,
    ReviewDue,
}

impl RevisionChange {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Revised => "revised",
            Self::Merged => "merged",
            Self::FieldsEdited => "fields_edited",
            Self::Confirmed => "confirmed",
            Self::Disputed => "disputed",
            Self::DisputeResolved => "dispute_resolved",
            Self::Staled => "staled",
            Self::Superseded => "superseded",
            Self::Retracted => "retracted",
            Self::Pinned => "pinned",
            Self::Unpinned => "unpinned",
            Self::ReviewDue => "review_due",
        }
    }
}

/// A fact as it stood after one change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FactRevision {
    pub id: BrainFactRevisionId,
    pub fact_id: BrainFactId,
    pub revision: i32,
    pub change: RevisionChange,
    pub actor: Option<UserId>,
    pub status: FactStatus,
    pub statement: String,
    // JSON: the fact's JSONB fields as they stood at this revision.
    pub fields: serde_json::Map<String, serde_json::Value>,
    pub recorded_at: DateTime<Utc>,
}
