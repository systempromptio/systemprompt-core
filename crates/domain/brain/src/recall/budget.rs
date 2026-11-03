//! Core injection fill.
//!
//! The visible facts are ordered by [`FillOrder`]: `rank` (recall plan
//! P2 step 1) takes descending fused score, then in-context importance,
//! then fact id, so a fact every tier agreed on is never cut for a
//! higher-importance sibling found by one; `importance` is the pre-P2
//! order (importance, then score, then id). In that order a fact is taken
//! while its kind is under its quota, the fact count is under `max_facts`
//! and the estimated tokens fit the budget. A fact over its kind's quota
//! is withheld as `Quota`; one past the fact count as `MaxFacts`; one
//! whose tokens do not fit as `Budget`. Tokens are estimated as a quarter
//! of the line's characters, rounded up.
//!
//! Three gates run before and inside that loop (P2 steps 2, 3, 4). A fact
//! whose fused score is under `min_fused_score` is withheld as `LowScore`
//! before the fill, so a message no tier scores injects nothing. When the
//! recall named a subject, `linked_share` of `max_facts` (rounded to the
//! nearest, never more than the link hits present) is
//! held for facts the linked tier found: another fact is `MaxFacts` once
//! only those slots remain, and a linked fact withheld for its quota or
//! tokens releases its slot. When a subject is named, facts found by the
//! context tier alone may take at most `context_share` of `max_facts`
//! (rounded down); one past that is withheld as `ContextCap`.
//!
//! The citation loop (P2B): before any of that, `usage_weight ×
//! (cited_rate − ignored_rate)` is added to each fact's fused score, so
//! the `low_score` gate and the `rank` order both see it; the same term
//! is the tie-break between importance and fact id, so a cited fact
//! outranks an ignored sibling under `importance` order too. The rates
//! sum to one, so a fact with no usage history (0.5/0.5 from the job's
//! prior, or 0/0 before the job first rates it) adds exactly zero, and a
//! weight of zero leaves every score untouched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_identifiers::FactKindId;
use systemprompt_models::brain::{
    BrainFact, FillOrder, InjectionPlan, RetrievalDoc, RetrievalTier, ScoredFact, WithheldFact,
    WithheldReason, WorkflowProfileDecl,
};

use super::fusion::found_by;

/// The limits one recall fills against.
#[derive(Debug, Clone, PartialEq)]
pub struct BudgetLimits {
    pub quotas: BTreeMap<FactKindId, u32>,
    pub max_facts: u32,
    pub budget_tokens: u32,
    pub order: FillOrder,
    pub linked_share: f64,
    pub min_fused_score: f64,
    pub context_share: f64,
    pub usage_weight: f64,
}

impl BudgetLimits {
    #[must_use]
    pub fn resolve(
        retrieval: &RetrievalDoc,
        workflow: Option<&WorkflowProfileDecl>,
        budget_tokens: u32,
    ) -> Self {
        let mut quotas = retrieval.quotas.clone();
        let mut budget = budget_tokens;
        if let Some(workflow) = workflow {
            quotas.extend(workflow.quotas.iter().map(|(k, v)| (k.clone(), *v)));
            if let Some(cap) = workflow.budget_tokens {
                budget = budget.min(cap);
            }
        }
        Self {
            quotas,
            max_facts: retrieval.max_facts,
            budget_tokens: budget,
            order: retrieval.fill_order,
            linked_share: retrieval.linked_share,
            min_fused_score: retrieval.min_fused_score,
            context_share: retrieval.context_share,
            usage_weight: retrieval.usage_weight,
        }
    }

    #[must_use]
    pub fn plain(quotas: BTreeMap<FactKindId, u32>, max_facts: u32, budget_tokens: u32) -> Self {
        Self {
            quotas,
            max_facts,
            budget_tokens,
            ..Self::resolve(&RetrievalDoc::default(), None, budget_tokens)
        }
    }

    fn linked_reserve(&self, candidates: &[Candidate]) -> u32 {
        let linked = candidates
            .iter()
            .filter(|c| found_by(&c.scored, RetrievalTier::Linked))
            .count();
        share_of(self.max_facts, self.linked_share, f64::round).min(count(linked))
    }

    fn context_cap(&self) -> u32 {
        share_of(self.max_facts, self.context_share, f64::floor)
    }
}

/// A visible fact, the text it would be injected as, and its in-context
/// importance.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub scored: ScoredFact,
    pub text: String,
    pub importance: f64,
}

#[must_use]
pub fn estimate_tokens(text: &str) -> u32 {
    u32::try_from(text.chars().count().div_ceil(4)).unwrap_or(u32::MAX)
}

fn share_of(max_facts: u32, share: f64, round: fn(f64) -> f64) -> u32 {
    // Why: lint-ok: a slot count at most max_facts; the rounding is exact.
    let slots = round(f64::from(max_facts) * share.clamp(0.0, 1.0)) as u32;
    slots.min(max_facts)
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn context_only(scored: &ScoredFact) -> bool {
    !scored.ranks.is_empty()
        && scored
            .ranks
            .iter()
            .all(|rank| rank.tier == RetrievalTier::Context)
}

#[must_use]
pub fn usage_term(fact: &BrainFact, usage_weight: f64) -> f64 {
    usage_weight * (fact.usage_cited_rate - fact.usage_ignored_rate) + 0.0
}

fn apply_usage(candidates: &mut [Candidate], usage_weight: f64) {
    for candidate in candidates {
        candidate.scored.score += usage_term(&candidate.scored.fact, usage_weight);
    }
}

fn sort_candidates(candidates: &mut [Candidate], order: FillOrder, usage_weight: f64) {
    candidates.sort_by(|a, b| {
        let usage = |c: &Candidate| usage_term(&c.scored.fact, usage_weight);
        let (primary, secondary) = match order {
            FillOrder::Rank => (
                b.scored.score.total_cmp(&a.scored.score),
                b.importance.total_cmp(&a.importance),
            ),
            FillOrder::Importance => (
                b.importance.total_cmp(&a.importance),
                b.scored.score.total_cmp(&a.scored.score),
            ),
        };
        primary
            .then(secondary)
            .then_with(|| usage(b).total_cmp(&usage(a)))
            .then_with(|| a.scored.fact.id.cmp(&b.scored.fact.id))
    });
}

struct Fill<'a> {
    limits: &'a BudgetLimits,
    plan: InjectionPlan,
    per_kind: BTreeMap<FactKindId, u32>,
    reserved: u32,
    context_cap: u32,
    context_taken: u32,
}

impl Fill<'_> {
    fn admit(&mut self, candidate: &Candidate) -> Option<WithheldReason> {
        let fact = &candidate.scored.fact;
        let linked = found_by(&candidate.scored, RetrievalTier::Linked);
        let taken = u32::try_from(self.plan.facts.len()).unwrap_or(u32::MAX);
        let cap = if linked {
            self.limits.max_facts
        } else {
            self.limits.max_facts.saturating_sub(self.reserved)
        };
        let kind_taken = self.per_kind.get(&fact.kind).copied().unwrap_or(0);
        let over_quota = self
            .limits
            .quotas
            .get(&fact.kind)
            .is_some_and(|q| kind_taken >= *q);
        let reason = if candidate.scored.score < self.limits.min_fused_score {
            Some(WithheldReason::LowScore)
        } else if over_quota {
            Some(WithheldReason::Quota)
        } else if taken >= cap {
            Some(WithheldReason::MaxFacts)
        } else if context_only(&candidate.scored) && self.context_taken >= self.context_cap {
            Some(WithheldReason::ContextCap)
        } else if self
            .plan
            .tokens
            .saturating_add(estimate_tokens(&candidate.text))
            > self.limits.budget_tokens
        {
            Some(WithheldReason::Budget)
        } else {
            None
        };
        match reason {
            Some(_) if linked => self.reserved = self.reserved.saturating_sub(1),
            Some(_) => {},
            None => {
                self.plan.tokens += estimate_tokens(&candidate.text);
                *self.per_kind.entry(fact.kind.clone()).or_default() += 1;
                if linked {
                    self.reserved = self.reserved.saturating_sub(1);
                }
                if context_only(&candidate.scored) {
                    self.context_taken += 1;
                }
            },
        }
        reason
    }
}

#[must_use]
pub fn fill(
    mut candidates: Vec<Candidate>,
    limits: &BudgetLimits,
    subject_named: bool,
) -> InjectionPlan {
    apply_usage(&mut candidates, limits.usage_weight);
    sort_candidates(&mut candidates, limits.order, limits.usage_weight);
    let (reserved, context_cap) = if subject_named {
        (limits.linked_reserve(&candidates), limits.context_cap())
    } else {
        (0, limits.max_facts)
    };
    let mut fill = Fill {
        limits,
        plan: InjectionPlan::default(),
        per_kind: BTreeMap::new(),
        reserved,
        context_cap,
        context_taken: 0,
    };
    for candidate in candidates {
        match fill.admit(&candidate) {
            Some(reason) => fill.plan.withheld.push(WithheldFact {
                id: candidate.scored.fact.id.clone(),
                reason,
            }),
            None => fill.plan.facts.push(candidate.scored),
        }
    }
    fill.plan
}
