//! [`BrainRecallService`]: the brain's [`MemoryRecall`].
//!
//! A same-turn request reuses its turn's stored block or skips. A new turn
//! runs the four tiers concurrently on the store, fuses them by reciprocal
//! rank, lets extension retrievers re-rank, drops facts the lifecycle or
//! `visible()` withholds, and fills the token budget by in-context
//! importance (an extension injection policy first). No visible fact is a
//! `NoFacts` skip. The gateway enforces the deadline; every tier is
//! bounded by the retrieval document's limits.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use futures::future::join_all;
use systemprompt_extension::brain::{
    FactRecallSource, FactRetriever, ImportanceModel, InjectedFact, InjectionPolicy, MemoryRecall,
    RecallBlock, RecallError, RecallObservation, RecallOutcome, RecallReceipt, RecallRequest,
    ReuseHint, SkipReason, SourceAccess, WithheldSummary,
};
use systemprompt_extension::notifications::NotificationAudienceResolver;
use systemprompt_manifest::profile::{BrainProfile, StalePolicy};
use systemprompt_models::brain::{
    BrainFact, FactStatus, InjectionPlan, RecallCtx, RecallQuery, RetrievalTier, ScopeLevel,
    ScoredFact,
};

use super::budget::{self, BudgetLimits, Candidate};
use super::declared::{DeclaredRecall, DeclaredSource, receipt_workflow};
use super::fusion::{self, TierHits};
use super::reuse::parse_stored;
use super::score::{self, ScoreInput};
use super::visible::{Visibility, lifecycle_withheld};
use crate::outcomes::OutcomeService;

pub const STALE_MARKER: &str = " [stale: its source changed since it was confirmed]";
const MIN_TIER_LIMIT: u32 = 8;
const MAX_TIER_LIMIT: u32 = 50;

/// Recalls team memory for gateway requests and records each recall.
#[derive(Clone)]
pub struct BrainRecallService {
    pub(super) store: Arc<dyn FactRecallSource>,
    pub(super) source_access: Arc<dyn SourceAccess>,
    pub(super) declared: Arc<dyn DeclaredSource>,
    pub(super) profile: Arc<BrainProfile>,
    pub(super) retrievers: Vec<Arc<dyn FactRetriever>>,
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
    fn inject_stale(&self) -> bool {
        matches!(self.profile.recall.stale, StalePolicy::Marker)
    }

    fn context(&self, request: &RecallRequest, declared: &DeclaredRecall) -> RecallCtx {
        let principal = &request.principal;
        let scopes: Vec<ScopeLevel> = ScopeLevel::ALL
            .into_iter()
            .filter(|level| self.profile.recall.scopes.contains(level))
            .collect();
        let mut ctx = RecallCtx::new(
            principal.user_id.clone(),
            principal.actor.clone(),
            Utc::now(),
        )
        .with_client(principal.client)
        .with_scopes(scopes);
        if let Some(project) = &principal.project {
            ctx = ctx.with_project(project.clone());
            if let Some(workflow) = declared.workflow_named(project) {
                ctx = ctx.with_workflow(workflow.id.clone());
            }
        }
        ctx
    }

    async fn retrieve(
        &self,
        request: &RecallRequest,
        ctx: &RecallCtx,
        declared: &DeclaredRecall,
    ) -> Result<Vec<ScoredFact>, RecallError> {
        let retrieval = &declared.retrieval;
        let query = RecallQuery {
            subject_hints: request.subject_hints.clone(),
            limit: retrieval
                .max_facts
                .saturating_mul(2)
                .clamp(MIN_TIER_LIMIT, MAX_TIER_LIMIT),
        };
        let (pinned, subject, context, text) = tokio::join!(
            self.store
                .pinned(ctx, retrieval.max_facts.min(MAX_TIER_LIMIT)),
            self.store.by_subject(&query, ctx),
            self.store.by_context(&query, ctx),
            self.store.full_text(&query, ctx),
        );
        let tiers = vec![
            TierHits::new(RetrievalTier::Pinned, pinned?),
            TierHits::new(RetrievalTier::Subject, subject?),
            TierHits::new(RetrievalTier::Context, context?),
            TierHits::new(RetrievalTier::FullText, text?),
        ];
        let mut fused = fusion::fuse(tiers, &retrieval.tiers, retrieval.rrf_k);
        let extension = join_all(self.retrievers.iter().map(|r| r.retrieve(&query, ctx))).await;
        for (retriever, result) in self.retrievers.iter().zip(extension) {
            match result {
                Ok(ids) => fusion::boost(&mut fused, &ids, &retrieval.tiers, retrieval.rrf_k),
                Err(error) => tracing::warn!(
                    retriever = retriever.name(),
                    error = %error,
                    "Brain retriever failed; its ranking is skipped"
                ),
            }
        }
        Ok(fused)
    }

    fn plan(
        &self,
        visible: Vec<ScoredFact>,
        ctx: &RecallCtx,
        declared: &DeclaredRecall,
        budget_tokens: u32,
    ) -> InjectionPlan {
        let workflow = ctx.workflow.as_ref().and_then(|id| declared.workflow(id));
        let limits = BudgetLimits::resolve(&declared.retrieval, workflow, budget_tokens);
        if let Some(plan) = self
            .injection
            .iter()
            .find_map(|policy| policy.plan(&visible, ctx, limits.budget_tokens))
        {
            return plan;
        }
        let candidates = visible
            .into_iter()
            .map(|scored| Candidate {
                importance: self.importance(&scored, ctx, declared),
                text: fact_text(&scored.fact),
                scored,
            })
            .collect();
        budget::fill(candidates, &limits)
    }

    fn importance(&self, scored: &ScoredFact, ctx: &RecallCtx, declared: &DeclaredRecall) -> f64 {
        if let Some(score) = self
            .importance
            .iter()
            .find_map(|model| model.in_context(&scored.fact, ctx, &declared.importance))
        {
            return score;
        }
        let input = ScoreInput {
            config: &declared.importance,
            ctx,
            subject_matched: fusion::found_by(scored, RetrievalTier::Subject),
        };
        score::in_context(&scored.fact, &input).total
    }

    async fn new_turn(&self, request: &RecallRequest) -> Result<RecallOutcome, RecallError> {
        let declared = self.declared.load().await.map_err(|e| RecallError::Other {
            component: "declarations",
            source: Box::new(e),
        })?;
        let ctx = self.context(request, &declared);
        let mut withheld = WithheldSummary::default();
        let mut ranked: Vec<ScoredFact> = Vec::new();
        for scored in self.retrieve(request, &ctx, &declared).await? {
            match lifecycle_withheld(&scored.fact, ctx.now, self.inject_stale()) {
                Some(reason) => withheld.add(reason),
                None => ranked.push(scored),
            }
        }
        let visibility = Visibility::new(Arc::clone(&self.source_access), self.source_timeout)
            .with_audiences(self.audiences.clone())
            .with_topics(declared.topics.clone());
        let (visible, hidden) = visibility.filter(ranked, &request.principal).await;
        for (reason, count) in hidden.by_reason {
            *withheld.by_reason.entry(reason).or_default() += count;
        }
        let budget = match request.budget_tokens {
            0 => self.profile.recall.budget_tokens,
            requested => requested,
        };
        let plan = self.plan(visible, &ctx, &declared, budget);
        for fact in &plan.withheld {
            withheld.add(fact.reason);
        }
        if plan.facts.is_empty() {
            return Ok(RecallOutcome::Skipped {
                reason: SkipReason::NoFacts,
            });
        }
        let scope = ctx
            .project
            .clone()
            .unwrap_or_else(|| ScopeLevel::Org.as_str().to_owned());
        let block = plan
            .facts
            .iter()
            .fold(RecallBlock::new(scope, ctx.now), |block, scored| {
                block.with_fact(InjectedFact {
                    id: scored.fact.id.clone(),
                    revision: scored.fact.revision,
                    source: scored.fact.source.label(),
                    text: fact_text(&scored.fact),
                })
            })
            .with_withheld(withheld)
            .with_tokens(plan.tokens);
        Ok(RecallOutcome::Injected(block))
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
