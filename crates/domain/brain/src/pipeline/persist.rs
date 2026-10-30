//! The persist stage: one decided candidate becomes a write through the
//! fact repository. A new fact is inserted with revision 1; a revision,
//! merge or dispute is a [`FactChange`] applied to the existing fact, so
//! every change has its revision row; a dispute also opens a dispute row
//! and never overwrites the fact.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_identifiers::{BrainFactId, PipelineVersion, TopicId, UserId};
use systemprompt_models::brain::FactCandidate;

use super::dedupe::DedupeDecision;
use super::route::Routed;
use super::status::Lifecycle;
use crate::error::BrainResult;
use crate::models::{FactChange, NewDispute, NewFact};
use crate::repository::{DisputeRepository, FactRepository, SubjectRepository};

/// Everything one write needs.
#[derive(Debug, Clone)]
pub struct PersistPlan<'a> {
    pub candidate: &'a FactCandidate,
    pub statement: &'a str,
    pub identity_hash: &'a str,
    pub routed: &'a Routed,
    pub lifecycle: Lifecycle,
    pub actor: &'a UserId,
    pub version: &'a PipelineVersion,
    pub dispute_reason: Option<&'a str>,
}

/// What the write did to the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    Created,
    Revised,
    Unchanged,
    Merged,
    Disputed,
    Tombstoned,
}

/// The fact a write landed on and what it did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Persisted {
    pub fact: Option<BrainFactId>,
    pub effect: Effect,
}

pub async fn persist(
    conn: &mut PgConnection,
    plan: &PersistPlan<'_>,
    decision: DedupeDecision,
) -> BrainResult<Persisted> {
    let landed = |fact: BrainFactId, effect| Persisted {
        fact: Some(fact),
        effect,
    };
    match decision {
        DedupeDecision::Tombstoned => Ok(Persisted {
            fact: None,
            effect: Effect::Tombstoned,
        }),
        DedupeDecision::Unchanged(id) => Ok(landed(id, Effect::Unchanged)),
        DedupeDecision::Insert => insert(conn, plan).await,
        DedupeDecision::Revise(id) => {
            FactRepository::apply(conn, &id, revised(plan)).await?;
            Ok(landed(id, Effect::Revised))
        },
        DedupeDecision::Merge(id) => {
            let topics = merged_topics(conn, &id, &plan.candidate.topics).await?;
            let change = FactChange::Merged {
                topics,
                actor: Some(plan.actor.clone()),
            };
            FactRepository::apply(conn, &id, change).await?;
            Ok(landed(id, Effect::Merged))
        },
        DedupeDecision::Dispute { fact, similarity } => {
            let dispute = NewDispute {
                fact_id: fact.clone(),
                candidate: plan.candidate.clone(),
                similarity,
                reason: plan
                    .dispute_reason
                    .unwrap_or("a near-identical statement conflicts")
                    .to_owned(),
            };
            DisputeRepository::open(&mut *conn, &dispute).await?;
            let change = FactChange::Disputed {
                actor: Some(plan.actor.clone()),
            };
            FactRepository::apply(conn, &fact, change).await?;
            Ok(landed(fact, Effect::Disputed))
        },
    }
}

async fn insert(conn: &mut PgConnection, plan: &PersistPlan<'_>) -> BrainResult<Persisted> {
    SubjectRepository::ensure(&mut *conn, &plan.candidate.subject, None).await?;
    let mut builder = NewFact::builder(
        plan.candidate.clone(),
        plan.routed.scope.clone(),
        plan.statement,
        plan.identity_hash,
    )
    .status(plan.lifecycle.status)
    .importance_base(plan.lifecycle.importance_base)
    .review_after(plan.lifecycle.review_after)
    .pipeline_version(plan.version.clone());
    if let Some(owner) = &plan.routed.owner {
        builder = builder.owner(owner.clone());
    }
    let inserted = FactRepository::insert_candidate(conn, &builder.build()).await?;
    let effect = if inserted.created {
        Effect::Created
    } else {
        Effect::Unchanged
    };
    Ok(Persisted {
        fact: Some(inserted.id),
        effect,
    })
}

fn revised(plan: &PersistPlan<'_>) -> FactChange {
    FactChange::Revised {
        statement: plan.statement.to_owned(),
        fields: plan.candidate.fields.clone(),
        source: plan.candidate.source.clone(),
        topics: plan.candidate.topics.clone(),
        status: plan.lifecycle.status,
        importance_base: plan.lifecycle.importance_base,
        review_after: Some(plan.lifecycle.review_after),
        actor: Some(plan.actor.clone()),
    }
}

async fn merged_topics(
    conn: &mut PgConnection,
    id: &BrainFactId,
    added: &[TopicId],
) -> BrainResult<Vec<TopicId>> {
    let mut topics = FactRepository::find_on(conn, id)
        .await?
        .map(|fact| fact.topics)
        .unwrap_or_default();
    for topic in added {
        if !topics.contains(topic) {
            topics.push(topic.clone());
        }
    }
    Ok(topics)
}
