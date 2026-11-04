//! A new-turn recall: context (held keys built once, `now` read once,
//! `as_of` from the request), retrieval over the working-set candidates
//! as ids (the five core tiers, the lifecycle count and the extension
//! retrievers run concurrently), one load of every id a tier returned,
//! fusion (the facts only an extension found are loaded and admitted by
//! the tier SQL's own rule, see [`super::introduce`] and [`super::admit`]),
//! visibility, the reranker ([`super::rerank`], over the visible facts
//! only, so the fill takes them in the order it gave) and the budget
//! fill. A recall *names a subject* when the
//! subject tier returned a fact the reader can see (a fact the linked
//! tier reached through a hidden subject's bridges names nothing), *has a
//! full-text hit* when that tier returned one the reader can see, and
//! *asks a kind* when the message carries one of a declared kind's ask
//! words and names nothing unknown ([`RecallSignal`], read after the
//! visibility pass); the fill's linked reserve and context cap read the
//! first, its no-signal gate all three. Before fusion the unknown-name
//! rule ([`super::signal`]) may empty the full-text tier.
//!
//! An id the load does not return (the fact was erased between the two
//! statements) is dropped from every tier before fusion
//! (`introduce::load_tiers`), so it takes no rank. An introduced fact the
//! admission rule rejects for a lifecycle reason is not counted again on
//! the receipt, since `withheld_counts`
//! already counted every reachable stale, disputed and review-due fact,
//! and one rejected for scope is counted as `scope`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use std::sync::Arc;

use chrono::Utc;
use futures::future::join_all;
use systemprompt_extension::brain::{
    InjectedFact, RecallBlock, RecallError, RecallOutcome, RecallRequest, SkipReason,
    WithheldSummary,
};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::{
    InjectionPlan, RecallCtx, RecallQuery, RetrievalTier, ScopeLevel, ScoredFact, WithheldReason,
};

use super::admit::{self, Admission};
use super::budget::{self, BudgetLimits, Candidate, RecallSignal};
use super::declared::DeclaredRecall;
use super::held::{self, HeldKeys};
use super::limits::tier_limits;
use super::score::{self, ScoreInput};
use super::service::{BrainRecallService, fact_text};
use super::visible::{Visibility, lifecycle_withheld};
use super::{fusion, introduce, rerank, signal};

#[derive(Debug, Clone, Copy)]
struct PlanShape {
    budget_tokens: u32,
    signal: RecallSignal,
    reranked: bool,
}

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
    ) -> Result<(Vec<ScoredFact>, bool), RecallError> {
        let retrieval = &declared.retrieval;
        let limits = tier_limits(retrieval);
        let asked = signal::asked_kinds(&request.subject_hints, &declared.kinds);
        let kind_asked = !asked.is_empty();
        let query = RecallQuery {
            subject_hints: request.subject_hints.clone(),
            limit: limits.tier,
            links_weight: retrieval.links_weight,
            synonyms: declared.synonyms.groups.clone(),
            search_max_df: retrieval.search_max_df,
            search_quorum: retrieval.search_quorum,
            search_retry_quorum: retrieval.search_retry_quorum,
            search_quorum_over: retrieval.search_quorum_over,
            alias_min_sim: retrieval.alias_min_sim,
            max_subjects: retrieval.max_subjects,
            search_stop_words: retrieval.search_stop_words.clone(),
            search_max_ndoc: retrieval.search_max_ndoc,
            subject_limit: limits.subject,
            asked_kinds: asked,
            alias_longest_wins: retrieval.alias_longest_wins,
            links_depth: retrieval.links_depth,
            links_decay: retrieval.links_decay,
            links_require_readable_anchor: retrieval.links_require_readable_anchor,
        };
        let fan_out = join_all(self.retrievers.iter().map(|r| r.retrieve(&query, ctx)));
        let (core, counts, extension) = tokio::join!(
            self.store.tier_ids(&query, ctx, limits.pinned),
            self.store.withheld_counts(ctx),
            fan_out,
        );
        for (reason, count) in counts? {
            *withheld.by_reason.entry(reason).or_default() += count;
        }
        let mut tiers = core?;
        let unknown_name = signal::withhold_unknown_names(
            self.store.as_ref(),
            &request.subject_hints,
            retrieval,
            &mut tiers,
            withheld,
        )
        .await?;
        let kind_asked = kind_asked && !unknown_name;
        let mut facts = introduce::load_tiers(self.loader.as_ref(), &mut tiers).await?;
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
        Ok((fusion::attach(fused, facts), kind_asked))
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
        if shape.reranked {
            budget::fill_in_order(candidates, &limits, shape.signal)
        } else {
            budget::fill(candidates, &limits, shape.signal)
        }
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
    ) -> Result<(Vec<ScoredFact>, WithheldSummary, RecallSignal), RecallError> {
        let mut withheld = WithheldSummary::default();
        let mut ranked: Vec<ScoredFact> = Vec::new();
        let (fused, kind_asked) = self.retrieve(request, ctx, declared, &mut withheld).await?;
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
        let signal = signal::signal_of(&visible, kind_asked);
        Ok((visible, withheld, signal))
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
        let (visible, mut withheld, signal) = self.visible(request, &ctx, held, &declared).await?;
        let (visible, reranked) = self.rerank(request, visible, &declared).await;
        let budget = match request.budget_tokens {
            0 => self.profile.recall.budget_tokens,
            requested => requested,
        };
        let shape = PlanShape {
            budget_tokens: budget,
            signal,
            reranked: reranked.is_some(),
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
            .with_tokens(plan.tokens)
            .with_reranked(reranked.as_ref().map(|r| rerank::record(r, &plan.facts)));
        Ok(RecallOutcome::Injected(block))
    }
}

const fn counted_by_withheld_sql(reason: WithheldReason) -> bool {
    matches!(
        reason,
        WithheldReason::Stale | WithheldReason::Disputed | WithheldReason::ReviewDue
    )
}
