//! Person-entered facts: the kind must be declared and the caller must hold
//! the authority that governs the fact's scope; the fact itself is built by the
//! consumer from the `brain.fact.entered` event this emits on its own
//! transaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use systemprompt_extension::brain::BrainSinkError;
use systemprompt_identifiers::EventOutboxId;
use systemprompt_models::brain::EnteredFact;

use super::{BrainCaller, BrainService, authority};
use crate::error::{BrainError, BrainResult};

impl BrainService {
    pub async fn enter(
        &self,
        caller: &BrainCaller,
        fact: &EnteredFact,
    ) -> BrainResult<EventOutboxId> {
        let who = &caller.user_id;
        let declared = self.declared.load().await?;
        if declared.kind(&fact.kind).is_none() {
            return Err(BrainError::UnknownKind(fact.kind.clone()));
        }
        authority::may_enter(caller, &fact.scope, &fact.kind, &declared.workflows)?;
        if !self.sink.is_available() {
            return Err(BrainError::Sink(BrainSinkError::Unavailable));
        }
        let event = fact.to_event(who.clone(), Utc::now())?;
        let mut tx = self.repos.write_pool().begin().await?;
        let outbox_id = self.sink.emit(&mut tx, &event).await?;
        tx.commit().await?;
        tracing::info!(
            actor = %who,
            kind = %fact.kind,
            outbox_id = %outbox_id,
            "Brain fact entered"
        );
        Ok(outbox_id)
    }
}
