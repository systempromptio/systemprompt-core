//! [`NewFact`]: a validated, routed fact the pipeline inserts as a
//! candidate, and [`Inserted`], what the insert answered.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{BrainFactId, PipelineVersion, UserId};
use systemprompt_models::brain::{FactCandidate, FactScope, FactStatus, statement_hash};

/// A fact ready to store: the candidate, its routed scope, its rendered
/// statement and identity, and its lifecycle settings.
#[derive(Debug, Clone, PartialEq)]
pub struct NewFact {
    pub id: BrainFactId,
    pub candidate: FactCandidate,
    pub scope: FactScope,
    pub statement: String,
    pub identity_hash: String,
    pub statement_hash: String,
    pub status: FactStatus,
    pub importance_base: f64,
    pub valid_from: Option<DateTime<Utc>>,
    pub valid_to: Option<DateTime<Utc>>,
    pub review_after: Option<DateTime<Utc>>,
    pub retention_until: Option<DateTime<Utc>>,
    pub owner_user_id: Option<UserId>,
    pub pipeline_version: Option<PipelineVersion>,
}

impl NewFact {
    #[must_use]
    pub fn builder(
        candidate: FactCandidate,
        scope: FactScope,
        statement: impl Into<String>,
        identity_hash: impl Into<String>,
    ) -> NewFactBuilder {
        let statement = statement.into();
        NewFactBuilder {
            fact: Self {
                id: BrainFactId::generate(),
                candidate,
                scope,
                statement_hash: statement_hash(&statement),
                statement,
                identity_hash: identity_hash.into(),
                status: FactStatus::Candidate,
                importance_base: 0.0,
                valid_from: None,
                valid_to: None,
                review_after: None,
                retention_until: None,
                owner_user_id: None,
                pipeline_version: None,
            },
        }
    }
}

/// Builds a [`NewFact`]; the id is minted and the statement hash derived.
#[derive(Debug, Clone)]
pub struct NewFactBuilder {
    fact: NewFact,
}

impl NewFactBuilder {
    #[must_use]
    pub const fn status(mut self, status: FactStatus) -> Self {
        self.fact.status = status;
        self
    }

    #[must_use]
    pub const fn importance_base(mut self, importance_base: f64) -> Self {
        self.fact.importance_base = importance_base;
        self
    }

    #[must_use]
    pub const fn valid(mut self, from: Option<DateTime<Utc>>, to: Option<DateTime<Utc>>) -> Self {
        self.fact.valid_from = from;
        self.fact.valid_to = to;
        self
    }

    #[must_use]
    pub const fn review_after(mut self, review_after: DateTime<Utc>) -> Self {
        self.fact.review_after = Some(review_after);
        self
    }

    #[must_use]
    pub const fn retention_until(mut self, retention_until: DateTime<Utc>) -> Self {
        self.fact.retention_until = Some(retention_until);
        self
    }

    #[must_use]
    pub fn owner(mut self, owner: UserId) -> Self {
        self.fact.owner_user_id = Some(owner);
        self
    }

    #[must_use]
    pub fn pipeline_version(mut self, version: PipelineVersion) -> Self {
        self.fact.pipeline_version = Some(version);
        self
    }

    #[must_use]
    pub fn build(self) -> NewFact {
        self.fact
    }
}

/// The fact an insert landed on: `created` is `false` when a fact with the
/// same kind and identity already existed, and `id` is that fact's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inserted {
    pub id: BrainFactId,
    pub created: bool,
}
