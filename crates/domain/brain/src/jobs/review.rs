//! [`BrainReviewJob`]: every thirty minutes, facts whose `review_after` has
//! passed and whose review was not yet requested are marked review-due
//! (one revision each, which sets `review_notified_at`) and a
//! `brain.fact.review_due` notice goes to whoever confirmed the fact, else
//! its owner, else the `brain-review` stream, on the same transaction.
//! Recall already withholds a fact past its review date.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use chrono::Utc;
use systemprompt_database::DbPool;
use systemprompt_extension::notifications::DynNotificationSink;
use systemprompt_identifiers::NotificationStreamId;
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_models::brain::{BrainFact, FactSource};
use systemprompt_models::notifications::{
    Attention, NotificationEvent, NotificationKind, NotificationKindError, SubjectRef,
};
use systemprompt_provider_contracts::{JobScope, submit_job};
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};

use crate::error::{BrainError, BrainResult};
use crate::models::FactChange;
use crate::repository::{BrainRepositories, FactRepository};

pub const FACT_REVIEW_DUE: &str = "brain.fact.review_due";
pub const BRAIN_REVIEW_STREAM: &str = "brain-review";
const FACT_SUBJECT: &str = "brain_fact";
const BATCH: i64 = 500;

/// Requests review of facts past their review date.
#[derive(Debug, Clone, Copy, Default)]
pub struct BrainReviewJob;

pub fn review_due_event(
    fact: &BrainFact,
) -> Result<Option<NotificationEvent>, NotificationKindError> {
    let recipient = fact.confirmed_by.as_ref().or(fact.owner_user_id.as_ref());
    let source_person = match &fact.source {
        FactSource::EnteredBy { user_id } => Some(user_id),
        FactSource::Session { owner, .. } => Some(owner),
        _ => None,
    };
    let Some(actor) = recipient.or(source_person) else {
        return Ok(None);
    };
    // JSON: the notification event's payload is its free-form contract field.
    let payload = serde_json::json!({
        "fact_id": fact.id,
        "kind": fact.kind,
        "review_after": fact.review_after,
    });
    let event = NotificationEvent::new(
        NotificationKind::new(FACT_REVIEW_DUE)?,
        actor.clone(),
        Utc::now(),
    )
    .with_subject(SubjectRef::new(FACT_SUBJECT, fact.id.as_str()))
    .with_excerpt(fact.statement.clone())
    .with_payload(payload);
    Ok(Some(match recipient {
        Some(user) => event.with_direct_recipient(user.clone(), Some(Attention::Agent)),
        None => event.with_stream(NotificationStreamId::new(BRAIN_REVIEW_STREAM)),
    }))
}

async fn mark_one(
    repos: &BrainRepositories,
    sink: Option<&DynNotificationSink>,
    fact: &BrainFact,
) -> BrainResult<()> {
    let mut tx = repos.write_pool().begin().await?;
    FactRepository::apply(&mut tx, &fact.id, FactChange::ReviewDue).await?;
    match (
        sink.filter(|sink| sink.is_available()),
        review_due_event(fact),
    ) {
        (Some(sink), Ok(Some(event))) => sink
            .emit(&mut tx, &event)
            .await
            .map_err(|e| BrainError::Notify(Box::new(e)))?,
        (_, Err(e)) => return Err(BrainError::Notify(Box::new(e))),
        _ => tracing::debug!(fact_id = %fact.id, "Brain review notice not sent"),
    }
    tx.commit().await?;
    Ok(())
}

#[async_trait]
impl Job for BrainReviewJob {
    fn name(&self) -> &'static str {
        "brain_review"
    }

    fn description(&self) -> &'static str {
        "Marks brain facts past their review date and asks their confirmer or owner to review them"
    }

    fn schedule(&self) -> &'static str {
        "0 */30 * * * *"
    }

    fn scope(&self) -> JobScope {
        JobScope::Cluster
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let started = Instant::now();
        if !ctx.get::<Arc<BrainProfile>>()?.enabled {
            return Ok(JobResult::success().with_stats(0, 0));
        }
        let repos = BrainRepositories::new(ctx.get::<DbPool>()?);
        let sink = ctx.get::<DynNotificationSink>().ok();
        let due = repos
            .facts
            .list_review_due(Utc::now(), BATCH)
            .await
            .map_err(|e| systemprompt_traits::ProviderError::Internal(Box::new(e)))?;
        let (mut marked, mut failed) = (0_u64, 0_u64);
        for fact in &due {
            match mark_one(&repos, sink, fact).await {
                Ok(()) => marked += 1,
                Err(error) => {
                    failed += 1;
                    tracing::warn!(fact_id = %fact.id, error = %error, "Brain review mark failed");
                },
            }
        }
        if marked > 0 {
            tracing::info!(marked, "Brain facts marked review-due");
        }
        let duration = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let result = if failed == 0 {
            JobResult::success()
        } else {
            JobResult::failure(format!(
                "{failed} brain facts could not be marked review-due"
            ))
        };
        Ok(result.with_stats(marked, failed).with_duration(duration))
    }
}

submit_job!(&BrainReviewJob);
