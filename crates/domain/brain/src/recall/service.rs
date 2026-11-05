//! [`BrainRecallService`]: the brain's [`MemoryRecall`].
//!
//! A same-turn request reuses its turn's stored block or skips. A new turn
//! builds the principal's held keys once, runs the core tiers, the
//! lifecycle count and the extension retrievers concurrently over the
//! working-set candidates, loads every fact a tier returned once, fuses
//! every tier by reciprocal rank, drops facts the lifecycle or `visible()`
//! withholds, lets the first registered reranker reorder what is left
//! (see [`super::rerank`]), and fills the token budget by in-context
//! importance (an extension injection policy first); see
//! [`super::new_turn`]. No visible fact is a `NoFacts`
//! skip. The gateway enforces the deadline; every tier is bounded by the
//! retrieval document's limits.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_extension::brain::{
    FactRecallSource, FactRetriever, ImportanceModel, InjectionPolicy, MemoryRecall, RecallError,
    RecallObservation, RecallOutcome, RecallReceipt, RecallRequest, Reranker, ReuseHint,
    SkipReason, SourceAccess,
};
use systemprompt_extension::notifications::NotificationAudienceResolver;
use systemprompt_manifest::profile::{BrainProfile, StalePolicy};
use systemprompt_models::brain::{BrainFact, FactStatus};

use super::declared::{DeclaredSource, receipt_workflow};
use super::introduce::FactLoader;
use super::reuse::parse_stored;
use crate::outcomes::OutcomeService;

pub const STALE_MARKER: &str = " [stale: its source changed since it was confirmed]";

/// Recalls team memory for gateway requests and records each recall.
#[derive(Clone)]
pub struct BrainRecallService {
    pub(super) store: Arc<dyn FactRecallSource>,
    pub(super) source_access: Arc<dyn SourceAccess>,
    pub(super) declared: Arc<dyn DeclaredSource>,
    pub(super) profile: Arc<BrainProfile>,
    pub(super) retrievers: Vec<Arc<dyn FactRetriever>>,
    pub(super) rerankers: Vec<Arc<dyn Reranker>>,
    pub(super) loader: Arc<dyn FactLoader>,
    pub(super) injection: Vec<Arc<dyn InjectionPolicy>>,
    pub(super) importance: Vec<Arc<dyn ImportanceModel>>,
    pub(super) audiences: Vec<Arc<dyn NotificationAudienceResolver>>,
    pub(super) source_timeout: Duration,
    pub(super) outcomes: Option<OutcomeService>,
}

#[must_use]
pub fn fact_text(fact: &BrainFact) -> String {
    if fact.status == FactStatus::Stale {
        format!("{}{STALE_MARKER}", fact.statement)
    } else {
        fact.statement.clone()
    }
}

impl BrainRecallService {
    pub(super) fn inject_stale(&self) -> bool {
        matches!(self.profile.recall.stale, StalePolicy::Marker)
    }

    async fn same_turn(&self, request: &RecallRequest) -> Result<RecallOutcome, RecallError> {
        let not_new = RecallOutcome::Skipped {
            reason: SkipReason::NotNewTurn,
        };
        let Some(conversation) = &request.conversation.gateway_conversation_id else {
            return Ok(not_new);
        };
        Ok(self
            .store
            .stored_block(conversation)
            .await?
            .as_ref()
            .and_then(parse_stored)
            .map_or(not_new, RecallOutcome::Injected))
    }
}

#[async_trait::async_trait]
impl MemoryRecall for BrainRecallService {
    fn name(&self) -> &'static str {
        "brain"
    }

    async fn recall(&self, request: &RecallRequest) -> Result<RecallOutcome, RecallError> {
        match request.reuse {
            ReuseHint::SameTurn => self.same_turn(request).await,
            ReuseHint::NewTurn => self.new_turn(request).await,
        }
    }

    async fn record(&self, receipt: &RecallReceipt) -> Result<(), RecallError> {
        let mut receipt = receipt.clone();
        receipt.workflow = receipt_workflow(self.declared.as_ref(), &receipt).await;
        self.store.record_recall(&receipt).await
    }

    async fn observe(&self, observation: &RecallObservation) -> Result<(), RecallError> {
        let Some(outcomes) = &self.outcomes else {
            return Ok(());
        };
        let written = outcomes
            .observe_response(&observation.ai_request_id, &observation.assistant_text)
            .await
            .map_err(|e| RecallError::Other {
                component: "outcomes",
                source: Box::new(e),
            })?;
        tracing::debug!(
            ai_request_id = %observation.ai_request_id,
            written,
            "Brain response outcomes recorded"
        );
        Ok(())
    }
}
