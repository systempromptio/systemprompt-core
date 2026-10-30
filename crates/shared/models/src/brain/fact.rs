//! A stored fact: the current projection of its latest revision.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{BrainFactId, FactKindId, TopicId, UserId};

use super::scope::FactScope;
use super::source::FactSource;
use super::status::FactStatus;
use super::subject::SubjectKey;

/// A fact as the store holds it. `statement` is rendered from the kind's
/// template; `identity_hash` keys revisions of the same fact and
/// `statement_hash` detects merges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BrainFact {
    pub id: BrainFactId,
    pub kind: FactKindId,
    pub subject: SubjectKey,
    pub scope: FactScope,
    pub statement: String,
    // JSON: fact fields are validated against the kind declaration and stored as JSONB.
    pub fields: serde_json::Map<String, serde_json::Value>,
    pub source: FactSource,
    pub topics: Vec<TopicId>,
    pub status: FactStatus,
    pub confirmed_by: Option<UserId>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub importance_base: f64,
    pub valid_from: Option<DateTime<Utc>>,
    pub valid_to: Option<DateTime<Utc>>,
    pub review_after: Option<DateTime<Utc>>,
    pub retention_until: Option<DateTime<Utc>>,
    pub legal_hold: bool,
    pub identity_hash: String,
    pub statement_hash: String,
    pub revision: i32,
    pub pinned: bool,
    pub owner_user_id: Option<UserId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl BrainFact {
    #[must_use]
    pub fn is_review_due(&self, now: DateTime<Utc>) -> bool {
        self.review_after.is_some_and(|due| due < now)
    }

    #[must_use]
    pub fn is_valid_at(&self, now: DateTime<Utc>) -> bool {
        self.valid_from.is_none_or(|from| from <= now) && self.valid_to.is_none_or(|to| now < to)
    }
}
