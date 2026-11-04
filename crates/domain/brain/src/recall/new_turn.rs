//! A new-turn recall: context (held keys built once, `now` read once,
//! `as_of` from the request), retrieval over the working-set candidates
//! as ids (the five core tiers, the lifecycle count and the extension
//! retrievers run concurrently), one load of every id a tier returned,
//! fusion (the facts only an extension found are loaded and admitted by
//! the tier SQL's own rule, see [`super::introduce`] and [`super::admit`]),
//! visibility, and the budget fill. A recall *names a subject* when the
//! subject or linked tier returned a fact; the fill's linked reserve and
//! context cap apply only then.
//!
//! An id the load does not return (the fact was erased between the two
//! statements) is dropped from every tier before fusion, so it takes no
//! rank. An introduced fact the admission rule rejects for a lifecycle
//! reason is not counted again on the receipt, since `withheld_counts`
//! already counted every reachable stale, disputed and review-due fact,
//! and one rejected for scope is counted as `scope`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::Utc;
use futures::future::join_all;
use systemprompt_extension::brain::{
    InjectedFact, RecallBlock, RecallError, RecallOutcome, RecallRequest, SkipReason, TierIds,
    WithheldSummary,
};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::{
    BrainFact, InjectionPlan, RecallCtx, RecallQuery, RetrievalTier, ScopeLevel, ScoredFact,
    WithheldReason,
};

use super::admit::{self, Admission};
use super::budget::{self, BudgetLimits, Candidate};
use super::declared::DeclaredRecall;
use super::held::{self, HeldKeys};
use super::score::{self, ScoreInput};
use super::service::{BrainRecallService, fact_text};
use super::visible::{Visibility, lifecycle_withheld};
use super::{fusion, introduce};

#[derive(Debug, Clone, Copy)]
struct PlanShape {
    budget_tokens: u32,
    subject_named: bool,
}

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
        withheld: &mut WithheldSummary,
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
            search_max_df: retrieval.search_max_df,
            search_quorum: retrieval.search_quorum,
            search_retry_quorum: retrieval.search_retry_quorum,
            search_quorum_over: retrieval.search_quorum_over,
            alias_min_sim: retrieval.alias_min_sim,
            max_subjects: retrieval.max_subjects,
        };
        let fan_out = join_all(self.retrievers.iter().map(|r| r.retrieve(&query, ctx)));
        let (core, counts, extension) = tokio::join!(
            self.store
                .tier_ids(&query, ctx, retrieval.max_facts.min(MAX_TIER_LIMIT)),
            self.store.withheld_counts(ctx),
            fan_out,
        );
        for (reason, count) in counts? {
            *withheld.by_reason.entry(reason).or_default() += count;
        }
        let mut tiers = core?;
        let mut facts = self.load_tiers(&mut tiers).await?;
        let ranked = introduce::ranked_of(&self.retrievers, extension);
        if retrieval.tiers.extension > 0.0 && !ranked.is_empty() {
            let mut known: HashSet<BrainFactId> = facts.keys().cloned().collect();
            let unseen = introduce::unseen(&known, &ranked, retrieval.extension_max_candidates);
            if !unseen.is_empty() {
                let admission = Admission::of(ctx, self.inject_stale());
                for fact in introduce::loaded(self.loader.as_ref(), &unseen).await {
                    match admit::admitted_by_sql(&fact, &admission) {
                        Some(reason) if counted_by_withheld_sql(reason) => {},
                        Some(reason) => withheld.add(reason),
                        None => {
                            known.insert(fact.id.clone());
                            facts.insert(fact.id.clone(), fact);
                        },
                    }
                }
            }
            tiers.extend(introduce::extension_hits(&ranked, &known));
        }
        let fused = fusion::fuse(tiers, &retrieval.tiers, retrieval.rrf_k);
        Ok(fusion::attach(fused, facts))
    }

    async fn load_tiers(
        &self,
        tiers: &mut [TierIds],
    ) -> Result<HashMap<BrainFactId, BrainFact>, RecallError> {
        let ids: Vec<BrainFactId> = tiers
            .iter()
            .flat_map(|hits| hits.ids.iter())
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let facts: HashMap<BrainFactId, BrainFact> = self
            .loader
            .load(&ids)
            .await?
            .into_iter()
            .map(|fact| (fact.id.clone(), fact))
            .collect();
        for hits in tiers.iter_mut() {
            hits.ids.retain(|id| facts.contains_key(id));
        }
        Ok(facts)
    }

    fn plan(
        &self,
        visible: Vec<ScoredFact>,
        ctx: &RecallCtx,
        declared: &DeclaredRecall,
        shape: PlanShape,
    ) -> InjectionPlan {
        let workflow = ctx.workflow.as_ref().and_then(|id| declared.workflow(id));
        let limits = BudgetLimits::resolve(&declared.retrieval, workflow, shape.budget_tokens);
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
        budget::fill(candidates, &limits, shape.subject_named)
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
    ) -> Result<(Vec<ScoredFact>, WithheldSummary, bool), RecallError> {
        let mut withheld = WithheldSummary::default();
        let mut ranked: Vec<ScoredFact> = Vec::new();
        let fused = self.retrieve(request, ctx, declared, &mut withheld).await?;
        let subject_named = fused.iter().any(|scored| {
            fusion::found_by(scored, RetrievalTier::Subject)
                || fusion::found_by(scored, RetrievalTier::Linked)
        });
        for scored in fused {
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
        Ok((visible, withheld, subject_named))
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
        let (visible, mut withheld, named) = self.visible(request, &ctx, held, &declared).await?;
        let budget = match request.budget_tokens {
            0 => self.profile.recall.budget_tokens,
            requested => requested,
        };
        let shape = PlanShape {
            budget_tokens: budget,
            subject_named: named,
        };
        let plan = self.plan(visible, &ctx, &declared, shape);
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

const fn counted_by_withheld_sql(reason: WithheldReason) -> bool {
    matches!(
        reason,
        WithheldReason::Stale | WithheldReason::Disputed | WithheldReason::ReviewDue
    )
}
