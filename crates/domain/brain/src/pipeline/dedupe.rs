//! The dedupe stage.
//!
//! How a validated candidate relates to what the store already holds. An
//! erased identity is refused; the same identity revises the fact or, with
//! an unchanged statement, leaves it; the same statement under another
//! identity merges into that fact; a near-identical statement about the
//! same subject and kind opens a dispute and never overwrites. [`decide`]
//! is pure over a [`DedupeSnapshot`] the stage reads from the store.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_identifiers::{BrainFactId, FactKindId};
use systemprompt_models::brain::{BrainFact, FactStatus, SubjectKey};

use super::similarity::trigram_similarity;
use crate::error::BrainResult;
use crate::models::FactFilter;
use crate::repository::{FactRepository, TombstoneRepository};

pub const DISPUTE_SIMILARITY: f64 = 0.85;
const NEIGHBOUR_LIMIT: i64 = 50;
const LIVE_STATUSES: [FactStatus; 4] = [
    FactStatus::Candidate,
    FactStatus::Proposed,
    FactStatus::Confirmed,
    FactStatus::Disputed,
];

/// A stored fact as dedupe compares it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Existing {
    pub id: BrainFactId,
    pub statement: String,
    pub statement_hash: String,
}

impl From<&BrainFact> for Existing {
    fn from(fact: &BrainFact) -> Self {
        Self {
            id: fact.id.clone(),
            statement: fact.statement.clone(),
            statement_hash: fact.statement_hash.clone(),
        }
    }
}

/// What the store holds for one candidate's identity, kind and subject.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DedupeSnapshot {
    pub tombstoned: bool,
    pub by_identity: Option<Existing>,
    pub neighbours: Vec<Existing>,
}

/// What persisting the candidate does.
#[derive(Debug, Clone, PartialEq)]
pub enum DedupeDecision {
    Insert,
    Revise(BrainFactId),
    Unchanged(BrainFactId),
    Merge(BrainFactId),
    Dispute {
        fact: BrainFactId,
        similarity: Option<f64>,
    },
    Tombstoned,
}

#[must_use]
pub fn decide(statement: &str, statement_hash: &str, snapshot: &DedupeSnapshot) -> DedupeDecision {
    if snapshot.tombstoned {
        return DedupeDecision::Tombstoned;
    }
    if let Some(existing) = &snapshot.by_identity {
        if existing.statement_hash == statement_hash {
            return DedupeDecision::Unchanged(existing.id.clone());
        }
        return DedupeDecision::Revise(existing.id.clone());
    }
    let others = snapshot.neighbours.iter();
    if let Some(same) = others.clone().find(|n| n.statement_hash == statement_hash) {
        return DedupeDecision::Merge(same.id.clone());
    }
    others
        .map(|n| (n, trigram_similarity(statement, &n.statement)))
        .filter(|(_, score)| *score > DISPUTE_SIMILARITY)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(DedupeDecision::Insert, |(n, score)| {
            DedupeDecision::Dispute {
                fact: n.id.clone(),
                similarity: Some(score),
            }
        })
}

#[must_use]
pub fn forced_dispute(snapshot: &DedupeSnapshot) -> Option<DedupeDecision> {
    if snapshot.tombstoned {
        return Some(DedupeDecision::Tombstoned);
    }
    snapshot
        .by_identity
        .as_ref()
        .map(|existing| DedupeDecision::Dispute {
            fact: existing.id.clone(),
            similarity: None,
        })
}

pub async fn snapshot(
    conn: &mut PgConnection,
    kind: &FactKindId,
    subject: &SubjectKey,
    identity_hash: &str,
) -> BrainResult<DedupeSnapshot> {
    if TombstoneRepository::exists(&mut *conn, kind, identity_hash).await? {
        return Ok(DedupeSnapshot {
            tombstoned: true,
            ..DedupeSnapshot::default()
        });
    }
    let by_identity = FactRepository::by_identity(&mut *conn, kind, identity_hash).await?;
    let filter = FactFilter::new()
        .with_kind(kind.clone())
        .with_subject(subject.clone())
        .with_statuses(LIVE_STATUSES.to_vec())
        .with_limit(NEIGHBOUR_LIMIT);
    let neighbours = FactRepository::by_subject_kind(conn, &filter)
        .await?
        .iter()
        .filter(|fact| fact.identity_hash != identity_hash)
        .map(Existing::from)
        .collect();
    Ok(DedupeSnapshot {
        tombstoned: false,
        by_identity: by_identity.as_ref().map(Existing::from),
        neighbours,
    })
}
