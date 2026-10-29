//! [`NotificationPipeline::process`]: one fact, on its own transaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use chrono::Utc;
use sqlx::PgConnection;
use systemprompt_identifiers::{EventOutboxId, UserId};
use systemprompt_models::notifications::{
    Attention, AttentionReason, DeliveryState, DigestWindow, NotificationEvent,
};

use super::context::BatchContext;
use super::plan::{self, Decision};
use super::{
    ActionPromptBuilder, NotificationPipeline, ProcessOutcome, StepFailure, coalesce, metrics,
    visibility,
};
use crate::error::NotificationsResult;
use crate::models::{NewNotifications, NewRecipient, PersistedRow};
use crate::repository::{NotificationFailureRepository, NotificationRepository};

type FailureRow = (Option<UserId>, String, String);

struct Candidate {
    user: UserId,
    attention: Attention,
    reason: AttentionReason,
    coalesce_window_secs: Option<u32>,
    digest: Option<DigestWindow>,
}

impl NotificationPipeline {
    #[tracing::instrument(skip_all, fields(outbox_id = %outbox_id, kind = %event.kind))]
    pub async fn process(
        &self,
        outbox_id: &EventOutboxId,
        event: &NotificationEvent,
        conn: &mut PgConnection,
        ctx: &mut BatchContext,
    ) -> NotificationsResult<ProcessOutcome> {
        let members = self.members();
        let plan = plan::plan(&self.parts.routers, &members, &mut *conn, ctx, event).await?;
        let mut failures: Vec<FailureRow> = plan.failures.iter().map(StepFailure::row).collect();
        let users: Vec<UserId> = plan
            .routed
            .recipients
            .iter()
            .map(|d| d.user.clone())
            .collect();
        let recorded =
            NotificationRepository::recorded_recipients(&mut *conn, outbox_id, &users).await?;
        let mut outcome = ProcessOutcome::default();
        let candidates = Self::candidates(event, &plan, &recorded, &mut outcome);
        let (recipients, rejected) = self
            .visible_recipients(outbox_id, event, &plan, candidates)
            .await;
        outcome.withheld += u32::try_from(rejected.len()).unwrap_or(u32::MAX);
        failures.extend(rejected);
        let batch = NewNotifications {
            kind: event.kind.clone(),
            stream: plan.routed.stream.clone(),
            actor: event.actor.clone(),
            actor_is_agent: event.actor_is_agent,
            via_agent: event.via_agent.clone(),
            subjects: event.subjects.clone(),
            action: ActionPromptBuilder::new(event).build(),
            event_at: event.occurred_at,
            outbox_id: outbox_id.clone(),
            recipients,
        };
        let persisted = NotificationRepository::persist(&mut *conn, &batch).await?;
        NotificationFailureRepository::record_many(&mut *conn, Some(outbox_id), &failures).await?;
        Self::tally(&mut outcome, persisted);
        Ok(outcome)
    }
    fn candidates(
        event: &NotificationEvent,
        plan: &plan::Plan,
        recorded: &HashSet<UserId>,
        outcome: &mut ProcessOutcome,
    ) -> Vec<Candidate> {
        let now = Utc::now();
        let mut candidates = Vec::new();
        for decision in plan
            .routed
            .recipients
            .iter()
            .filter(|d| !recorded.contains(&d.user))
        {
            match plan::decide(event, plan, decision, now) {
                Decision::Muted { .. } => outcome.withheld += 1,
                Decision::Deliver {
                    attention,
                    reason,
                    coalesce_window_secs,
                    digest,
                    ..
                } => candidates.push(Candidate {
                    user: decision.user.clone(),
                    attention,
                    reason,
                    coalesce_window_secs,
                    digest,
                }),
            }
        }
        metrics::recipients("muted", outcome.withheld as usize);
        candidates
    }

    async fn visible_recipients(
        &self,
        outbox_id: &EventOutboxId,
        event: &NotificationEvent,
        plan: &plan::Plan,
        candidates: Vec<Candidate>,
    ) -> (Vec<NewRecipient>, Vec<FailureRow>) {
        let readers: Vec<(UserId, bool)> = candidates
            .iter()
            .map(|c| (c.user.clone(), plan.in_audience.contains(&c.user)))
            .collect();
        let verdicts = visibility::check_many(
            &self.parts.visibilities,
            &event.subjects,
            &readers,
            self.members().timeout,
        )
        .await;
        let mut recipients = Vec::with_capacity(candidates.len());
        let mut failures = Vec::new();
        for (candidate, verdict) in candidates.into_iter().zip(verdicts) {
            match verdict {
                Ok(excerpt_allowed) => recipients.push(NewRecipient {
                    excerpt: event.excerpt.clone().filter(|_| excerpt_allowed),
                    coalesce_key: coalesce::coalesce_key(
                        event,
                        candidate.coalesce_window_secs,
                        outbox_id,
                    ),
                    held_until: coalesce::digest_release(event, candidate.digest),
                    user: candidate.user,
                    attention: candidate.attention,
                    reason: candidate.reason,
                }),
                Err(withheld) => {
                    metrics::recipients(withheld.decision(), 1);
                    failures.push(
                        StepFailure::withheld("visibility", Some(candidate.user), &withheld).row(),
                    );
                },
            }
        }
        (recipients, failures)
    }

    fn tally(outcome: &mut ProcessOutcome, persisted: Vec<PersistedRow>) {
        for row in persisted {
            outcome.coalesced += u32::from(!row.inserted);
            match row.delivery_state {
                DeliveryState::Held => outcome.held += 1,
                DeliveryState::Pending => outcome.pending.push(row.id.clone()),
                DeliveryState::Delivered => {},
            }
            outcome.persisted.push(row.id);
        }
        let inserted = outcome.persisted.len() - outcome.coalesced as usize;
        metrics::recipients("inserted", inserted);
        metrics::recipients("coalesced", outcome.coalesced as usize);
        metrics::recipients("held", outcome.held as usize);
    }
}
