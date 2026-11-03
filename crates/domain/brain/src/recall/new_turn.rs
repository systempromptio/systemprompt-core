//! A new-turn recall: context (held keys built once, `now` read once,
//! `as_of` from the request), retrieval over the working-set candidates,
//! fusion, the lifecycle counts, visibility, and the budget fill.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::Utc;
use futures::future::join_all;
use systemprompt_extension::brain::{
    InjectedFact, RecallBlock, RecallError, RecallOutcome, RecallRequest, SkipReason,
    WithheldSummary,
};
use systemprompt_models::brain::{
    InjectionPlan, RecallCtx, RecallQuery, RetrievalTier, ScopeLevel, ScoredFact,
};

use super::budget::{self, BudgetLimits, Candidate};
use super::declared::DeclaredRecall;
use super::fusion::{self, TierHits};
use super::held::{self, HeldKeys};
use super::score::{self, ScoreInput};
use super::service::{BrainRecallService, fact_text};
use super::visible::{Visibility, lifecycle_withheld};

const MIN_TIER_LIMIT: u32 = 8;
const MAX_TIER_LIMIT: u32 = 50;

impl BrainRecallService {
    async fn context(
        &self,
        request: &RecallRequest,
        declared: &DeclaredRecall,
    ) -> (RecallCtx, HeldKeys) {
        let principal = &request.principal;
        let scopes: Vec<ScopeLevel> = ScopeLevel::ALL
            .into_iter()
            .filter(|level| self.profile.recall.scopes.contains(level))
            .collect();
        let held = held::build(
            &principal.user_id,
            principal.project.as_deref(),
            &self.audiences,
        )
        .await;
        let now = Utc::now();
        let mut ctx = RecallCtx::new(principal.user_id.clone(), principal.actor.clone(), now)
            .with_client(principal.client)
            .with_scopes(scopes)
            .with_as_of(request.as_of.unwrap_or(now))
            .with_held_keys(held.keys.clone());
        if let Some(project) = &principal.project {
            ctx = ctx.with_project(project.clone());
            if let Some(workflow) = declared.workflow_named(project) {
                ctx = ctx.with_workflow(workflow.id.clone());
            }
        }
        (ctx, held)
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
            links_weight: retrieval.links_weight,
            synonyms: declared.synonyms.groups.clone(),
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

    async fn visible(
        &self,
        request: &RecallRequest,
        ctx: &RecallCtx,
        held: HeldKeys,
        declared: &DeclaredRecall,
    ) -> Result<(Vec<ScoredFact>, WithheldSummary), RecallError> {
        let mut withheld = WithheldSummary::default();
        for (reason, count) in self.store.withheld_counts(ctx).await? {
            *withheld.by_reason.entry(reason).or_default() += count;
        }
        let mut ranked: Vec<ScoredFact> = Vec::new();
        for scored in self.retrieve(request, ctx, declared).await? {
            match lifecycle_withheld(&scored.fact, ctx.as_of, self.inject_stale()) {
                Some(reason) => withheld.add(reason),
                None => ranked.push(scored),
            }
        }
        let visibility = Visibility::new(Arc::clone(&self.source_access), self.source_timeout)
            .with_audiences(self.audiences.clone())
            .with_topics(declared.topics.clone())
            .with_held(held);
        let (visible, hidden) = visibility.filter(ranked, &request.principal).await;
        for (reason, count) in hidden.by_reason {
            *withheld.by_reason.entry(reason).or_default() += count;
        }
        Ok((visible, withheld))
    }

    pub(super) async fn new_turn(
        &self,
        request: &RecallRequest,
    ) -> Result<RecallOutcome, RecallError> {
        let declared = self.declared.load().await.map_err(|e| RecallError::Other {
            component: "declarations",
            source: Box::new(e),
        })?;
        let (ctx, held) = self.context(request, &declared).await;
        let (visible, mut withheld) = self.visible(request, &ctx, held, &declared).await?;
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
            .fold(RecallBlock::new(scope, ctx.as_of), |block, scored| {
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
}
