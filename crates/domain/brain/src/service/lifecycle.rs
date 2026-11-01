//! Lifecycle actions on one fact: confirm, retract (retired, kept, never
//! recalled), erase (deleted with a tombstone so it is never extracted
//! again), pin, unpin and field edits. A field edit re-renders the
//! statement from the kind's template and is refused when it would change
//! the fact's identity.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{Duration, Utc};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::{FactRevision, identity_hash, render_statement};

use super::{BrainCaller, BrainService, authority};
use crate::error::{BrainError, BrainResult};
use crate::models::FactChange;
use crate::repository::FactRepository;

impl BrainService {
    pub async fn confirm(
        &self,
        caller: &BrainCaller,
        id: &BrainFactId,
    ) -> BrainResult<FactRevision> {
        let by = caller.user_id.clone();
        let default_days = self.profile.lifecycle.review_after_days;
        self.govern(caller, id, |fact, declared| {
            let days = declared
                .kind(&fact.kind)
                .and_then(|kind| kind.review_after_days)
                .unwrap_or(default_days);
            let review_after = Utc::now() + Duration::days(i64::from(days));
            Ok(FactChange::Confirmed {
                by,
                review_after: Some(review_after),
            })
        })
        .await
    }

    pub async fn retract(
        &self,
        caller: &BrainCaller,
        id: &BrainFactId,
        reason: &str,
    ) -> BrainResult<FactRevision> {
        let actor = caller.user_id.clone();
        let revision = self
            .govern(caller, id, |_, _| Ok(FactChange::Retracted { actor }))
            .await?;
        tracing::info!(fact_id = %id, reason, "Brain fact retracted");
        Ok(revision)
    }

    pub async fn pin(&self, caller: &BrainCaller, id: &BrainFactId) -> BrainResult<FactRevision> {
        let actor = caller.user_id.clone();
        self.govern(caller, id, |_, _| Ok(FactChange::Pinned { actor }))
            .await
    }

    pub async fn unpin(&self, caller: &BrainCaller, id: &BrainFactId) -> BrainResult<FactRevision> {
        let actor = caller.user_id.clone();
        self.govern(caller, id, |_, _| Ok(FactChange::Unpinned { actor }))
            .await
    }

    pub async fn edit_fields(
        &self,
        caller: &BrainCaller,
        id: &BrainFactId,
        // JSON: edited fields are checked against the kind's template and identity.
        fields: serde_json::Map<String, serde_json::Value>,
    ) -> BrainResult<FactRevision> {
        let actor = caller.user_id.clone();
        self.govern(caller, id, |fact, declared| {
            let kind = declared
                .kind(&fact.kind)
                .ok_or_else(|| BrainError::UnknownKind(fact.kind.clone()))?;
            if identity_hash(&fact.kind, &fact.subject, &kind.identity, &fields)
                != fact.identity_hash
            {
                return Err(BrainError::Forbidden {
                    reason: "an edit may not change the fact's identity fields".to_owned(),
                });
            }
            let statement = render_statement(&kind.statement, &fact.subject, &fields)?;
            Ok(FactChange::FieldsEdited {
                statement,
                fields,
                actor,
            })
        })
        .await
    }

    pub async fn erase(&self, caller: &BrainCaller, id: &BrainFactId) -> BrainResult<()> {
        let declared = self.declared.load().await?;
        let mut tx = self.repos.write_pool().begin().await?;
        let fact = Self::fact_on(&mut tx, id).await?;
        authority::may_govern(caller, &fact.scope, &fact.kind, &declared.workflows)?;
        FactRepository::erase(&mut tx, id, Some(&caller.user_id)).await?;
        tx.commit().await?;
        tracing::info!(fact_id = %id, actor = %caller.user_id, "Brain fact erased");
        Ok(())
    }
}
