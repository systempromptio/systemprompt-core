//! Disputes a person raises against a fact, and their resolution. Raising
//! one marks the fact disputed (withheld from recall) and tells whoever
//! confirmed or owns it. Resolving the last open dispute either restores
//! the fact to confirmed (dismissed) or retires it (upheld); an upheld
//! dispute retires the fact at once.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use systemprompt_identifiers::{BrainDisputeId, BrainFactId};
use systemprompt_models::brain::{BrainFact, FactCandidate, FactStatus};
use systemprompt_models::notifications::{
    Attention, NotificationEvent, NotificationKind, SubjectRef,
};

use super::{BrainCaller, BrainService, authority};
use crate::error::{BrainError, BrainResult};
use crate::models::{FactChange, NewDispute};
use crate::repository::{DisputeRepository, FactRepository};

pub const FACT_SUBJECT: &str = "brain_fact";
pub const FACT_DISPUTED: &str = "brain.fact.disputed";
const PERSON_EXTRACTOR: &str = "person";

/// Whether a dispute was right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisputeOutcome {
    Upheld,
    Dismissed,
}

impl DisputeOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Upheld => "upheld",
            Self::Dismissed => "dismissed",
        }
    }
}

/// The decision on one open dispute of one fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisputeResolution {
    pub fact_id: BrainFactId,
    pub dispute_id: BrainDisputeId,
    pub outcome: DisputeOutcome,
    pub note: Option<String>,
}

fn as_candidate(fact: &BrainFact) -> FactCandidate {
    FactCandidate {
        kind: fact.kind.clone(),
        subject: fact.subject.clone(),
        fields: fact.fields.clone(),
        source: fact.source.clone(),
        topics: fact.topics.clone(),
        scope: Some(fact.scope.clone()),
        extractor: PERSON_EXTRACTOR.to_owned(),
        confidence: None,
    }
}

fn disputed_event(
    fact: &BrainFact,
    caller: &BrainCaller,
    reason: &str,
) -> BrainResult<Option<NotificationEvent>> {
    let Some(recipient) = fact.confirmed_by.as_ref().or(fact.owner_user_id.as_ref()) else {
        return Ok(None);
    };
    if recipient == &caller.user_id {
        return Ok(None);
    }
    let kind = NotificationKind::new(FACT_DISPUTED).map_err(|e| BrainError::Notify(Box::new(e)))?;
    // JSON: the notification event's payload is its free-form contract field.
    let payload = serde_json::json!({ "fact_id": fact.id, "kind": fact.kind, "reason": reason });
    Ok(Some(
        NotificationEvent::new(kind, caller.user_id.clone(), Utc::now())
            .with_subject(SubjectRef::new(FACT_SUBJECT, fact.id.as_str()))
            .with_direct_recipient(recipient.clone(), Some(Attention::Agent))
            .with_excerpt(fact.statement.clone())
            .with_payload(payload),
    ))
}

impl BrainService {
    pub async fn dispute(
        &self,
        caller: &BrainCaller,
        id: &BrainFactId,
        reason: &str,
    ) -> BrainResult<BrainDisputeId> {
        let mut tx = self.repos.write_pool().begin().await?;
        let fact = Self::fact_on(&mut tx, id).await?;
        authority::may_view(caller, &fact.scope)?;
        let dispute = NewDispute {
            fact_id: id.clone(),
            candidate: as_candidate(&fact),
            similarity: None,
            reason: reason.to_owned(),
        };
        let dispute_id = DisputeRepository::open(&mut tx, &dispute).await?;
        FactRepository::apply(
            &mut tx,
            id,
            FactChange::Disputed {
                actor: Some(caller.user_id.clone()),
            },
        )
        .await?;
        if let Some(event) = disputed_event(&fact, caller, reason)? {
            self.notify(&mut tx, &event).await?;
        }
        tx.commit().await?;
        tracing::info!(fact_id = %id, dispute_id = %dispute_id, actor = %caller.user_id, "Brain fact disputed");
        Ok(dispute_id)
    }

    pub async fn resolve_dispute(
        &self,
        caller: &BrainCaller,
        resolution: &DisputeResolution,
    ) -> BrainResult<()> {
        let declared = self.declared.load().await?;
        let id = &resolution.fact_id;
        let mut tx = self.repos.write_pool().begin().await?;
        let fact = Self::fact_on(&mut tx, id).await?;
        authority::may_govern(caller, &fact.scope, &fact.kind, &declared.workflows)?;
        let note = resolution.note.as_deref().unwrap_or_default();
        let text = format!("{}: {note}", resolution.outcome.as_str());
        let open = DisputeRepository::open_for_fact(&mut *tx, id).await?;
        if !open.contains(&resolution.dispute_id)
            || !DisputeRepository::resolve(&mut tx, &resolution.dispute_id, &caller.user_id, &text)
                .await?
        {
            return Err(BrainError::Forbidden {
                reason: "the dispute is not open on this fact".to_owned(),
            });
        }
        let still_open: Vec<_> = open
            .into_iter()
            .filter(|dispute| dispute != &resolution.dispute_id)
            .collect();
        let status = match resolution.outcome {
            DisputeOutcome::Upheld => Some(FactStatus::Retired),
            DisputeOutcome::Dismissed if still_open.is_empty() => Some(FactStatus::Confirmed),
            DisputeOutcome::Dismissed => None,
        };
        if let Some(status) = status {
            let change = FactChange::DisputeResolved {
                status,
                actor: caller.user_id.clone(),
            };
            FactRepository::apply(&mut tx, id, change).await?;
        }
        tx.commit().await?;
        tracing::info!(
            fact_id = %id,
            dispute_id = %resolution.dispute_id,
            outcome = resolution.outcome.as_str(),
            actor = %caller.user_id,
            "Brain dispute resolved"
        );
        Ok(())
    }
}
