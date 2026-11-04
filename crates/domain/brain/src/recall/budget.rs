//! Core injection fill.
//!
//! The visible facts are ordered by [`FillOrder`]: `rank` takes descending
//! fused score, then in-context importance,
//! then fact id, so a fact every tier agreed on is never cut for a
//! higher-importance sibling found by one; `importance` is the previous
//! order (importance, then score, then id); [`fill_in_order`] takes the
//! candidates as given (a reranker's order) and sorts nothing. In that
//! order a fact is taken while its kind is under its quota, the fact
//! count is under `max_facts`
//! and the estimated tokens fit the budget. A fact over its kind's quota
//! is withheld as `Quota`; one past the fact count as `MaxFacts`; one
//! whose tokens do not fit as `Budget`. Tokens are estimated as a quarter
//! of the line's characters, rounded up.
//!
//! Three gates run before and inside that loop. A fact
//! whose fused score is under `min_fused_score` is withheld as `LowScore`
//! before the fill, so a message no tier scores injects nothing. When the
//! recall named a subject, `linked_share` of `max_facts` (rounded to the
//! nearest, never more than the link hits present) is
//! held for facts the linked tier found: another fact is `MaxFacts` once
//! only those slots remain, and a linked fact withheld for its quota or
//! tokens releases its slot. When a subject is named, facts found by the
//! context tier alone may take at most `context_share` of `max_facts`
//! (rounded down); one past that is withheld as `ContextCap`. With
//! `context_requires_signal` and no subject named, no full-text hit and
//! no asked kind (the message carries no word of any fact and names no
//! kind, see [`RecallSignal`]) that cap is zero: every context-only fact
//! is withheld as `ContextCap` and every fact only the extension tier
//! found as `NoSignal`, while a fact a core tier also found is never
//! touched. The extension tier is neither a signal nor another tier here:
//! a fact ranked by the context tier and an extension retriever is still
//! context-only, since a dense neighbour of a haiku is no evidence the
//! message is about the project.
//!
//! The citation loop: before any of that, `usage_weight ×
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
    FillOrder, InjectionPlan, RetrievalDoc, RetrievalTier, WithheldFact, WithheldReason,
    WorkflowProfileDecl,
};

pub use super::candidate::{Candidate, estimate_tokens, usage_term};
use super::candidate::{apply_usage, context_only, extension_only, share_of, sort_candidates};
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
    pub context_requires_signal: bool,
}

pub use super::signal::RecallSignal;

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
            context_requires_signal: retrieval.context_requires_signal,
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
        share_of(self.max_facts, self.linked_share, f64::round)
            .min(u32::try_from(linked).unwrap_or(u32::MAX))
    }

    const fn gate_closed(&self, signal: RecallSignal) -> bool {
        self.context_requires_signal && !signal.any()
    }

    fn context_cap(&self, signal: RecallSignal) -> u32 {
        if signal.subject_named {
            share_of(self.max_facts, self.context_share, f64::floor)
        } else if self.gate_closed(signal) {
            0
        } else {
            self.max_facts
        }
    }
}

struct Fill<'a> {
    limits: &'a BudgetLimits,
    plan: InjectionPlan,
    per_kind: BTreeMap<FactKindId, u32>,
    reserved: u32,
    context_cap: u32,
    context_taken: u32,
    gate_closed: bool,
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
        } else if self.gate_closed && extension_only(&candidate.scored) {
            Some(WithheldReason::NoSignal)
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
    signal: RecallSignal,
) -> InjectionPlan {
    apply_usage(&mut candidates, limits.usage_weight);
    sort_candidates(&mut candidates, limits.order, limits.usage_weight);
    fill_sorted(candidates, limits, signal)
}

#[must_use]
pub fn fill_in_order(
    mut candidates: Vec<Candidate>,
    limits: &BudgetLimits,
    signal: RecallSignal,
) -> InjectionPlan {
    apply_usage(&mut candidates, limits.usage_weight);
    fill_sorted(candidates, limits, signal)
}

fn fill_sorted(
    candidates: Vec<Candidate>,
    limits: &BudgetLimits,
    signal: RecallSignal,
) -> InjectionPlan {
    let reserved = if signal.subject_named {
        limits.linked_reserve(&candidates)
    } else {
        0
    };
    let mut fill = Fill {
        limits,
        plan: InjectionPlan::default(),
        per_kind: BTreeMap::new(),
        reserved,
        context_cap: limits.context_cap(signal),
        context_taken: 0,
        gate_closed: limits.gate_closed(signal),
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
